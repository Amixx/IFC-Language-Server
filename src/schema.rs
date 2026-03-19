//! Schema documentation types and lookup interface.
//! This stays local and synchronous so hover remains predictable.

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum IfcVersion {
    Ifc2x3Tc1,
    Ifc4Add2Tc1,
    Ifc4x3Add2,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct EntityAttributeDoc {
    pub name: String,
    pub type_name: String,
    pub description: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct EntityDoc {
    pub summary: String,
    pub attributes: Vec<EntityAttributeDoc>,
    pub url: Option<String>,
}

#[derive(Default)]
pub struct SchemaDocs;

impl SchemaDocs {
    pub fn new() -> Self {
        Self
    }

    pub fn get_entity_doc(&self, _version: IfcVersion, _entity_name: &str) -> Option<EntityDoc> {
        None
    }
}
