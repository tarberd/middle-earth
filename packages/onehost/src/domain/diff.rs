use std::collections::BTreeMap;
use thiserror::Error;

use super::element::{DomainXmlElement, DomainXmlParseError};

/// Enumerates errors that can occur during Domain XML normalization or diff calculation.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum DomainXmlDiffError {
    #[error("Failed to parse synthesized Domain XML: {0}")]
    SynthesizedXmlParseError(DomainXmlParseError),

    #[error("Failed to parse live Domain XML: {0}")]
    LiveXmlParseError(DomainXmlParseError),

    #[error("Synthesized Domain XML has no root <domain> element")]
    MissingSynthesizedRootDomain,

    #[error("Live Domain XML has no root <domain> element")]
    MissingLiveRootDomain,
}

/// Represents a specific semantic difference between synthesized and live Domain XML.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainXmlDifference {
    TextContentMismatch {
        element_path: String,
        expected: String,
        actual: String,
    },
    AttributeMismatch {
        element_path: String,
        attribute_name: String,
        expected: String,
        actual: String,
    },
    MissingAttribute {
        element_path: String,
        attribute_name: String,
        expected: String,
    },
    UnexpectedAttribute {
        element_path: String,
        attribute_name: String,
        actual: String,
    },
    MissingChildElement {
        parent_path: String,
        expected_child: String,
    },
    UnexpectedChildElement {
        parent_path: String,
        actual_child: String,
    },
}

/// Result of comparing synthesized Domain XML with live Libvirt Domain XML.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainXmlDiffResult {
    pub has_drift: bool,
    pub differences: Vec<DomainXmlDifference>,
    pub normalized_synthesized_xml: String,
    pub normalized_live_xml: String,
}

impl DomainXmlDiffResult {
    /// Formats the differences into a human-readable summary.
    pub fn summary(&self) -> String {
        if !self.has_drift {
            return "No semantic drift detected between declared and live Domain XML.".to_string();
        }

        let header = format!("Detected {} semantic difference(s):\n", self.differences.len());
        let diff_lines: String = self
            .differences
            .iter()
            .enumerate()
            .map(|(index, diff)| match diff {
                DomainXmlDifference::TextContentMismatch {
                    element_path,
                    expected,
                    actual,
                } => {
                    format!(
                        "  {}. Value mismatch at {}: expected '{}', live has '{}'\n",
                        index + 1,
                        element_path,
                        expected,
                        actual
                    )
                }
                DomainXmlDifference::AttributeMismatch {
                    element_path,
                    attribute_name,
                    expected,
                    actual,
                } => {
                    format!(
                        "  {}. Attribute '{}' mismatch at {}: expected '{}', live has '{}'\n",
                        index + 1,
                        attribute_name,
                        element_path,
                        expected,
                        actual
                    )
                }
                DomainXmlDifference::MissingAttribute {
                    element_path,
                    attribute_name,
                    expected,
                } => {
                    format!(
                        "  {}. Missing attribute '{}' at {}: expected '{}'\n",
                        index + 1,
                        attribute_name,
                        element_path,
                        expected
                    )
                }
                DomainXmlDifference::UnexpectedAttribute {
                    element_path,
                    attribute_name,
                    actual,
                } => {
                    format!(
                        "  {}. Unexpected attribute '{}' at {}: live has '{}'\n",
                        index + 1,
                        attribute_name,
                        element_path,
                        actual
                    )
                }
                DomainXmlDifference::MissingChildElement {
                    parent_path,
                    expected_child,
                } => {
                    format!(
                        "  {}. Missing expected child under {}:\n{}\n",
                        index + 1,
                        parent_path,
                        expected_child
                    )
                }
                DomainXmlDifference::UnexpectedChildElement {
                    parent_path,
                    actual_child,
                } => {
                    format!(
                        "  {}. Unexpected child under {}:\n{}\n",
                        index + 1,
                        parent_path,
                        actual_child
                    )
                }
            })
            .collect();

        format!("{header}{diff_lines}")
    }
}

/// Normalizes Domain XML trees to eliminate volatile runtime attributes and auto-allocated defaults using pure functional transformations.
pub struct DomainXmlNormalizer;

