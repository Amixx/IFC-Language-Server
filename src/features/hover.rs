//! Hover feature logic.
//! Resolves the syntax node under the cursor and renders hover content.

use tower_lsp::lsp_types::{Hover, HoverContents, MarkupContent, MarkupKind, Position};

use crate::document::Document;
use crate::schema::{EntityDoc, SchemaDocs};

pub fn hover(document: &Document, position: Position, schema_docs: &SchemaDocs) -> Option<Hover> {
    document.tree.as_ref()?;

    if let Some(node) = document.node_at_position(position) {
        if node.kind() == "entity_name" {
            let entity_text = node.utf8_text(document.text.as_bytes()).ok()?;
            if let Some(version) = document.version
                && let Some(entity_doc) = schema_docs.get_entity_doc(version, entity_text)
            {
                return Some(Hover {
                    contents: HoverContents::Markup(MarkupContent {
                        kind: MarkupKind::Markdown,
                        value: render_entity_hover(entity_doc),
                    }),
                    range: None,
                });
            }
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

fn render_entity_hover(entity_doc: &EntityDoc) -> String {
    let mut markdown = format!("**{}**\n\n{}", entity_doc.name, entity_doc.summary);

    if !entity_doc.attributes.is_empty() {
        markdown.push_str("\n\n| Attribute | Type | Description |\n| --- | --- | --- |");
        for attribute in &entity_doc.attributes {
            markdown.push_str(&format!(
                "\n| {} | {} | {} |",
                escape_table_cell(&attribute.name),
                escape_table_cell(&attribute.type_name),
                escape_table_cell(&attribute.description),
            ));
        }
    }

    markdown.push_str(&format!("\n\n[Official documentation]({})", entity_doc.url));

    markdown
}

fn escape_table_cell(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace('|', "\\|")
}

#[cfg(test)]
mod tests {
    use tower_lsp::lsp_types::Position;
    use tree_sitter::Parser;

    use super::*;

    #[test]
    fn returns_ifc4x3_entity_hover_from_schema_docs() {
        let text = include_str!("../../samples/Building-Architecture-IFC4x3.ifc").to_string();
        let offset = text.find("IFCWALL").expect("sample should contain IFCWALL");
        let position = position_at_offset(&text, offset);

        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_ifc::LANGUAGE.into())
            .expect("Error loading IFC parser");

        let document = Document::parse(&mut parser, text);
        let schema_docs = SchemaDocs::new();
        let hover = hover(&document, position, &schema_docs).expect("expected hover");
        let value = match hover.contents {
            HoverContents::Markup(markup) => markup.value,
            _ => panic!("expected markdown hover"),
        };

        assert!(value.contains("IfcWall"));
        assert!(value.contains("Official documentation"));
    }

    fn position_at_offset(text: &str, offset: usize) -> Position {
        let prefix = &text[..offset];
        let line = prefix.bytes().filter(|byte| *byte == b'\n').count() as u32;
        let character = prefix
            .rsplit_once('\n')
            .map(|(_, tail)| tail.chars().count())
            .unwrap_or(prefix.chars().count()) as u32;

        Position { line, character }
    }
}
