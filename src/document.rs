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
        let version = detect_version(&text);

        Self {
            text,
            tree,
            version,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_ifc4x3_version_from_file_schema() {
        let text = "FILE_SCHEMA(('IFC4X3_ADD2'));";
        assert_eq!(detect_version(text), Some(IfcVersion::Ifc4x3Add2));
    }

    #[test]
    fn detects_ifc4_version_from_file_schema() {
        let text = "FILE_SCHEMA(('IFC4'));";
        assert_eq!(detect_version(text), Some(IfcVersion::Ifc4Add2Tc1));
    }

    #[test]
    fn detects_ifc2x3_version_from_file_schema() {
        let text = "FILE_SCHEMA(('IFC2X3'));";
        assert_eq!(detect_version(text), Some(IfcVersion::Ifc2x3Tc1));
    }
}