impl DomainXmlNormalizer {
    /// Pure functional normalization: consumes input trees and returns a tuple of normalized immutable trees.
    pub fn normalize_domain_trees(
        synthesized: DomainXmlElement,
        live: DomainXmlElement,
    ) -> (DomainXmlElement, DomainXmlElement) {
        // 1. Strip dynamic domain id attribute from root <domain>
        let synth_without_id = synthesized.without_attribute("id");
        let live_without_id = live.without_attribute("id");

        // 2. Remove all <alias> tags recursively
        let synth_without_alias = synth_without_id.without_child_tag_recursive("alias");
        let live_without_alias = live_without_id.without_child_tag_recursive("alias");

        // 3. Remove dynamic <seclabel> from live if synthesized has none
        let synth_has_dynamic_seclabel = synth_without_alias
            .children
            .iter()
            .any(|child| child.tag_name == "seclabel" && child.get_attribute("type") == Some("dynamic"));

        let live_without_seclabel = if !synth_has_dynamic_seclabel {
            live_without_alias.transform_children(|child| {
                if child.tag_name == "seclabel" && child.get_attribute("type") == Some("dynamic") {
                    None
                } else {
                    Some(child)
                }
            })
        } else {
            live_without_alias
        };

        // 4. Reconcile device defaults (auto-allocated addresses and MACs)
        let (synth_reconciled, live_reconciled) =
            Self::reconcile_device_defaults(synth_without_alias, live_without_seclabel);

        // 5. Deterministic canonical sorting of child elements
        let synth_canonical = Self::sort_element_children_deterministically(synth_reconciled);
        let live_canonical = Self::sort_element_children_deterministically(live_reconciled);

        (synth_canonical, live_canonical)
    }

