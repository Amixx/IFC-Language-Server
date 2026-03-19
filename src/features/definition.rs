//! Go-to-definition feature entry point.
//! This module will resolve entity id references to local definitions.

use tower_lsp::lsp_types::{GotoDefinitionResponse, Position, Url};

use crate::document::Document;

pub fn goto_definition(
    _uri: &Url,
    document: &Document,
    _position: Position,
) -> Option<GotoDefinitionResponse> {
    let _ = document.definitions.len();
    None
}
