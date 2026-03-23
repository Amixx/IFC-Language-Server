//! Parsed in-memory representation of one IFC document.
//! Owns the source text, syntax tree, and small per-document indexes.

use std::collections::HashMap;

use tower_lsp::lsp_types::{Position, Range};
use tree_sitter::{Node, Parser, Point, Tree};

use crate::schema::IfcVersion;

#[derive(Debug)]
pub struct Document {
    pub text: String,
    pub tree: Option<Tree>,
    pub version: Option<IfcVersion>,
    pub definitions: HashMap<String, Range>,
    pub references: HashMap<String, Vec<Range>>,
}

impl Document {
    pub fn parse(parser: &mut Parser, text: String) -> Self {
        let tree = parser.parse(&text, None);

        Self {
            text,
            tree,
            version: None,
            definitions: HashMap::new(),
            references: HashMap::new(),
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
        assert!(document.definitions.is_empty());
        assert!(document.references.is_empty());
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
}
