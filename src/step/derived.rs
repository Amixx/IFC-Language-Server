//! Resolution of STEP `*` values for the currently supported IFC derived-attribute cases.

use crate::document::{Document, EntityInstanceInfo, ParameterValue};
use crate::schema::{EntityAttributeDoc, EntityDoc, SchemaDoc};

pub struct ResolvedDerivedValue {
    pub attribute_name: String,
    pub value: Option<String>,
    pub preview: Option<String>,
    pub resolution_note: Option<String>,
}

pub fn resolve_omitted_value(
    document: &Document,
    schema: &SchemaDoc,
    instance_id: u32,
    entity_name: &str,
    parameter_index: usize,
) -> Option<ResolvedDerivedValue> {
    let instance = document.instance_by_id(instance_id)?;
    let entity_doc = schema.entity(entity_name)?;
    let attribute = entity_doc.attributes.get(parameter_index)?;
    if !attribute.allows_omitted {
        return None;
    }

    let resolution = resolve_derived_attribute(document, schema, instance, entity_doc, attribute);
    Some(ResolvedDerivedValue {
        attribute_name: attribute.name.clone(),
        value: resolution.as_ref().map(|value| value.value.clone()),
        preview: resolution.as_ref().and_then(|value| value.preview.clone()),
        resolution_note: resolution.and_then(|value| value.resolution_note),
    })
}

fn resolve_derived_attribute(
    document: &Document,
    schema: &SchemaDoc,
    instance: &EntityInstanceInfo,
    entity_doc: &EntityDoc,
    attribute: &EntityAttributeDoc,
) -> Option<ResolvedParameterValue> {
    match (entity_doc.name.as_str(), attribute.name.as_str()) {
        ("IfcSIUnit", "Dimensions") => resolve_ifc_si_unit_dimensions(instance, entity_doc),
        ("IfcGeometricRepresentationSubContext", _) => {
            resolve_subcontext_parent_value(document, schema, instance, attribute)
        }
        _ => None,
    }
}

fn resolve_ifc_si_unit_dimensions(
    instance: &EntityInstanceInfo,
    entity_doc: &EntityDoc,
) -> Option<ResolvedParameterValue> {
    let unit_name = instance
        .parameters
        .iter()
        .zip(&entity_doc.attributes)
        .find_map(
            |(value, attribute)| match (attribute.name.as_str(), value) {
                ("Name", ParameterValue::Enumeration { value, .. }) => Some(value.as_str()),
                _ => None,
            },
        )?;
    let dimensions = ifc_dimensions_for_si_unit(unit_name)?;

    Some(ResolvedParameterValue {
        value: format!(
            "`IfcDimensionalExponents({}, {}, {}, {}, {}, {}, {})`",
            dimensions[0],
            dimensions[1],
            dimensions[2],
            dimensions[3],
            dimensions[4],
            dimensions[5],
            dimensions[6],
        ),
        preview: None,
        resolution_note: Some("resolved from `Name`".to_string()),
    })
}

fn resolve_subcontext_parent_value(
    document: &Document,
    schema: &SchemaDoc,
    instance: &EntityInstanceInfo,
    attribute: &EntityAttributeDoc,
) -> Option<ResolvedParameterValue> {
    let parent_context_id = instance
        .parameters
        .iter()
        .zip(schema.entity(&instance.entity_name)?.attributes.iter())
        .find_map(|(value, attr)| match (attr.name.as_str(), value) {
            ("ParentContext", ParameterValue::Reference { id, .. }) => Some(*id),
            _ => None,
        })?;
    let parent_instance = document.instance_by_id(parent_context_id)?;
    let parent_entity = schema.entity(&parent_instance.entity_name)?;
    let parent_index = parent_entity
        .attributes
        .iter()
        .position(|candidate| candidate.name == attribute.name)?;
    let parent_value = parent_instance.parameters.get(parent_index)?;

    let (value, preview) = render_parameter_resolution(document, parent_value)?;

    Some(ResolvedParameterValue {
        value,
        preview,
        resolution_note: Some("resolved from `ParentContext`".to_string()),
    })
}

