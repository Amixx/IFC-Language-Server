//! Hover feature logic.
//! Resolves the syntax node under the cursor and renders hover content.

use tower_lsp::lsp_types::{Hover, HoverContents, MarkupContent, MarkupKind, Position};

use crate::document::Document;
use crate::schema::SchemaDocs;

pub fn hover(document: &Document, position: Position, schema_docs: &SchemaDocs) -> Option<Hover> {
    document.tree.as_ref()?;

    if let Some(node) = document.node_at_position(position) {
        if node.kind() == "entity_name" {
            let entity_text = node.utf8_text(document.text.as_bytes()).ok()?;
            if let Some(version) = document.version {
                let _ = schema_docs.get_entity_doc(version, entity_text);
            }
            let hover_text = format!(
                "**{}**\n\n\
                This is an IFC entity.\n\n\
                *Documentation coming soon!*\n\n\
                (Node type: `{}`)",
                entity_text,
                node.kind()
            );

            return Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: hover_text,
                }),
                range: None,
            });
        }
    }

    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: "Hover over an IFC entity name (like `IFCWALL`) to see documentation."
                .to_string(),
        }),
        range: None,
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn parse_document(text: &str) -> Document {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_ifc::LANGUAGE.into())
            .expect("Error loading IFC parser");

        Document::parse(&mut parser, text.to_string())
    }

    fn position_at(text: &str, needle: &str) -> Position {
        let offset = text.find(needle).expect("needle should exist") as u32;
        Position::new(0, offset)
    }

    fn hover_text(hover: Hover) -> String {
        match hover.contents {
            HoverContents::Markup(markup) => markup.value,
            contents => panic!("unexpected hover contents: {contents:?}"),
        }
    }

    #[test]
    fn hover_returns_entity_placeholder_for_entity_names() {
        let text = "#1=IFCWALL($);";
        let document = parse_document(text);

        let hover = hover(&document, position_at(text, "IFCWALL"), &SchemaDocs::new())
            .expect("hover should exist");
        let value = hover_text(hover);

        assert!(value.contains("**IFCWALL**"));
        assert!(value.contains("Documentation coming soon!"));
    }

    #[test]
    fn hover_returns_generic_message_for_non_entity_name_nodes() {
        let text = "#1=IFCWALL($);";
        let document = parse_document(text);

        let hover =
            hover(&document, position_at(text, "#1"), &SchemaDocs::new()).expect("hover exists");
        let value = hover_text(hover);

        assert!(value.contains("Hover over an IFC entity name"));
    }

    #[test]
    fn hover_returns_none_without_a_syntax_tree() {
        let document = Document {
            text: "#1=IFCWALL($);".to_string(),
            tree: None,
            version: None,
            definitions: HashMap::new(),
            references: HashMap::new(),
        };

        assert!(hover(&document, Position::new(0, 0), &SchemaDocs::new()).is_none());
    }
}
