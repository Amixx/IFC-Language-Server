//! Parsed in-memory representation of one IFC document.
//! Owns the source text, syntax tree, and small per-document indexes.

use std::collections::HashMap;

use tower_lsp::lsp_types::{Position, Range};
use tree_sitter::{Node, Parser, Point, Tree, TreeCursor};

use crate::schema::IfcVersion;

#[derive(Debug)]
pub struct Document {
    pub text: String,
    pub tree: Option<Tree>,
    pub version: Option<IfcVersion>,
    pub definitions: HashMap<u32, DefinitionInfo>,
    pub references: HashMap<u32, Vec<Range>>,
    pub instances: Vec<EntityInstanceInfo>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DefinitionInfo {
    pub id_range: Range,
    pub entity_range: Range,
    pub entity_name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EntityInstanceInfo {
    pub id: Option<u32>,
    pub id_range: Option<Range>,
    pub entity_name: String,
    pub entity_name_range: Range,
    pub entity_range: Range,
    pub parameter_list_range: Option<Range>,
    pub parameters: Vec<ParameterValue>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParameterValue {
    Reference {
        id: u32,
        range: Range,
    },
    Enumeration {
        value: String,
        range: Range,
    },
    String {
        range: Range,
    },
    Number {
        text: String,
        range: Range,
    },
    Null {
        range: Range,
    },
    Omitted {
        range: Range,
    },
    List {
        items: Vec<ParameterValue>,
        range: Range,
    },
    Typed {
        type_name: String,
        inner: Vec<ParameterValue>,
        range: Range,
    },
    Unknown {
        range: Range,
    },
}

impl ParameterValue {
    pub fn range(&self) -> Range {
        match self {
            ParameterValue::Reference { range, .. }
            | ParameterValue::Enumeration { range, .. }
            | ParameterValue::String { range }
            | ParameterValue::Number { range, .. }
            | ParameterValue::Null { range }
            | ParameterValue::Omitted { range }
            | ParameterValue::List { range, .. }
            | ParameterValue::Typed { range, .. }
            | ParameterValue::Unknown { range } => *range,
        }
    }

    pub fn kind_name(&self) -> &'static str {
        match self {
            ParameterValue::Reference { .. } => "reference",
            ParameterValue::Enumeration { .. } => "enumeration",
            ParameterValue::String { .. } => "string",
            ParameterValue::Number { .. } => "number",
            ParameterValue::Null { .. } => "null",
            ParameterValue::Omitted { .. } => "omitted value",
            ParameterValue::List { .. } => "aggregate",
            ParameterValue::Typed { .. } => "typed value",
            ParameterValue::Unknown { .. } => "unknown value",
        }
    }
}

impl Document {
    pub fn parse(parser: &mut Parser, text: String) -> Self {
        let tree = parser.parse(&text, None);
        let version = detect_version(&text);
        let (definitions, references, instances) = build_indexes(&tree, &text);
        Self {
            text,
            tree,
            version,
            definitions,
            references,
            instances,
        }
    }

    pub fn node_at_position(&self, position: Position) -> Option<Node<'_>> {
        let tree = self.tree.as_ref()?;
        let point = Point {
            row: position.line as usize,
            column: position.character as usize,
        };

        tree.root_node().descendant_for_point_range(point, point)
    }
}

fn detect_version(text: &str) -> Option<IfcVersion> {
    let schema_start = text.find("FILE_SCHEMA")?;
    let schema_section = &text[schema_start..text.len().min(schema_start + 256)];

    if schema_section.contains("IFC4X3_ADD2") {
        Some(IfcVersion::Ifc4x3Add2)
    } else if schema_section.contains("IFC4") {
        Some(IfcVersion::Ifc4Add2Tc1)
    } else if schema_section.contains("IFC2X3") {
        Some(IfcVersion::Ifc2x3Tc1)
    } else {
        None
    }
}

fn build_indexes(
    tree: &Option<Tree>,
    text: &str,
) -> (
    HashMap<u32, DefinitionInfo>,
    HashMap<u32, Vec<Range>>,
    Vec<EntityInstanceInfo>,
) {
    let mut definitions = HashMap::new();
    let mut references = HashMap::new();
    let mut instances = Vec::new();

    let tree = match tree {
        Some(t) => t,
        None => return (definitions, references, instances),
    };

    let mut cursor = tree.root_node().walk();
    traverse(
        &mut cursor,
        text,
        &mut definitions,
        &mut references,
        &mut instances,
    );

    (definitions, references, instances)
}

fn traverse(
    cursor: &mut TreeCursor,
    text: &str,
    definitions: &mut HashMap<u32, DefinitionInfo>,
    references: &mut HashMap<u32, Vec<Range>>,
    instances: &mut Vec<EntityInstanceInfo>,
) {
    loop {
        let node = cursor.node();

        if node.kind() == "entity_instance" {
            if let Some(instance) = parse_entity_instance(node, text) {
                if let Some(id) = instance.id {
                    definitions.insert(
                        id,
                        DefinitionInfo {
                            id_range: instance
                                .id_range
                                .expect("definition ids should have a range"),
                            entity_range: instance.entity_range,
                            entity_name: instance.entity_name.clone(),
                        },
                    );
                }
                instances.push(instance);
            }
        } else if node.kind() == "reference" {
            if let Ok(ref_text) = node.utf8_text(text.as_bytes()) {
                if let Ok(id) = ref_text.trim_start_matches('#').parse::<u32>() {
                    references.entry(id).or_default().push(node_range(&node));
                }
            }
        }

        if cursor.goto_first_child() {
            traverse(cursor, text, definitions, references, instances);
            cursor.goto_parent();
        }

        if !cursor.goto_next_sibling() {
            break;
        }
    }
}

fn parse_entity_instance(node: Node<'_>, text: &str) -> Option<EntityInstanceInfo> {
    let mut child_cursor = node.walk();
    let mut id = None;
    let mut id_range = None;
    let mut entity_name = None;
    let mut entity_name_range = None;
    let mut parameter_list_range = None;
    let mut parameters = Vec::new();

    for child in node.children(&mut child_cursor) {
        match child.kind() {
            "instance_id" => {
                let id_text = child.utf8_text(text.as_bytes()).ok()?;
                id = id_text.trim_start_matches('#').parse::<u32>().ok();
                id_range = Some(node_range(&child));
            }
            "entity_name" => {
                let name = child.utf8_text(text.as_bytes()).ok()?;
                entity_name = Some(name.to_ascii_uppercase());
                entity_name_range = Some(node_range(&child));
            }
            "parameter_list" => {
                parameter_list_range = Some(node_range(&child));
                parameters = parse_parameter_list(child, text);
            }
            _ => {}
        }
    }

    Some(EntityInstanceInfo {
        id,
        id_range,
        entity_name: entity_name?,
        entity_name_range: entity_name_range?,
        entity_range: node_range(&node),
        parameter_list_range,
        parameters,
    })
}

fn parse_parameter_list(node: Node<'_>, text: &str) -> Vec<ParameterValue> {
    let mut cursor = node.walk();
    let Some(sequence) = node
        .children(&mut cursor)
        .find(|child| child.kind() == "parameter_sequence")
    else {
        return Vec::new();
    };

    let mut seq_cursor = sequence.walk();
    sequence
        .children(&mut seq_cursor)
        .filter(|child| child.kind() == "parameter")
        .map(|parameter| parse_parameter(parameter, text))
        .collect()
}

fn parse_parameter(node: Node<'_>, text: &str) -> ParameterValue {
    let mut cursor = node.walk();
    let Some(value_node) = node.children(&mut cursor).find(|child| child.is_named()) else {
        return ParameterValue::Unknown {
            range: node_range(&node),
        };
    };

    parse_parameter_value(value_node, text)
}

fn parse_parameter_value(node: Node<'_>, text: &str) -> ParameterValue {
    match node.kind() {
        "reference" => {
            let range = node_range(&node);
            let id = node
                .utf8_text(text.as_bytes())
                .ok()
                .and_then(|value| value.trim_start_matches('#').parse::<u32>().ok());
            match id {
                Some(id) => ParameterValue::Reference { id, range },
                None => ParameterValue::Unknown { range },
            }
        }
        "enumeration" => {
            let range = node_range(&node);
            let value = node
                .utf8_text(text.as_bytes())
                .ok()
                .map(|value| value.trim_matches('.').to_ascii_uppercase())
                .unwrap_or_default();
            ParameterValue::Enumeration { value, range }
        }
        "string" => ParameterValue::String {
            range: node_range(&node),
        },
        "number" => ParameterValue::Number {
            text: node
                .utf8_text(text.as_bytes())
                .unwrap_or_default()
                .to_string(),
            range: node_range(&node),
        },
        "null_value" => ParameterValue::Null {
            range: node_range(&node),
        },
        "omitted_value" => ParameterValue::Omitted {
            range: node_range(&node),
        },
        "list" => {
            let range = node_range(&node);
            let mut cursor = node.walk();
            let items = node
                .children(&mut cursor)
                .find(|child| child.kind() == "parameter_sequence")
                .map(|sequence| {
                    let mut seq_cursor = sequence.walk();
                    sequence
                        .children(&mut seq_cursor)
                        .filter(|child| child.kind() == "parameter")
                        .map(|parameter| parse_parameter(parameter, text))
                        .collect()
                })
                .unwrap_or_default();
            ParameterValue::List { items, range }
        }
        "typed_parameter" => {
            let range = node_range(&node);
            let mut cursor = node.walk();
            let mut type_name = None;
            let mut inner = Vec::new();

            for child in node.children(&mut cursor) {
                match child.kind() {
                    "entity_name" => {
                        type_name = child
                            .utf8_text(text.as_bytes())
                            .ok()
                            .map(|name| name.to_ascii_uppercase());
                    }
                    "parameter_list" => inner = parse_parameter_list(child, text),
                    _ => {}
                }
            }

            ParameterValue::Typed {
                type_name: type_name.unwrap_or_default(),
                inner,
                range,
            }
        }
        _ => ParameterValue::Unknown {
            range: node_range(&node),
        },
    }
}

fn node_range(node: &Node<'_>) -> Range {
    let start = node.start_position();
    let end = node.end_position();

    Range {
        start: Position::new(start.row as u32, start.column as u32),
        end: Position::new(end.row as u32, end.column as u32),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_document(text: &str) -> Document {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_ifc::LANGUAGE.into())
            .expect("Error loading IFC parser");

        Document::parse(&mut parser, text.to_string())
    }

    fn position_at(text: &str, needle: &str) -> Position {
        let offset = text.find(needle).expect("needle should exist") as u32;
        Position::new(0, offset)
    }

    #[test]
    fn parse_keeps_text_and_initial_state() {
        let text = "#1=IFCWALL($);";
        let document = parse_document(text);
        assert_eq!(document.text, text);
        assert!(document.tree.is_some());
        assert_eq!(document.version, None);
        // definitions and references are now populated, not empty
    }

    #[test]
    fn node_at_position_finds_entity_name() {
        let text = "#1=IFCWALL($);";
        let document = parse_document(text);

        let node = document
            .node_at_position(position_at(text, "IFCWALL"))
            .expect("entity_name node should exist");

        assert_eq!(node.kind(), "entity_name");
        assert_eq!(
            node.utf8_text(document.text.as_bytes()).ok(),
            Some("IFCWALL")
        );
    }

    #[test]
    fn node_at_position_finds_reference() {
        let text = "#1=IFCWALL(#2);";
        let document = parse_document(text);

        let node = document
            .node_at_position(position_at(text, "#2"))
            .expect("reference node should exist");

        assert_eq!(node.kind(), "reference");
        assert_eq!(node.utf8_text(document.text.as_bytes()).ok(), Some("#2"));
    }

    #[test]
    fn parse_indexes_entity_definition() {
        let text = "#1=IFCWALL($);";
        let document = parse_document(text);
        assert!(document.definitions.contains_key(&1));
        assert!(document.references.is_empty());
        assert_eq!(
            document.definitions[&1].id_range,
            Range {
                start: Position::new(0, 0),
                end: Position::new(0, 2),
            }
        );
        assert_eq!(
            document.definitions[&1].entity_range,
            Range {
                start: Position::new(0, 0),
                end: Position::new(0, text.len() as u32),
            }
        );
    }

    #[test]
    fn parse_indexes_multiple_definitions() {
        let text = "#1=IFCWALL($);\n#2=IFCDOOR($);";
        let document = parse_document(text);
        assert!(document.definitions.contains_key(&1));
        assert!(document.definitions.contains_key(&2));
        assert_eq!(document.definitions.len(), 2);
    }

    #[test]
    fn parse_indexes_references() {
        let text = "#1=IFCWALL(#2);";
        let document = parse_document(text);
        assert!(document.references.contains_key(&2));
        assert_eq!(document.references[&2].len(), 1);
    }

    #[test]
    fn parse_indexes_multiple_references_to_same_id() {
        let text = "#1=IFCWALL(#2);\n#3=IFCDOOR(#2);";
        let document = parse_document(text);
        assert_eq!(document.references[&2].len(), 2);
    }
}
