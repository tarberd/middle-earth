use std::path::Path;
use thiserror::Error;

use super::element::{DomainXmlElement, DomainXmlParseError};

/// Enumerates errors that can occur during domain template XML synthesis.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum DomainTemplateSynthesisError {
    #[error("Failed to parse domain template XML: {0}")]
    XmlParseError(#[from] DomainXmlParseError),

    #[error("Template XML root element must be <domain>, but found an alternative root or empty document")]
    MissingRootDomainElement,

    #[error("No <name> element found in template XML: expected a <name> element to inject instance identity")]
    MissingNameElement,

    #[error("No <uuid> element found in template XML: expected a <uuid> element to inject instance UUID")]
    MissingUuidElement,

    #[error(
        "No designated OS disk found in template XML: expected exactly one <disk> element with onehost:role='os-disk'"
    )]
    MissingDesignatedOsDisk,

    #[error(
        "Ambiguous template XML: expected exactly one designated OS disk, but found {count} <disk> elements with onehost:role='os-disk'"
    )]
    MultipleDesignatedOsDisksFound { count: usize },
}

/// Parameters supplied to `DomainTemplateEngine` to synthesize concrete Domain XML.
pub struct DomainTemplateInjectionParameters<'a> {
    pub instance_name: &'a str,
    pub instance_uuid: &'a str,
    pub overlay_disk_path: &'a Path,
    pub instance_nvram_path: &'a Path,
    pub nvram_template_path: Option<&'a Path>,
    pub ovmf_code_path: Option<&'a Path>,
}

/// Transforms declarative Libvirt Domain Template XML into concrete, instance-specific Domain XML.
pub struct DomainTemplateEngine;

impl DomainTemplateEngine {
    /// Ingests a domain template XML string and returns the concrete Domain XML string via pure functional AST transformation.
    pub fn synthesize_concrete_domain_xml(
        template_xml_content: &str,
        injection_parameters: &DomainTemplateInjectionParameters,
    ) -> Result<String, DomainTemplateSynthesisError> {
        let template_root = DomainXmlElement::parse(template_xml_content)?;

        (template_root.tag_name == "domain")
            .then_some(())
            .ok_or(DomainTemplateSynthesisError::MissingRootDomainElement)?;

        template_root
            .has_child("name")
            .then_some(())
            .ok_or(DomainTemplateSynthesisError::MissingNameElement)?;

        template_root
            .has_child("uuid")
            .then_some(())
            .ok_or(DomainTemplateSynthesisError::MissingUuidElement)?;

        // Validate designated OS disk count
        let designated_disk_count = template_root
            .find_child_by_tag("devices")
            .map(|devices| {
                devices
                    .children
                    .iter()
                    .filter(|device_child| {
                        device_child.tag_name == "disk" && Self::is_designated_os_disk(device_child)
                    })
                    .count()
            })
            .unwrap_or(0);

        match designated_disk_count {
            0 => Err(DomainTemplateSynthesisError::MissingDesignatedOsDisk),
            1 => Ok(()),
            count => Err(DomainTemplateSynthesisError::MultipleDesignatedOsDisksFound { count }),
        }?;

        // 1. Strip xmlns:onehost attributes and transform children purely
        let DomainXmlElement {
            attributes,
            children,
            ..
        } = template_root;

        let sanitized_attributes: Vec<(String, String)> = attributes
            .into_iter()
            .filter(|(attribute_key, _)| {
                !attribute_key.starts_with("xmlns:onehost") && attribute_key != "onehost"
            })
            .collect();

        let transformed_children: Vec<DomainXmlElement> = children
            .into_iter()
            .map(|child| match child.tag_name.as_str() {
                "name" => child.with_replaced_text(injection_parameters.instance_name),
                "uuid" => child.with_replaced_text(injection_parameters.instance_uuid),
                "os" => Self::transform_os_element(child, injection_parameters),
                "devices" => Self::transform_devices_element(child, injection_parameters),
                _ => child,
            })
            .collect();

        let transformed_domain = DomainXmlElement::new("domain")
            .with_attributes(sanitized_attributes)
            .with_children(transformed_children);

        Ok(transformed_domain.to_xml_string(0))
    }

    fn is_designated_os_disk(element: &DomainXmlElement) -> bool {
        element.attributes.iter().any(|(attribute_key, attribute_value)| {
            (attribute_key == "onehost:role"
                || attribute_key.ends_with(":role")
                || attribute_key == "role")
                && attribute_value == "os-disk"
        })
    }

    fn transform_os_element(
        os_element: DomainXmlElement,
        injection_parameters: &DomainTemplateInjectionParameters,
    ) -> DomainXmlElement {
        let rendered_nvram_path = injection_parameters
            .instance_nvram_path
            .to_str()
            .unwrap_or("");

        let has_nvram = os_element.has_child("nvram");

        if has_nvram {
            os_element.transform_children(|child| {
                if child.tag_name == "nvram" {
                    let base_nvram = child.with_replaced_text(rendered_nvram_path);
                    Some(match injection_parameters.nvram_template_path {
                        Some(template_path) => {
                            base_nvram.with_attribute("template", template_path.to_str().unwrap_or(""))
                        }
                        None => base_nvram,
                    })
                } else {
                    Some(child)
                }
            })
        } else {
            let base_nvram = DomainXmlElement::with_text_node("nvram", rendered_nvram_path);
            let new_nvram = match injection_parameters.nvram_template_path {
                Some(template_path) => {
                    base_nvram.with_attribute("template", template_path.to_str().unwrap_or(""))
                }
                None => base_nvram,
            };
            os_element.with_child(new_nvram)
        }
    }

    fn transform_devices_element(
        devices_element: DomainXmlElement,
        injection_parameters: &DomainTemplateInjectionParameters,
    ) -> DomainXmlElement {
        devices_element.transform_children(|child| {
            if child.tag_name == "disk" && Self::is_designated_os_disk(&child) {
                Some(Self::transform_designated_disk(
                    child,
                    injection_parameters.overlay_disk_path,
                ))
            } else {
                Some(child)
            }
        })
    }

    fn transform_designated_disk(
        disk: DomainXmlElement,
        overlay_disk_path: &Path,
    ) -> DomainXmlElement {
        let DomainXmlElement {
            tag_name,
            attributes,
            children,
            ..
        } = disk;

        let sanitized_attributes: Vec<(String, String)> = attributes
            .into_iter()
            .filter(|(attribute_key, _)| {
                !(attribute_key == "onehost:role"
                    || attribute_key.ends_with(":role")
                    || attribute_key == "role")
            })
            .collect();

        let remaining_children: Vec<DomainXmlElement> = children
            .into_iter()
            .filter(|child| child.tag_name != "driver" && child.tag_name != "source")
            .collect();

        let driver_node = DomainXmlElement::new("driver")
            .with_attribute("name", "qemu")
            .with_attribute("type", "qcow2");

        let source_node = DomainXmlElement::new("source")
            .with_attribute("file", overlay_disk_path.to_str().unwrap_or(""));

        let final_children: Vec<DomainXmlElement> = [driver_node, source_node]
            .into_iter()
            .chain(remaining_children)
            .collect();

        DomainXmlElement::new(tag_name)
            .with_attributes(sanitized_attributes)
            .with_children(final_children)
    }
}
