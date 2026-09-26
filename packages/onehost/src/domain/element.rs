use quick_xml::events::Event;
use quick_xml::reader::Reader;

/// An immutable in-memory representation of an XML element for functional transformation and diffing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainXmlElement {
    pub tag_name: String,
    pub attributes: Vec<(String, String)>,
    pub children: Vec<DomainXmlElement>,
    pub text_content: Option<String>,
}

impl DomainXmlElement {
    /// Constructs a new element with the given tag name, empty attributes, and empty children.
    pub fn new(tag_name: impl Into<String>) -> Self {
        Self {
            tag_name: tag_name.into(),
            attributes: Vec::new(),
            children: Vec::new(),
            text_content: None,
        }
    }

    /// Constructs an element with tag name and text content.
    pub fn with_text_node(tag_name: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            tag_name: tag_name.into(),
            attributes: Vec::new(),
            children: Vec::new(),
            text_content: Some(text.into()),
        }
    }

    /// Look up an attribute value by key.
    pub fn get_attribute(&self, key: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|(attribute_key, _)| attribute_key == key)
            .map(|(_, attribute_value)| attribute_value.as_str())
    }

    /// Checks if a child element with the specified tag name exists.
    pub fn has_child(&self, tag: &str) -> bool {
        self.children.iter().any(|child| child.tag_name == tag)
    }

    /// Finds the first child element with the specified tag name.
    pub fn find_child_by_tag(&self, tag: &str) -> Option<&DomainXmlElement> {
        self.children.iter().find(|child| child.tag_name == tag)
    }

    /// Checks if an attribute exists and has the specified value.
    pub fn has_attribute_value(&self, key: &str, expected_value: &str) -> bool {
        self.get_attribute(key) == Some(expected_value)
    }

    /// Checks if any attribute matches the given predicate.
    pub fn has_matching_attribute<P>(&self, predicate: P) -> bool
    where
        P: Fn(&str, &str) -> bool,
    {
        self.attributes
            .iter()
            .any(|(attribute_key, attribute_value)| predicate(attribute_key, attribute_value))
    }

    /// Looks up an attribute value on a named child element.
    pub fn child_attribute(&self, child_tag: &str, attribute_key: &str) -> Option<&str> {
        self.find_child_by_tag(child_tag)
            .and_then(|child| child.get_attribute(attribute_key))
    }

    /// Looks up text content on a named child element.
    pub fn child_text(&self, child_tag: &str) -> Option<&str> {
        self.find_child_by_tag(child_tag)
            .and_then(|child| child.text_content.as_deref())
    }

    /// Resolves a nested child element following a sequence of tag names.
    pub fn find_path<'a>(&'a self, tag_path: &[&str]) -> Option<&'a DomainXmlElement> {
        tag_path
            .iter()
            .try_fold(self, |current_node, &tag| current_node.find_child_by_tag(tag))
    }

    /// Resolves text content at a nested path of tag names.
    pub fn path_text(&self, tag_path: &[&str]) -> Option<&str> {
        self.find_path(tag_path)
            .and_then(|element| element.text_content.as_deref())
    }

    /// Returns an iterator yielding all children with the specified tag name.
    pub fn find_children<'a>(&'a self, tag: &'a str) -> impl Iterator<Item = &'a DomainXmlElement> {
        self.children.iter().filter(move |child| child.tag_name == tag)
    }

    /// Functional builder: returns a new element with an additional attribute.
    pub fn with_attribute(self, key: impl Into<String>, value: impl Into<String>) -> Self {
        let attribute_key = key.into();
        let attribute_value = value.into();
        let filtered: Vec<(String, String)> = self
            .attributes
            .into_iter()
            .filter(|(existing_key, _)| existing_key != &attribute_key)
            .collect();
        let updated_attributes: Vec<(String, String)> = filtered
            .into_iter()
            .chain(std::iter::once((attribute_key, attribute_value)))
            .collect::<std::collections::BTreeMap<_, _>>()
            .into_iter()
            .collect();

        Self {
            attributes: updated_attributes,
            ..self
        }
    }

    /// Functional builder: sets all attributes at once, sorting them alphabetically.
    pub fn with_attributes(self, attributes: Vec<(String, String)>) -> Self {
        let sorted_attributes: Vec<(String, String)> = attributes
            .into_iter()
            .collect::<std::collections::BTreeMap<_, _>>()
            .into_iter()
            .collect();

        Self {
            attributes: sorted_attributes,
            ..self
        }
    }

    /// Functional builder: returns a new element with the specified attribute removed.
    pub fn without_attribute(self, key_to_remove: &str) -> Self {
        let filtered_attributes: Vec<(String, String)> = self
            .attributes
            .into_iter()
            .filter(|(existing_key, _)| existing_key != key_to_remove)
            .collect();

        Self {
            attributes: filtered_attributes,
            ..self
        }
    }

    /// Functional builder: returns a new element with the specified text content.
    pub fn with_replaced_text(self, new_text: impl Into<String>) -> Self {
        Self {
            text_content: Some(new_text.into()),
            ..self
        }
    }

    /// Functional builder: returns a new element with an appended child element.
    pub fn with_child(self, child: DomainXmlElement) -> Self {
        let new_children: Vec<DomainXmlElement> = self
            .children
            .into_iter()
            .chain(std::iter::once(child))
            .collect();

        Self {
            children: new_children,
            ..self
        }
    }

    /// Functional builder: returns a new element with completely replaced children.
    pub fn with_children(self, new_children: Vec<DomainXmlElement>) -> Self {
        Self {
            children: new_children,
            ..self
        }
    }

    /// Functional builder: returns a new element with child tags removed recursively.
    pub fn without_child_tag_recursive(self, tag_to_remove: &str) -> Self {
        let filtered_children: Vec<DomainXmlElement> = self
            .children
            .into_iter()
            .filter(|child| child.tag_name != tag_to_remove)
            .map(|child| child.without_child_tag_recursive(tag_to_remove))
            .collect();

        Self {
            children: filtered_children,
            ..self
        }
    }

    /// Functional builder: maps/filters children using a transformation closure.
    pub fn transform_children<F>(self, transform_fn: F) -> Self
    where
        F: Fn(DomainXmlElement) -> Option<DomainXmlElement>,
    {
        let new_children: Vec<DomainXmlElement> = self
            .children
            .into_iter()
            .filter_map(transform_fn)
            .collect();

        Self {
            children: new_children,
            ..self
        }
    }

    /// Ingests an XML string and parses it into an in-memory immutable `DomainXmlElement` tree.
    pub fn parse(xml_content: &str) -> Result<DomainXmlElement, DomainXmlParseError> {
        let mut reader = Reader::from_str(xml_content);
        reader.config_mut().trim_text(true);

        let mut event_buffer = Vec::new();
        Self::parse_document_root(&mut reader, &mut event_buffer)
    }

    fn parse_document_root(
        reader: &mut Reader<&[u8]>,
        event_buffer: &mut Vec<u8>,
    ) -> Result<DomainXmlElement, DomainXmlParseError> {
        event_buffer.clear();
        match reader.read_event_into(event_buffer) {
            Ok(Event::Start(start_event)) => {
                let owned_start = start_event.into_owned();
                let root = Self::parse_start_tag(owned_start, reader, event_buffer)?;
                Self::verify_eof(reader, event_buffer)?;
                Ok(root)
            }
            Ok(Event::Empty(empty_event)) => {
                let owned_empty = empty_event.into_owned();
                let root = Self::parse_empty_tag(owned_empty)?;
                Self::verify_eof(reader, event_buffer)?;
                Ok(root)
            }
            Ok(Event::Comment(_)) | Ok(Event::Decl(_)) | Ok(Event::DocType(_)) | Ok(Event::PI(_)) => {
                Self::parse_document_root(reader, event_buffer)
            }
            Ok(Event::Eof) => Err(DomainXmlParseError::EmptyContent),
            Err(error) => Err(DomainXmlParseError::XmlParserError {
                message: error.to_string(),
            }),
            _ => Err(DomainXmlParseError::UnexpectedLeadingEvent),
        }
    }

    fn verify_eof(
        reader: &mut Reader<&[u8]>,
        event_buffer: &mut Vec<u8>,
    ) -> Result<(), DomainXmlParseError> {
        event_buffer.clear();
        match reader.read_event_into(event_buffer) {
            Ok(Event::Eof) => Ok(()),
            Ok(Event::Comment(_)) | Ok(Event::PI(_)) => Self::verify_eof(reader, event_buffer),
            Ok(Event::Start(_)) | Ok(Event::Empty(_)) => {
                Err(DomainXmlParseError::MultipleRootElements)
            }
            Ok(Event::Text(text)) => {
                if text.as_ref().iter().all(|byte| byte.is_ascii_whitespace()) {
                    Self::verify_eof(reader, event_buffer)
                } else {
                    Err(DomainXmlParseError::TrailingContent)
                }
            }
            Err(error) => Err(DomainXmlParseError::XmlParserError {
                message: error.to_string(),
            }),
            _ => Self::verify_eof(reader, event_buffer),
        }
    }

    fn parse_start_tag(
        start_event: quick_xml::events::BytesStart<'_>,
        reader: &mut Reader<&[u8]>,
        event_buffer: &mut Vec<u8>,
    ) -> Result<DomainXmlElement, DomainXmlParseError> {
        let tag_name = std::str::from_utf8(start_event.name().as_ref())
            .map_err(|utf8_error| DomainXmlParseError::InvalidUtf8 {
                field_name: "tag name".to_string(),
                details: utf8_error.to_string(),
            })?
            .to_string();

        let attributes = Self::parse_attributes(start_event.attributes())?;
        let (children, text_content) = Self::parse_element_body(&tag_name, reader, event_buffer)?;

        Ok(DomainXmlElement {
            tag_name,
            attributes,
            children,
            text_content,
        })
    }

    fn parse_empty_tag(
        empty_event: quick_xml::events::BytesStart<'_>,
    ) -> Result<DomainXmlElement, DomainXmlParseError> {
        let tag_name = std::str::from_utf8(empty_event.name().as_ref())
            .map_err(|utf8_error| DomainXmlParseError::InvalidUtf8 {
                field_name: "tag name".to_string(),
                details: utf8_error.to_string(),
            })?
            .to_string();

        let attributes = Self::parse_attributes(empty_event.attributes())?;

        Ok(DomainXmlElement {
            tag_name,
            attributes,
            children: Vec::new(),
            text_content: None,
        })
    }

    fn parse_attributes(
        attributes_iter: quick_xml::events::attributes::Attributes<'_>,
    ) -> Result<Vec<(String, String)>, DomainXmlParseError> {
        attributes_iter
            .map(|attribute_result| {
                let attribute = attribute_result.map_err(|attribute_error| {
                    DomainXmlParseError::AttributeParseError {
                        message: attribute_error.to_string(),
                    }
                })?;
                let key = std::str::from_utf8(attribute.key.as_ref())
                    .map_err(|utf8_error| DomainXmlParseError::InvalidUtf8 {
                        field_name: "attribute key".to_string(),
                        details: utf8_error.to_string(),
                    })?
                    .to_string();
                let value = std::str::from_utf8(attribute.value.as_ref())
                    .map_err(|utf8_error| DomainXmlParseError::InvalidUtf8 {
                        field_name: "attribute value".to_string(),
                        details: utf8_error.to_string(),
                    })?
                    .to_string();
                Ok((key, value))
            })
            .collect::<Result<std::collections::BTreeMap<_, _>, DomainXmlParseError>>()
            .map(|sorted_attributes| sorted_attributes.into_iter().collect())
    }

    fn next_body_item(
        expected_tag: &str,
        reader: &mut Reader<&[u8]>,
        event_buffer: &mut Vec<u8>,
    ) -> Option<Result<BodyItem, DomainXmlParseError>> {
        event_buffer.clear();
        match reader.read_event_into(event_buffer) {
            Ok(Event::Start(child_start)) => {
                let owned_start = child_start.into_owned();
                Some(
                    Self::parse_start_tag(owned_start, reader, event_buffer)
                        .map(BodyItem::Child),
                )
            }
            Ok(Event::Empty(child_empty)) => {
                let owned_empty = child_empty.into_owned();
                Some(Self::parse_empty_tag(owned_empty).map(BodyItem::Child))
            }
            Ok(Event::Text(text_event)) => {
                let parsed_text = text_event
                    .unescape()
                    .map_err(|unescape_error| DomainXmlParseError::TextUnescapeError {
                        message: unescape_error.to_string(),
                    })
                    .map(|unescaped_text| unescaped_text.trim().to_string());
                match parsed_text {
                    Ok(text) if !text.is_empty() => Some(Ok(BodyItem::Text(text))),
                    Ok(_) => Self::next_body_item(expected_tag, reader, event_buffer),
                    Err(error) => Some(Err(error)),
                }
            }
            Ok(Event::CData(cdata_event)) => {
                let parsed_cdata = std::str::from_utf8(cdata_event.as_ref())
                    .map_err(|utf8_error| DomainXmlParseError::InvalidUtf8 {
                        field_name: "CDATA".to_string(),
                        details: utf8_error.to_string(),
                    })
                    .map(|raw_cdata| raw_cdata.trim().to_string());
                match parsed_cdata {
                    Ok(text) if !text.is_empty() => Some(Ok(BodyItem::Text(text))),
                    Ok(_) => Self::next_body_item(expected_tag, reader, event_buffer),
                    Err(error) => Some(Err(error)),
                }
            }
            Ok(Event::End(end_event)) => {
                let end_name = end_event.name();
                let parsed_end_tag = std::str::from_utf8(end_name.as_ref())
                    .map_err(|utf8_error| DomainXmlParseError::InvalidUtf8 {
                        field_name: "closing tag".to_string(),
                        details: utf8_error.to_string(),
                    });
                match parsed_end_tag {
                    Ok(end_tag) if end_tag == expected_tag => None,
                    Ok(end_tag) => Some(Err(DomainXmlParseError::MismatchedClosingTag {
                        expected_tag: expected_tag.to_string(),
                        actual_tag: end_tag.to_string(),
                    })),
                    Err(error) => Some(Err(error)),
                }
            }
            Ok(Event::Eof) => Some(Err(DomainXmlParseError::UnclosedTag {
                expected_tag: expected_tag.to_string(),
            })),
            Ok(Event::Comment(_)) | Ok(Event::Decl(_)) | Ok(Event::DocType(_)) | Ok(Event::PI(_)) => {
                Self::next_body_item(expected_tag, reader, event_buffer)
            }
            Err(error) => Some(Err(DomainXmlParseError::XmlParserError {
                message: error.to_string(),
            })),
        }
    }

    fn parse_element_body(
        expected_tag: &str,
        reader: &mut Reader<&[u8]>,
        event_buffer: &mut Vec<u8>,
    ) -> Result<(Vec<DomainXmlElement>, Option<String>), DomainXmlParseError> {
        let items: Result<Vec<BodyItem>, DomainXmlParseError> =
            std::iter::from_fn(|| Self::next_body_item(expected_tag, reader, event_buffer))
                .collect();

        let parsed_items = items?;

        let (children, text_fragments): (Vec<DomainXmlElement>, Vec<String>) = parsed_items
            .into_iter()
            .fold((Vec::new(), Vec::new()), |(mut children_acc, mut text_acc), item| {
                match item {
                    BodyItem::Child(child) => children_acc.push(child),
                    BodyItem::Text(text) => text_acc.push(text),
                }
                (children_acc, text_acc)
            });

        let text_content = Some(text_fragments.concat()).filter(|text| !text.is_empty());

        Ok((children, text_content))
    }

    /// Serializes the element into canonical formatted XML.
    pub fn to_xml_string(&self, indent_level: usize) -> String {
        let indent = "  ".repeat(indent_level);
        let rendered_attributes: String = self
            .attributes
            .iter()
            .map(|(attribute_key, attribute_value)| {
                let escaped_attribute_value = attribute_value
                    .replace('&', "&amp;")
                    .replace('<', "&lt;")
                    .replace('"', "&quot;");
                format!(" {attribute_key}=\"{escaped_attribute_value}\"")
            })
            .collect();

        if self.children.is_empty() && self.text_content.is_none() {
            format!("{indent}<{}{}/>\n", self.tag_name, rendered_attributes)
        } else if self.children.is_empty() {
            let text = self.text_content.as_deref().unwrap_or("");
            let escaped_text = text.replace('&', "&amp;").replace('<', "&lt;");
            format!(
                "{indent}<{}{}>{escaped_text}</{}>\n",
                self.tag_name, rendered_attributes, self.tag_name
            )
        } else {
            let formatted_text = self
                .text_content
                .as_deref()
                .map(|text| {
                    let escaped_text = text.replace('&', "&amp;").replace('<', "&lt;");
                    format!("{indent}  {escaped_text}\n")
                })
                .unwrap_or_default();
            let rendered_children: String = self
                .children
                .iter()
                .map(|child| child.to_xml_string(indent_level + 1))
                .collect();
            format!(
                "{indent}<{}{}>\n{}{rendered_children}{indent}</{}>\n",
                self.tag_name, rendered_attributes, formatted_text, self.tag_name
            )
        }
    }
}

enum BodyItem {
    Child(DomainXmlElement),
    Text(String),
}

/// Enumerates errors that can occur while parsing XML into `DomainXmlElement`.
#[derive(Debug, thiserror::Error, PartialEq, Eq, Clone)]
pub enum DomainXmlParseError {
    #[error("Empty XML content")]
    EmptyContent,

    #[error("XML parser error: {message}")]
    XmlParserError { message: String },

    #[error("Unexpected event before root XML element")]
    UnexpectedLeadingEvent,

    #[error("Multiple root elements encountered in XML")]
    MultipleRootElements,

    #[error("Unexpected content after root element")]
    TrailingContent,

    #[error("Invalid UTF-8 in {field_name}: {details}")]
    InvalidUtf8 { field_name: String, details: String },

    #[error("Failed to parse attribute: {message}")]
    AttributeParseError { message: String },

    #[error("Failed to unescape text content: {message}")]
    TextUnescapeError { message: String },

    #[error("Mismatched closing tag: expected </{expected_tag}>, found </{actual_tag}>")]
    MismatchedClosingTag {
        expected_tag: String,
        actual_tag: String,
    },

    #[error("Unclosed XML tag encountered: <{expected_tag}>")]
    UnclosedTag { expected_tag: String },
}
