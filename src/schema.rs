//! Schema documentation types and lookup interface.
//! This stays local and synchronous so hover remains predictable.

use std::collections::HashMap;

use serde::Deserialize;

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum IfcVersion {
    Ifc2x3Tc1,
    Ifc4Add2Tc1,
    Ifc4x3Add2,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EntityAttributeDoc {
    pub name: String,
    pub type_name: String,
    pub description: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EntityDoc {
    pub name: String,
    pub summary: String,
    pub attributes: Vec<EntityAttributeDoc>,
    pub url: String,
}

#[derive(Default)]
pub struct SchemaDocs {
    docs: HashMap<IfcVersion, HashMap<String, EntityDoc>>,
}

impl SchemaDocs {
    pub fn new() -> Self {
        let mut docs = HashMap::new();
        docs.insert(
            IfcVersion::Ifc2x3Tc1,
            load_docs(include_str!("../data/schema-docs/ifc2x3_tc1.json")),
        );
        docs.insert(
            IfcVersion::Ifc4Add2Tc1,
            load_docs(include_str!("../data/schema-docs/ifc4_add2_tc1.json")),
        );
        docs.insert(
            IfcVersion::Ifc4x3Add2,
            load_docs(include_str!("../data/schema-docs/ifc4x3_add2.json")),
        );

        Self { docs }
    }

    pub fn get_entity_doc(&self, version: IfcVersion, entity_name: &str) -> Option<&EntityDoc> {
        let key = entity_name.to_ascii_uppercase();
        self.docs.get(&version)?.get(&key)
    }
}

fn load_docs(json: &str) -> HashMap<String, EntityDoc> {
    serde_json::from_str(json).expect("Bundled schema documentation must be valid JSON")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_ifc4x3_entity_docs() {
        let docs = SchemaDocs::new();
        let entity = docs
            .get_entity_doc(IfcVersion::Ifc4x3Add2, "IFCWALL")
            .expect("expected IFC4x3 docs for IfcWall");

        assert_eq!(entity.name, "IfcWall");
        assert!(!entity.summary.is_empty());
        assert!(!entity.attributes.is_empty());
        assert!(entity.url.contains("IfcWall"));
    }

    #[test]
    fn loads_ifc4_entity_docs() {
        let docs = SchemaDocs::new();
        let entity = docs
            .get_entity_doc(IfcVersion::Ifc4Add2Tc1, "IFCWALL")
            .expect("expected IFC4 docs for IfcWall");

        assert_eq!(entity.name, "IfcWall");
        assert!(!entity.summary.is_empty());
        assert!(!entity.attributes.is_empty());
        assert!(entity.url.contains("ifcwall.htm"));
    }

    #[test]
    fn loads_ifc2x3_entity_docs() {
        let docs = SchemaDocs::new();
        let wall = docs
            .get_entity_doc(IfcVersion::Ifc2x3Tc1, "IFCWALL")
            .expect("expected IFC2x3 docs for IfcWall");
        let door = docs
            .get_entity_doc(IfcVersion::Ifc2x3Tc1, "IFCDOOR")
            .expect("expected IFC2x3 docs for IfcDoor");

        assert_eq!(wall.name, "IfcWall");
        assert!(!wall.summary.is_empty());
        assert_eq!(wall.attributes.len(), 0);
        assert!(wall.url.contains("ifcwall.htm"));

        assert_eq!(door.name, "IfcDoor");
        assert!(!door.summary.is_empty());
        assert!(!door.attributes.is_empty());
        assert!(door.url.contains("ifcdoor.htm"));
    }
}