    fn reconcile_device_defaults(
        synthesized_root: DomainXmlElement,
        live_root: DomainXmlElement,
    ) -> (DomainXmlElement, DomainXmlElement) {
        let synth_device_keys: Vec<(String, bool, bool)> = synthesized_root
            .find_child_by_tag("devices")
            .map(|devices| {
                devices
                    .children
                    .iter()
                    .map(|synthesized_device| {
                        (
                            Self::compute_device_identity_key(synthesized_device),
                            synthesized_device.has_child("address"),
                            synthesized_device.has_child("mac"),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();

        let new_live_root = live_root.transform_children(|child| {
            if child.tag_name == "devices" {
                Some(child.transform_children(|live_device| {
                    let live_key = Self::compute_device_identity_key(&live_device);
                    let matching_synth = synth_device_keys.iter().find(|(key, _, _)| key == &live_key);

                    if let Some((_, has_address, has_mac)) = matching_synth {
                        let without_address = if !has_address {
                            live_device.transform_children(|device_child| {
                                if device_child.tag_name == "address" {
                                    None
                                } else {
                                    Some(device_child)
                                }
                            })
                        } else {
                            live_device
                        };

                        let reconciled_device = if without_address.tag_name == "interface" && !has_mac {
                            without_address.transform_children(|device_child| {
                                if device_child.tag_name == "mac" {
                                    None
                                } else {
                                    Some(device_child)
                                }
                            })
                        } else {
                            without_address
                        };

                        Some(reconciled_device)
                    } else {
                        Some(live_device)
                    }
                }))
            } else {
                Some(child)
            }
        });

        (synthesized_root, new_live_root)
    }

    /// Computes a semantic identity key for matching devices across trees.
    pub fn compute_device_identity_key(element: &DomainXmlElement) -> String {
        match element.tag_name.as_str() {
            "disk" => {
                let target_device = element
                    .find_child_by_tag("target")
                    .and_then(|target_element| target_element.get_attribute("dev"))
                    .unwrap_or("");
                format!("disk[target={}]", target_device)
            }
            "interface" => {
                let source = element
                    .find_child_by_tag("source")
                    .and_then(|source_element| {
                        source_element
                            .get_attribute("bridge")
                            .or_else(|| source_element.get_attribute("network"))
                    })
                    .unwrap_or("");
                let target_device = element
                    .find_child_by_tag("target")
                    .and_then(|target_element| target_element.get_attribute("dev"))
                    .unwrap_or("");
                if !target_device.is_empty() {
                    format!("interface[source={},target={}]", source, target_device)
                } else {
                    format!("interface[source={}]", source)
                }
            }
            "controller" => {
                let controller_type = element.get_attribute("type").unwrap_or("");
                let controller_index = element.get_attribute("index").unwrap_or("");
                format!("controller[type={},index={}]", controller_type, controller_index)
            }
            "hostdev" => {
                let hostdev_type = element.get_attribute("type").unwrap_or("");
                let source_bus = element
                    .find_child_by_tag("source")
                    .and_then(|source_element| source_element.find_child_by_tag("address"))
                    .and_then(|address_element| address_element.get_attribute("bus"))
                    .unwrap_or("");
                format!("hostdev[type={},bus={}]", hostdev_type, source_bus)
            }
            "qemu:arg" => {
                let argument_value = element.get_attribute("value").unwrap_or("");
                format!("qemu:arg[val={}]", argument_value)
            }
            _ => element.tag_name.clone(),
        }
    }

    fn sort_element_children_deterministically(element: DomainXmlElement) -> DomainXmlElement {
        let domain_order = [
            "name",
            "uuid",
            "title",
            "description",
            "metadata",
            "memory",
            "currentMemory",
            "memoryBacking",
            "vcpu",
            "vcpus",
            "iothreads",
            "cputune",
            "os",
            "features",
            "cpu",
            "clock",
            "on_poweroff",
            "on_reboot",
            "on_crash",
            "pm",
            "devices",
            "qemu:commandline",
        ];

        let devices_order = [
            "emulator",
            "disk",
            "controller",
            "interface",
            "serial",
            "console",
            "channel",
            "input",
            "tpm",
            "graphics",
            "sound",
            "audio",
            "video",
            "hostdev",
            "memballoon",
        ];

        let DomainXmlElement {
            tag_name,
            attributes,
            children,
            text_content,
        } = element;

        let sorted_children: Vec<DomainXmlElement> = if tag_name == "domain" {
            children
                .into_iter()
                .map(Self::sort_element_children_deterministically)
                .enumerate()
                .map(|(index, child)| {
                    let position = domain_order
                        .iter()
                        .position(|&item| item == child.tag_name)
                        .unwrap_or(usize::MAX);
                    ((position, child.tag_name.clone(), index), child)
                })
                .collect::<BTreeMap<_, _>>()
                .into_values()
                .collect()
        } else if tag_name == "devices" {
            children
                .into_iter()
                .map(Self::sort_element_children_deterministically)
                .enumerate()
                .map(|(index, child)| {
                    let position = devices_order
                        .iter()
                        .position(|&item| item == child.tag_name)
                        .unwrap_or(usize::MAX);
                    let identity = Self::compute_device_identity_key(&child);
                    ((position, identity, index), child)
                })
                .collect::<BTreeMap<_, _>>()
                .into_values()
                .collect()
        } else {
            children
                .into_iter()
                .map(Self::sort_element_children_deterministically)
                .enumerate()
                .map(|(index, child)| {
                    let identity = Self::compute_device_identity_key(&child);
                    ((identity, index), child)
                })
                .collect::<BTreeMap<_, _>>()
                .into_values()
                .collect()
        };

        let base_element = DomainXmlElement::new(tag_name)
            .with_attributes(attributes)
            .with_children(sorted_children);

        text_content
            .into_iter()
            .fold(base_element, |element_acc, text| element_acc.with_replaced_text(text))
    }
}

/// Computes semantic differences between synthesized concrete Domain XML and live Libvirt XML using functional comparison pipelines.
pub struct DomainXmlDiffer;

impl DomainXmlDiffer {
    /// Compares synthesized Domain XML against live Libvirt Domain XML.
    pub fn compare_domain_xmls(
        synthesized_xml_content: &str,
        live_xml_content: &str,
    ) -> Result<DomainXmlDiffResult, DomainXmlDiffError> {
        let synthesized_root = DomainXmlElement::parse(synthesized_xml_content)
            .map_err(DomainXmlDiffError::SynthesizedXmlParseError)?;

        (synthesized_root.tag_name == "domain")
            .then_some(())
            .ok_or(DomainXmlDiffError::MissingSynthesizedRootDomain)?;

        let live_root = DomainXmlElement::parse(live_xml_content)
            .map_err(DomainXmlDiffError::LiveXmlParseError)?;

        (live_root.tag_name == "domain")
            .then_some(())
            .ok_or(DomainXmlDiffError::MissingLiveRootDomain)?;

        // Apply pure functional normalization
        let (normalized_synthesized, normalized_live) =
            DomainXmlNormalizer::normalize_domain_trees(synthesized_root, live_root);

        let differences = Self::calculate_differences(&normalized_synthesized, &normalized_live, "");

        let normalized_synthesized_xml = normalized_synthesized.to_xml_string(0);
        let normalized_live_xml = normalized_live.to_xml_string(0);

        let has_drift = !differences.is_empty() || normalized_synthesized_xml != normalized_live_xml;

        Ok(DomainXmlDiffResult {
            has_drift,
            differences,
            normalized_synthesized_xml,
            normalized_live_xml,
        })
    }

    fn calculate_differences(
        synthesized: &DomainXmlElement,
        live: &DomainXmlElement,
        parent_path: &str,
    ) -> Vec<DomainXmlDifference> {
        let current_path = if parent_path.is_empty() {
            format!("/{}", synthesized.tag_name)
        } else {
            format!("{}/{}", parent_path, synthesized.tag_name)
        };

        // 1. Attribute differences (pure functional iterator chaining)
        let missing_and_mismatched_attributes = synthesized
            .attributes
            .iter()
            .filter_map(|(attribute_key, expected_value)| match live.get_attribute(attribute_key) {
                Some(actual_value) if expected_value != actual_value => {
                    Some(DomainXmlDifference::AttributeMismatch {
                        element_path: current_path.clone(),
                        attribute_name: attribute_key.clone(),
                        expected: expected_value.clone(),
                        actual: actual_value.to_string(),
                    })
                }
                None => Some(DomainXmlDifference::MissingAttribute {
                    element_path: current_path.clone(),
                    attribute_name: attribute_key.clone(),
                    expected: expected_value.clone(),
                }),
                _ => None,
            });

        let unexpected_attributes = live
            .attributes
            .iter()
            .filter(|(attribute_key, _)| synthesized.get_attribute(attribute_key).is_none())
            .map(|(attribute_key, actual_value)| DomainXmlDifference::UnexpectedAttribute {
                element_path: current_path.clone(),
                attribute_name: attribute_key.clone(),
                actual: actual_value.clone(),
            });

        // 2. Text content difference
        let text_difference = match (synthesized.text_content.as_deref(), live.text_content.as_deref()) {
            (Some(expected_text), Some(actual_text)) if expected_text != actual_text => {
                Some(DomainXmlDifference::TextContentMismatch {
                    element_path: current_path.clone(),
                    expected: expected_text.to_string(),
                    actual: actual_text.to_string(),
                })
            }
            (Some(expected_text), None) => Some(DomainXmlDifference::TextContentMismatch {
                element_path: current_path.clone(),
                expected: expected_text.to_string(),
                actual: String::new(),
            }),
            (None, Some(actual_text)) => Some(DomainXmlDifference::TextContentMismatch {
                element_path: current_path.clone(),
                expected: String::new(),
                actual: actual_text.to_string(),
            }),
            _ => None,
        };

        // 3. Child differences using semantic identity keys
        let (matched_live_children, child_differences) = synthesized.children.iter().fold(
            (Vec::<&DomainXmlElement>::new(), Vec::<DomainXmlDifference>::new()),
            |(matched_children, accumulated_differences), synthesized_child| {
                let key = DomainXmlNormalizer::compute_device_identity_key(synthesized_child);
                let matching_live = live
                    .children
                    .iter()
                    .find(|live_child| {
                        !matched_children.contains(live_child)
                            && DomainXmlNormalizer::compute_device_identity_key(live_child) == key
                    });

                match matching_live {
                    Some(live_child) => {
                        let child_sub_differences =
                            Self::calculate_differences(synthesized_child, live_child, &current_path);
                        let updated_matched: Vec<&DomainXmlElement> = matched_children
                            .into_iter()
                            .chain(std::iter::once(live_child))
                            .collect();
                        let updated_differences: Vec<DomainXmlDifference> = accumulated_differences
                            .into_iter()
                            .chain(child_sub_differences)
                            .collect();
                        (updated_matched, updated_differences)
                    }
                    None => {
                        let missing_difference = DomainXmlDifference::MissingChildElement {
                            parent_path: current_path.clone(),
                            expected_child: synthesized_child.to_xml_string(0).trim().to_string(),
                        };
                        let updated_differences: Vec<DomainXmlDifference> = accumulated_differences
                            .into_iter()
                            .chain(std::iter::once(missing_difference))
                            .collect();
                        (matched_children, updated_differences)
                    }
                }
            },
        );

        let unexpected_child_differences = live
            .children
            .iter()
            .filter(|live_child| !matched_live_children.contains(live_child))
            .map(|live_child| DomainXmlDifference::UnexpectedChildElement {
                parent_path: current_path.clone(),
                actual_child: live_child.to_xml_string(0).trim().to_string(),
            });

        // Combine all differences functionally
        missing_and_mismatched_attributes
            .chain(unexpected_attributes)
            .chain(text_difference)
            .chain(child_differences)
            .chain(unexpected_child_differences)
            .collect()
    }
}