fn render_parameter_resolution(
    document: &Document,
    value: &ParameterValue,
) -> Option<(String, Option<String>)> {
    match value {
        ParameterValue::Reference { id, .. } => Some((
            format!("`#{}`", id),
            Some(format!(
                "```ifc\n{}\n```",
                document.entity_instance_text_at_definition(*id)?.trim()
            )),
        )),
        ParameterValue::Enumeration { value, .. } => Some((format!("`.{}.`", value), None)),
        ParameterValue::Number { text, .. } => Some((format!("`{}`", text), None)),
        ParameterValue::Null { .. } => Some(("`$`".to_string(), None)),
        ParameterValue::String { .. } => Some(("string value".to_string(), None)),
        ParameterValue::Omitted { .. } => Some(("`*`".to_string(), None)),
        ParameterValue::List { .. } => Some(("aggregate value".to_string(), None)),
        ParameterValue::Typed { type_name, .. } => {
            Some((format!("typed value `{}`", type_name), None))
        }
        ParameterValue::Unknown { .. } => None,
    }
}

struct ResolvedParameterValue {
    value: String,
    preview: Option<String>,
    resolution_note: Option<String>,
}

fn ifc_dimensions_for_si_unit(unit_name: &str) -> Option<[i32; 7]> {
    Some(match unit_name {
        "METRE" => [1, 0, 0, 0, 0, 0, 0],
        "SQUARE_METRE" => [2, 0, 0, 0, 0, 0, 0],
        "CUBIC_METRE" => [3, 0, 0, 0, 0, 0, 0],
        "GRAM" => [0, 1, 0, 0, 0, 0, 0],
        "SECOND" => [0, 0, 1, 0, 0, 0, 0],
        "AMPERE" => [0, 0, 0, 1, 0, 0, 0],
        "KELVIN" => [0, 0, 0, 0, 1, 0, 0],
        "MOLE" => [0, 0, 0, 0, 0, 1, 0],
        "CANDELA" => [0, 0, 0, 0, 0, 0, 1],
        "RADIAN" => [0, 0, 0, 0, 0, 0, 0],
        "STERADIAN" => [0, 0, 0, 0, 0, 0, 0],
        "HERTZ" => [0, 0, -1, 0, 0, 0, 0],
        "NEWTON" => [1, 1, -2, 0, 0, 0, 0],
        "PASCAL" => [-1, 1, -2, 0, 0, 0, 0],
        "JOULE" => [2, 1, -2, 0, 0, 0, 0],
        "WATT" => [2, 1, -3, 0, 0, 0, 0],
        "COULOMB" => [0, 0, 1, 1, 0, 0, 0],
        "VOLT" => [2, 1, -3, -1, 0, 0, 0],
        "FARAD" => [-2, -1, 4, 2, 0, 0, 0],
        "OHM" => [2, 1, -3, -2, 0, 0, 0],
        "SIEMENS" => [-2, -1, 3, 2, 0, 0, 0],
        "WEBER" => [2, 1, -2, -1, 0, 0, 0],
        "TESLA" => [0, 1, -2, -1, 0, 0, 0],
        "HENRY" => [2, 1, -2, -2, 0, 0, 0],
        "DEGREE_CELSIUS" => [0, 0, 0, 0, 1, 0, 0],
        "LUMEN" => [0, 0, 0, 0, 0, 0, 1],
        "LUX" => [-2, 0, 0, 0, 0, 0, 1],
        "BECQUEREL" => [0, 0, -1, 0, 0, 0, 0],
        "GRAY" => [2, 0, -2, 0, 0, 0, 0],
        "SIEVERT" => [2, 0, -2, 0, 0, 0, 0],
        _ => return None,
    })
}
