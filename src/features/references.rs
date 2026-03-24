//! Find-references feature entry point.
//! This module will return single-document reference locations for entity ids.

use tower_lsp::lsp_types::{Location, Position, Url};

use crate::document::Document;

pub fn find_references(
    uri: &Url,
    document: &Document,
    position: Position,
) -> Option<Vec<Location>> {
    let node = document.node_at_position(position)?;

    let id_node = match node.kind() {
        "instance_id" | "reference" => node,
        _ => return None,
    };

    let text = id_node.utf8_text(document.text.as_bytes()).ok()?;
    let id = text.trim_start_matches('#').parse::<u32>().ok()?;

    let mut locations = Vec::new();

    if let Some(definition) = document.definitions.get(&id) {
        locations.push(Location {
            uri: uri.clone(),
            range: definition.id_range,
        });
    }

    if let Some(refs) = document.references.get(&id) {
        locations.extend(refs.iter().map(|r| Location {
            uri: uri.clone(),
            range: *r,
        }));
    }

    if locations.is_empty() {
        None
    } else {
        Some(locations)
    }
}
