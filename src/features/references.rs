//! Find-references feature entry point.
//! This module will return single-document reference locations for entity ids.

use tower_lsp::lsp_types::{Location, Position, Url};

use crate::document::Document;

pub fn find_references(
    _uri: &Url,
    document: &Document,
    _position: Position,
) -> Option<Vec<Location>> {
    let _ = document.references.len();
    None
}
