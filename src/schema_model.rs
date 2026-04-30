//! Runtime loader and resolver for generated schema-model assets.

use std::collections::{BTreeMap, HashMap, HashSet};

use schema_model::{
    DerivedAttributeDef, NamedTypeKind, SchemaModel, TypeDef, TypeRef, normalize_name,
};

use crate::schema::IfcVersion;

pub struct SchemaModelStore {
    schemas: HashMap<IfcVersion, ResolvedSchema>,
}

impl SchemaModelStore {
    pub fn new() -> Self {
        let mut schemas = HashMap::new();
        schemas.insert(
            IfcVersion::Ifc2x3Tc1,
            ResolvedSchema::from_json(include_str!(
                "../data/schema-models/ifc2x3_tc1_schema_model.json"
            ))
            .expect("bundled IFC2X3 schema model must be valid"),
        );
        schemas.insert(
            IfcVersion::Ifc4Add2Tc1,
            ResolvedSchema::from_json(include_str!(
                "../data/schema-models/ifc4_add2_tc1_schema_model.json"
            ))
            .expect("bundled IFC4 schema model must be valid"),
        );
        schemas.insert(
            IfcVersion::Ifc4x3Add2,
            ResolvedSchema::from_json(include_str!(
                "../data/schema-models/ifc4x3_add2_schema_model.json"
            ))
            .expect("bundled IFC4X3 schema model must be valid"),
        );

        Self { schemas }
    }

    pub fn get(&self, version: IfcVersion) -> Option<&ResolvedSchema> {
        self.schemas.get(&version)
    }
}

pub struct ResolvedSchema {
    raw: SchemaModel,
    entities: HashMap<String, ResolvedEntity>,
}

impl ResolvedSchema {
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        let raw: SchemaModel = serde_json::from_str(json)?;
        Ok(Self::from_model(raw))
    }

    pub fn from_model(raw: SchemaModel) -> Self {
        let mut entities = HashMap::new();

        for name in raw.entities.keys() {
            let all_attributes = resolve_all_attributes(&raw.entities, name, &mut HashSet::new());
            let all_supertypes = resolve_all_supertypes(&raw.entities, name, &mut HashSet::new());
            let declared = raw
                .entities
                .get(name)
                .expect("entity key should resolve to entity");

            entities.insert(
                name.clone(),
                ResolvedEntity {
                    name: declared.name.clone(),
                    all_attributes,
                    all_supertypes,
                },
            );
        }

        Self { raw, entities }
    }

    pub fn entity(&self, name: &str) -> Option<&ResolvedEntity> {
        self.entities.get(&normalize_name(name))
    }

    pub fn type_decl(&self, name: &str) -> Option<&TypeDef> {
        self.raw.type_decl(name)
    }

    pub fn is_entity_compatible(&self, actual: &str, expected: &str) -> bool {
        let actual = normalize_name(actual);
        let expected = normalize_name(expected);

        actual == expected
            || self
                .entities
                .get(&actual)
                .is_some_and(|entity| entity.all_supertypes.contains(&expected))
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedEntity {
    pub name: String,
    pub all_attributes: Vec<ResolvedAttribute>,
    pub all_supertypes: HashSet<String>,
}

#[derive(Debug, Clone)]
pub struct ResolvedAttribute {
    pub name: String,
    pub ty: TypeRef,
    pub optional: bool,
    pub declared_in: String,
    pub allows_omitted: bool,
}

fn resolve_all_attributes(
    entities: &BTreeMap<String, schema_model::EntityDef>,
    name: &str,
    visiting: &mut HashSet<String>,
) -> Vec<ResolvedAttribute> {
    if !visiting.insert(name.to_string()) {
        return Vec::new();
    }

    let Some(entity) = entities.get(name) else {
        visiting.remove(name);
        return Vec::new();
    };

    let mut attributes = Vec::new();

    for supertype in &entity.supertypes {
        attributes.extend(resolve_all_attributes(entities, supertype, visiting));
    }

    let derived_attributes = entity.derived_attributes.clone();

    attributes.extend(entity.attributes.iter().map(|attr| ResolvedAttribute {
        name: attr.name.clone(),
        ty: attr.ty.clone(),
        optional: attr.optional,
        declared_in: entity.name.clone(),
        allows_omitted: false,
    }));

    for attribute in &mut attributes {
        if matches_derived_override(attribute, &derived_attributes) {
            attribute.allows_omitted = true;
        }
    }

    visiting.remove(name);
    attributes
}

fn matches_derived_override(
    attribute: &ResolvedAttribute,
    derived_attributes: &[DerivedAttributeDef],
) -> bool {
    derived_attributes.iter().any(|derived| {
        derived.name.eq_ignore_ascii_case(&attribute.name)
            && derived
                .declared_in
                .as_ref()
                .is_none_or(|declared_in| declared_in.eq_ignore_ascii_case(&attribute.declared_in))
    })
}

fn resolve_all_supertypes(
    entities: &BTreeMap<String, schema_model::EntityDef>,
    name: &str,
    visiting: &mut HashSet<String>,
) -> HashSet<String> {
    if !visiting.insert(name.to_string()) {
        return HashSet::new();
    }

    let mut all_supertypes = HashSet::new();

    if let Some(entity) = entities.get(name) {
        for supertype in &entity.supertypes {
            all_supertypes.insert(supertype.clone());
            all_supertypes.extend(resolve_all_supertypes(entities, supertype, visiting));
        }
    }

    visiting.remove(name);
    all_supertypes
}

pub fn unwrap_named_type<'a>(schema: &'a ResolvedSchema, name: &str) -> Option<&'a TypeDef> {
    schema.type_decl(name)
}

