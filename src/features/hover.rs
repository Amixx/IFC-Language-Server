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
