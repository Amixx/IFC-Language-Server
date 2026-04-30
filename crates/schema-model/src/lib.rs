use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchemaModel {
    pub schema_name: String,
    pub entities: BTreeMap<String, EntityDef>,
    pub types: BTreeMap<String, TypeDef>,
}

impl SchemaModel {
    pub fn new(schema_name: impl Into<String>) -> Self {
        Self {
            schema_name: schema_name.into(),
            entities: BTreeMap::new(),
            types: BTreeMap::new(),
        }
    }

    pub fn entity(&self, name: &str) -> Option<&EntityDef> {
        self.entities.get(&normalize_name(name))
    }

    pub fn type_decl(&self, name: &str) -> Option<&TypeDef> {
        self.types.get(&normalize_name(name))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityDef {
    pub name: String,
    pub attributes: Vec<AttributeDef>,
    pub derived_attributes: Vec<DerivedAttributeDef>,
    pub supertypes: Vec<String>,
    pub where_rules: Vec<WhereRuleDef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttributeDef {
    pub name: String,
    pub ty: TypeRef,
    pub optional: bool,
    pub position: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DerivedAttributeDef {
    pub name: String,
    pub declared_in: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TypeDef {
    Alias(AliasTypeDef),
    Enumeration(EnumerationTypeDef),
    Select(SelectTypeDef),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AliasTypeDef {
    pub name: String,
    pub target: TypeRef,
    pub where_rules: Vec<WhereRuleDef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnumerationTypeDef {
    pub name: String,
    pub items: Vec<String>,
    pub extensible: bool,
    pub where_rules: Vec<WhereRuleDef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectTypeDef {
    pub name: String,
    pub options: Vec<TypeRef>,
    pub extensible: bool,
    pub generic_entity: bool,
    pub where_rules: Vec<WhereRuleDef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WhereRuleDef {
    pub label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TypeRef {
    Primitive(PrimitiveType),
    Named(NamedTypeRef),
    Aggregate(AggregateTypeRef),
    GenericEntity { label: Option<String> },
    Generic { label: Option<String> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NamedTypeRef {
    pub name: String,
    pub kind: NamedTypeKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NamedTypeKind {
    Entity,
    Type,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AggregateTypeRef {
    pub kind: AggregateKind,
    pub bounds: Option<AggregateBounds>,
    pub item: Box<TypeRef>,
    pub unique: bool,
    pub optional: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AggregateKind {
    Set,
    Bag,
    List,
    Array,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AggregateBounds {
    pub lower: BoundValue,
    pub upper: BoundValue,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BoundValue {
    Integer(u32),
    Unbounded,
    UnsupportedExpression,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrimitiveType {
    Number,
    Real,
    Integer,
    Logical,
    Boolean,
    String { width: Option<usize>, fixed: bool },
    Binary { width: Option<usize>, fixed: bool },
}

pub fn normalize_name(name: &str) -> String {
    name.to_ascii_uppercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_is_case_insensitive() {
        let mut model = SchemaModel::new("IFC4");
        model.entities.insert(
            normalize_name("IfcWall"),
            EntityDef {
                name: "IfcWall".to_string(),
                attributes: Vec::new(),
                derived_attributes: Vec::new(),
                supertypes: vec!["IFCBUILTELEMENT".to_string()],
                where_rules: Vec::new(),
            },
        );

        assert!(model.entity("IFCWALL").is_some());
        assert!(model.entity("IfcWall").is_some());
        assert!(model.entity("ifcwall").is_some());
    }

    #[test]
    fn model_round_trips_through_json() {
        let mut model = SchemaModel::new("IFC4X3_ADD2");
        model.types.insert(
            normalize_name("IfcLabel"),
            TypeDef::Alias(AliasTypeDef {
                name: "IfcLabel".to_string(),
                target: TypeRef::Primitive(PrimitiveType::String {
                    width: Some(255),
                    fixed: false,
                }),
                where_rules: vec![WhereRuleDef {
                    label: Some("WR1".to_string()),
                }],
            }),
        );

        let json = serde_json::to_string_pretty(&model).expect("schema model should serialize");
        let restored: SchemaModel =
            serde_json::from_str(&json).expect("schema model should deserialize");

        assert_eq!(restored, model);
    }
}
