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