#[cfg(test)]
mod tests {
    use schema_model::{
        AttributeDef, EntityDef, NamedTypeRef, PrimitiveType, TypeDef,
        normalize_name as normalize_key,
    };

    use super::*;

    fn demo_schema() -> SchemaModel {
        let mut schema = SchemaModel::new("DEMO");
        schema.entities.insert(
            normalize_key("IfcRoot"),
            EntityDef {
                name: "IfcRoot".to_string(),
                attributes: vec![AttributeDef {
                    name: "GlobalId".to_string(),
                    ty: TypeRef::Primitive(PrimitiveType::String {
                        width: None,
                        fixed: false,
                    }),
                    optional: false,
                    position: 0,
                }],
                derived_attributes: Vec::new(),
                supertypes: Vec::new(),
                where_rules: Vec::new(),
            },
        );
        schema.entities.insert(
            normalize_key("IfcElement"),
            EntityDef {
                name: "IfcElement".to_string(),
                attributes: vec![AttributeDef {
                    name: "Tag".to_string(),
                    ty: TypeRef::Named(NamedTypeRef {
                        name: "IFCLABEL".to_string(),
                        kind: NamedTypeKind::Type,
                    }),
                    optional: true,
                    position: 0,
                }],
                derived_attributes: Vec::new(),
                supertypes: vec!["IFCROOT".to_string()],
                where_rules: Vec::new(),
            },
        );
        schema.entities.insert(
            normalize_key("IfcWall"),
            EntityDef {
                name: "IfcWall".to_string(),
                attributes: vec![AttributeDef {
                    name: "PredefinedType".to_string(),
                    ty: TypeRef::Named(NamedTypeRef {
                        name: "IFCWALLTYPEENUM".to_string(),
                        kind: NamedTypeKind::Type,
                    }),
                    optional: true,
                    position: 0,
                }],
                derived_attributes: Vec::new(),
                supertypes: vec!["IFCELEMENT".to_string()],
                where_rules: Vec::new(),
            },
        );
        schema.types.insert(
            normalize_key("IfcLabel"),
            TypeDef::Alias(schema_model::AliasTypeDef {
                name: "IfcLabel".to_string(),
                target: TypeRef::Primitive(PrimitiveType::String {
                    width: Some(255),
                    fixed: false,
                }),
                where_rules: Vec::new(),
            }),
        );
        schema
    }

    #[test]
    fn resolved_schema_flattens_inherited_attributes() {
        let schema = ResolvedSchema::from_model(demo_schema());
        let wall = schema.entity("IFCWALL").expect("IfcWall should resolve");

        assert_eq!(wall.all_attributes.len(), 3);
        assert_eq!(wall.all_attributes[0].name, "GlobalId");
        assert_eq!(wall.all_attributes[1].name, "Tag");
        assert_eq!(wall.all_attributes[2].name, "PredefinedType");
        assert_eq!(wall.all_attributes[2].declared_in, "IfcWall");
    }

    #[test]
    fn resolved_schema_knows_subtype_compatibility() {
        let schema = ResolvedSchema::from_model(demo_schema());

        assert!(schema.is_entity_compatible("IFCWALL", "IFCROOT"));
        assert!(schema.is_entity_compatible("IFCWALL", "IFCELEMENT"));
        assert!(!schema.is_entity_compatible("IFCROOT", "IFCWALL"));
    }
}
