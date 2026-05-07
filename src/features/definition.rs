//! Go-to-definition feature entry point.
//! Resolves local `#id` references to their defining entity instances in the same document.
//! Cross-file lookup and schema-level symbol navigation are intentionally not handled here.

use crate::document::Document;
use tower_lsp::lsp_types::{GotoDefinitionResponse, Location, Position, Url};

pub fn goto_definition(
    uri: &Url,
    document: &Document,
    position: Position,
) -> Option<GotoDefinitionResponse> {
    let node = document.node_at_position(position)?;

    let id = if node.kind() == "reference" {
        let text = node.utf8_text(document.text.as_bytes()).ok()?;
        text.trim_start_matches('#').parse::<u32>().ok()?
    } else {
        return None;
    };

    let definition = document.definitions.get(&id)?;

    Some(GotoDefinitionResponse::Scalar(Location {
        uri: uri.clone(),
        range: definition.id_range,
    }))
}
