//! Spatial tree feature for IFC model navigation.
//! It builds relationships from parsed entity instances and returns one lazy
//! tree level per request so clients can render large models incrementally.

use std::collections::{HashMap, HashSet};

use serde::Serialize;
use tower_lsp::lsp_types::Range;

use crate::document::{Document, EntityInstanceInfo, ParameterValue};

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SpatialTreeNode {
    pub id: u32,
    pub entity_name: String,
    pub label: String,
    pub description: Option<String>,
    pub range: Range,
    pub has_children: bool,
    pub child_count: usize,
}

#[derive(Default)]
struct SpatialRelations {
    children_by_parent: HashMap<u32, Vec<u32>>,
    child_ids: HashSet<u32>,
}

pub fn children(document: &Document, parent_id: Option<u32>) -> Vec<SpatialTreeNode> {
    let relations = build_spatial_relations(document);
    let ids = match parent_id {
        Some(id) => relations
            .children_by_parent
            .get(&id)
            .cloned()
            .unwrap_or_default(),
        None => root_ids(document, &relations),
    };

    ids.into_iter()
        .filter_map(|id| {
            let instance = document.instance_by_id(id)?;
            let child_count = relations
                .children_by_parent
                .get(&id)
                .map(Vec::len)
                .unwrap_or_default();

            Some(SpatialTreeNode {
                id,
                entity_name: instance.entity_name.clone(),
                label: instance_label(document, instance),
                description: Some(format!("#{}", id)),
                range: instance.entity_range,
                has_children: child_count > 0,
                child_count,
            })
        })
        .collect()
}

fn build_spatial_relations(document: &Document) -> SpatialRelations {
    let mut relations = SpatialRelations::default();

    for instance in &document.instances {
        match instance.entity_name.as_str() {
            "IFCRELAGGREGATES" => {
                let Some(parent_id) = reference_parameter(instance, 4) else {
                    continue;
                };
                for child_id in list_reference_parameter(instance, 5) {
                    relations.add_child(parent_id, child_id);
                }
            }
            "IFCRELCONTAINEDINSPATIALSTRUCTURE" => {
                let Some(parent_id) = reference_parameter(instance, 5) else {
                    continue;
                };
                for child_id in list_reference_parameter(instance, 4) {
                    relations.add_child(parent_id, child_id);
                }
            }
            _ => {}
        }
    }

    relations
}

impl SpatialRelations {
    fn add_child(&mut self, parent_id: u32, child_id: u32) {
        let children = self.children_by_parent.entry(parent_id).or_default();
        if !children.contains(&child_id) {
            children.push(child_id);
            self.child_ids.insert(child_id);
        }
    }
}

fn root_ids(document: &Document, relations: &SpatialRelations) -> Vec<u32> {
    let projects: Vec<u32> = document
        .instances
        .iter()
        .filter(|instance| instance.entity_name == "IFCPROJECT")
        .filter_map(|instance| instance.id)
        .collect();

    if !projects.is_empty() {
        return projects;
    }

    document
        .instances
        .iter()
        .filter(|instance| {
            is_spatial_structure(&instance.entity_name)
                && instance
                    .id
                    .map(|id| !relations.child_ids.contains(&id))
                    .unwrap_or(false)
        })
        .filter_map(|instance| instance.id)
        .collect()
}

fn is_spatial_structure(entity_name: &str) -> bool {
    matches!(
        entity_name,
        "IFCPROJECT" | "IFCSITE" | "IFCBUILDING" | "IFCBUILDINGSTOREY" | "IFCSPACE"
    )
}

fn reference_parameter(instance: &EntityInstanceInfo, index: usize) -> Option<u32> {
    match instance.parameters.get(index)? {
        ParameterValue::Reference { id, .. } => Some(*id),
        _ => None,
    }
}

fn list_reference_parameter(instance: &EntityInstanceInfo, index: usize) -> Vec<u32> {
    match instance.parameters.get(index) {
        Some(ParameterValue::List { items, .. }) => items
            .iter()
            .filter_map(|item| match item {
                ParameterValue::Reference { id, .. } => Some(*id),
                _ => None,
            })
            .collect(),
        Some(ParameterValue::Reference { id, .. }) => vec![*id],
        _ => Vec::new(),
    }
}

fn instance_label(document: &Document, instance: &EntityInstanceInfo) -> String {
    let id = instance.id.map(|id| format!("#{}", id)).unwrap_or_default();
    let name = instance
        .parameters
        .get(2)
        .and_then(|value| parameter_string(document, value));

    match name {
        Some(name) if !name.is_empty() => format!("{} {} {}", instance.entity_name, id, name),
        _ if !id.is_empty() => format!("{} {}", instance.entity_name, id),
        _ => instance.entity_name.clone(),
    }
}

fn parameter_string(document: &Document, value: &ParameterValue) -> Option<String> {
    match value {
        ParameterValue::String { range } => text_for_range(&document.text, *range)
            .map(|text| text.trim_matches('\'').replace("''", "'")),
        _ => None,
    }
}

fn text_for_range(text: &str, range: Range) -> Option<String> {
    if range.start.line != range.end.line {
        return None;
    }

    let line = text.lines().nth(range.start.line as usize)?;
    let start = range.start.character as usize;
    let end = range.end.character as usize;
    if start > end || end > line.len() {
        return None;
    }

    Some(line[start..end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::Document;
    use tree_sitter::Parser;

    fn parse_document(text: &str) -> Document {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_ifc::LANGUAGE.into())
            .expect("Error loading IFC parser");

        Document::parse(&mut parser, text.to_string())
    }

    #[test]
    fn roots_are_ifc_projects() {
        let document = parse_document("#1=IFCPROJECT('g',$,'Project',$,$,$,$,$,$);");

        let roots = children(&document, None);

        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].id, 1);
        assert_eq!(roots[0].label, "IFCPROJECT #1 Project");
    }

    #[test]
    fn aggregate_children_are_returned_lazily() {
        let document = parse_document(
            "#1=IFCPROJECT('g',$,'Project',$,$,$,$,$,$);\n\
             #2=IFCSITE('s',$,'Site',$,$,$,$,$,$,$,$,$,$,$);\n\
             #3=IFCBUILDING('b',$,'Building',$,$,$,$,$,$,$,$,$);\n\
             #10=IFCRELAGGREGATES('r',$,$,$,#1,(#2,#3));",
        );

        let roots = children(&document, None);
        let children = children(&document, Some(roots[0].id));

        assert_eq!(
            children.iter().map(|node| node.id).collect::<Vec<_>>(),
            vec![2, 3]
        );
    }

    #[test]
    fn containment_children_are_returned_under_structure() {
        let document = parse_document(
            "#1=IFCBUILDINGSTOREY('s',$,'Level 1',$,$,$,$,$,$);\n\
             #2=IFCWALL('w',$,'Wall A',$,$,$,$,$);\n\
             #3=IFCDOOR('d',$,'Door A',$,$,$,$,$,$);\n\
             #10=IFCRELCONTAINEDINSPATIALSTRUCTURE('r',$,$,$,(#2,#3),#1);",
        );

        let roots = children(&document, None);
        let children = children(&document, Some(roots[0].id));

        assert_eq!(roots[0].id, 1);
        assert_eq!(
            children.iter().map(|node| node.id).collect::<Vec<_>>(),
            vec![2, 3]
        );
    }
}
