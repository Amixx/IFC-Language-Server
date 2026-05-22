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
    let (id, offset) = document.id_token_at_position(position)?;
    if document.definitions.get(&id) == Some(&offset) {
        return None;
    }

    Some(GotoDefinitionResponse::Scalar(Location {
        uri: uri.clone(),
        range: document.definition_range(id)?,
    }))
}
