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
    pub definitions: HashMap<u32, Range>,
    pub references: HashMap<u32, Vec<Range>>,
}

impl Document {
    pub fn parse(parser: &mut Parser, text: String) -> Self {
        let tree = parser.parse(&text, None);
        let version = detect_version(&text);
        let (definitions, references) = build_indexes(&tree, &text);
        Self {
            text,
            tree,
            version,
            definitions: definitions,
            references: references,
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
) -> (HashMap<u32, Range>, HashMap<u32, Vec<Range>>) {
    let mut definitions = HashMap::new();
    let mut references = HashMap::new();

    let tree = match tree {
        Some(t) => t,
        None => return (definitions, references),
    };

    let mut cursor = tree.root_node().walk();
    traverse(&mut cursor, text, &mut definitions, &mut references);

    (definitions, references)
}

fn traverse(
    cursor: &mut TreeCursor,
    text: &str,
    definitions: &mut HashMap<u32, Range>,
    references: &mut HashMap<u32, Vec<Range>>,
) {
    loop {
        let node = cursor.node();

        if node.kind() == "entity_instance" {
            if let Some(id_node) = node
                .children(&mut cursor.clone())
                .find(|c| c.kind() == "instance_id")
            {
                if let Ok(id_text) = id_node.utf8_text(text.as_bytes()) {
                    if let Ok(id) = id_text.trim_start_matches('#').parse::<u32>() {
                        let start = id_node.start_position();
                        let end = id_node.end_position();
                        definitions.insert(
                            id,
                            Range {
                                start: Position::new(start.row as u32, start.column as u32),
                                end: Position::new(end.row as u32, end.column as u32),
                            },
                        );
                    }
                }
            }
        } else if node.kind() == "reference" {
            if let Ok(ref_text) = node.utf8_text(text.as_bytes()) {
                if let Ok(id) = ref_text.trim_start_matches('#').parse::<u32>() {
                    let start = node.start_position();
                    let end = node.end_position();
                    references.entry(id).or_insert_with(Vec::new).push(Range {
                        start: Position::new(start.row as u32, start.column as u32),
                        end: Position::new(end.row as u32, end.column as u32),
                    });
                }
            }
        }

        if cursor.goto_first_child() {
            traverse(cursor, text, definitions, references);
            cursor.goto_parent();
        }

        if !cursor.goto_next_sibling() {
            break;
        }
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
