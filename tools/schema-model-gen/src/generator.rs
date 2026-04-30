use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use espr::ast::{
    AttributeDecl, Bound, BuiltInConstant, Entity, EntityAttribute, Expression, Extensibility,
    Literal, SimpleType, SubTypeConstraint, SyntaxTree, Type, TypeDecl, WhereClause,
};
use schema_model::{
    AggregateBounds, AggregateKind, AggregateTypeRef, AliasTypeDef, AttributeDef, BoundValue,
    DerivedAttributeDef, EntityDef, EnumerationTypeDef, NamedTypeKind, NamedTypeRef, PrimitiveType,
    SchemaModel, SelectTypeDef, TypeDef, TypeRef, WhereRuleDef, normalize_name,
};

#[derive(Debug)]
pub enum GenerateError {
    Parse(String),
    MissingSchema,
}

impl fmt::Display for GenerateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GenerateError::Parse(message) => write!(f, "failed to parse EXPRESS schema: {message}"),
            GenerateError::MissingSchema => write!(f, "EXPRESS input does not contain a schema"),
        }
    }
}

impl Error for GenerateError {}

pub fn generate_schema_model(source: &str) -> Result<SchemaModel, GenerateError> {
    let sanitized = sanitize_express_source(source);
    let syntax_tree = SyntaxTree::parse(&sanitized)
        .map_err(|error| GenerateError::Parse(format!("{error:?}")))?;
    let schema = syntax_tree
        .schemas
        .first()
        .ok_or(GenerateError::MissingSchema)?;

    let entity_names = schema
        .entities
        .iter()
        .map(|entity| normalize_name(&entity.name))
        .collect::<BTreeSet<_>>();
    let type_names = schema
        .types
        .iter()
        .map(|type_decl| normalize_name(&type_decl.type_id))
        .collect::<BTreeSet<_>>();

    let mut model = SchemaModel::new(schema.name.clone());
    let subtype_constraints = schema.subtype_constraints.as_slice();

    for entity in &schema.entities {
        let entity_def = normalize_entity(entity, subtype_constraints, &entity_names, &type_names);
        model
            .entities
            .insert(normalize_name(&entity.name), entity_def);
    }

    for type_decl in &schema.types {
        let type_def = normalize_type_decl(type_decl, &entity_names, &type_names);
        model
            .types
            .insert(normalize_name(&type_decl.type_id), type_def);
    }

    Ok(model)
}

fn sanitize_express_source(source: &str) -> String {
    let source = source.replace("\r\n", "\n");
    let source = strip_block_sections(&source, "FUNCTION", "END_FUNCTION;");
    let source = strip_block_sections(&source, "PROCEDURE", "END_PROCEDURE;");
    strip_block_sections(&source, "RULE", "END_RULE;")
}

fn strip_block_sections(source: &str, start_keyword: &str, end_keyword: &str) -> String {
    let mut output = String::new();
    let mut skipping = false;

    for line in source.lines() {
        let trimmed = line.trim_start();

        if skipping {
            if trimmed.starts_with(end_keyword) {
                skipping = false;
            }
            continue;
        }

        if trimmed.starts_with(start_keyword) {
            skipping = true;
            continue;
        }

        output.push_str(line);
        output.push('\n');
    }

    output
}

fn normalize_entity(
    entity: &Entity,
    _subtype_constraints: &[SubTypeConstraint],
    entity_names: &BTreeSet<String>,
    type_names: &BTreeSet<String>,
) -> EntityDef {
    EntityDef {
        name: entity.name.clone(),
        attributes: entity
            .attributes
            .iter()
            .enumerate()
            .filter_map(|(position, attr)| {
                normalize_attribute(attr, position, entity_names, type_names)
            })
            .collect(),
        derived_attributes: normalize_derived_attributes(entity),
        supertypes: entity
            .subtype_of
            .as_ref()
            .map(|decl| {
                decl.entity_references
                    .iter()
                    .map(|name| normalize_name(name))
                    .collect()
            })
            .unwrap_or_default(),
        where_rules: where_rules(&entity.where_clause),
    }
}

fn normalize_derived_attributes(entity: &Entity) -> Vec<DerivedAttributeDef> {
    entity
        .derive_clause
        .as_ref()
        .map(|clause| {
            clause
                .attributes
                .iter()
                .map(|attribute| match &attribute.attr {
                    AttributeDecl::Reference(name) => DerivedAttributeDef {
                        name: name.clone(),
                        declared_in: None,
                    },
                    AttributeDecl::Qualified {
                        group,
                        attribute,
                        rename: _,
                    } => DerivedAttributeDef {
                        name: attribute.clone(),
                        declared_in: Some(normalize_name(group)),
                    },
                })
                .collect()
        })
        .unwrap_or_default()
}

fn normalize_attribute(
    attr: &EntityAttribute,
    position: usize,
    entity_names: &BTreeSet<String>,
    type_names: &BTreeSet<String>,
) -> Option<AttributeDef> {
    let AttributeDecl::Reference(name) = &attr.name else {
        return None;
    };

    Some(AttributeDef {
        name: name.clone(),
        ty: normalize_type_ref(&attr.ty, entity_names, type_names),
        optional: attr.optional,
        position,
    })
}

fn normalize_type_decl(
    type_decl: &TypeDecl,
    entity_names: &BTreeSet<String>,
    type_names: &BTreeSet<String>,
) -> TypeDef {
    match &type_decl.underlying_type {
        Type::Enumeration {
            extensibility,
            items,
        } => TypeDef::Enumeration(EnumerationTypeDef {
            name: type_decl.type_id.clone(),
            items: items.iter().map(|item| normalize_name(item)).collect(),
            extensible: !matches!(extensibility, Extensibility::None),
            where_rules: where_rules(&type_decl.where_clause),
        }),
        Type::Select {
            extensibility,
            types,
        } => TypeDef::Select(SelectTypeDef {
            name: type_decl.type_id.clone(),
            options: types
                .iter()
                .map(|name| normalize_named_type_ref(name, entity_names, type_names))
                .map(TypeRef::Named)
                .collect(),
            extensible: !matches!(extensibility, Extensibility::None),
            generic_entity: matches!(extensibility, Extensibility::GenericEntity),
            where_rules: where_rules(&type_decl.where_clause),
        }),
        ty => TypeDef::Alias(AliasTypeDef {
            name: type_decl.type_id.clone(),
            target: normalize_type_ref(ty, entity_names, type_names),
            where_rules: where_rules(&type_decl.where_clause),
        }),
    }
}

fn normalize_type_ref(
    ty: &Type,
    entity_names: &BTreeSet<String>,
    type_names: &BTreeSet<String>,
) -> TypeRef {
    match ty {
        Type::Simple(simple) => TypeRef::Primitive(normalize_simple_type(simple)),
        Type::Named(name) => {
            TypeRef::Named(normalize_named_type_ref(name, entity_names, type_names))
        }
        Type::Set { base, bound } => TypeRef::Aggregate(AggregateTypeRef {
            kind: AggregateKind::Set,
            bounds: bound.as_ref().map(normalize_bounds),
            item: Box::new(normalize_type_ref(base, entity_names, type_names)),
            unique: false,
            optional: false,
        }),
        Type::Bag { base, bound } => TypeRef::Aggregate(AggregateTypeRef {
            kind: AggregateKind::Bag,
            bounds: bound.as_ref().map(normalize_bounds),
            item: Box::new(normalize_type_ref(base, entity_names, type_names)),
            unique: false,
            optional: false,
        }),
        Type::List {
            base,
            bound,
            unique,
        } => TypeRef::Aggregate(AggregateTypeRef {
            kind: AggregateKind::List,
            bounds: bound.as_ref().map(normalize_bounds),
            item: Box::new(normalize_type_ref(base, entity_names, type_names)),
            unique: *unique,
            optional: false,
        }),
        Type::Array {
            base,
            bound,
            unique,
            optional,
        } => TypeRef::Aggregate(AggregateTypeRef {
            kind: AggregateKind::Array,
            bounds: bound.as_ref().map(normalize_bounds),
            item: Box::new(normalize_type_ref(base, entity_names, type_names)),
            unique: *unique,
            optional: *optional,
        }),
        Type::Aggregate { base, label } => TypeRef::Aggregate(AggregateTypeRef {
            kind: AggregateKind::List,
            bounds: None,
            item: Box::new(normalize_type_ref(base, entity_names, type_names)),
            unique: false,
            optional: label.is_some(),
        }),
        Type::GenericEntity(label) => TypeRef::GenericEntity {
            label: label.clone(),
        },
        Type::Generic(label) => TypeRef::Generic {
            label: label.clone(),
        },
        Type::Enumeration {
            extensibility,
            items,
        } => TypeRef::Named(NamedTypeRef {
            name: format!(
                "__anonymous_enum:{}:{}",
                matches!(extensibility, Extensibility::Extensible),
                items.join("|")
            ),
            kind: NamedTypeKind::Type,
        }),
        Type::Select {
            extensibility,
            types,
        } => TypeRef::Named(NamedTypeRef {
            name: format!(
                "__anonymous_select:{}:{}",
                matches!(extensibility, Extensibility::GenericEntity),
                types.join("|")
            ),
            kind: NamedTypeKind::Type,
        }),
    }
}

fn normalize_simple_type(simple: &SimpleType) -> PrimitiveType {
    match simple {
        SimpleType::Number => PrimitiveType::Number,
        SimpleType::Real => PrimitiveType::Real,
        SimpleType::Integer => PrimitiveType::Integer,
        SimpleType::Logical => PrimitiveType::Logical,
        SimpleType::Boolen => PrimitiveType::Boolean,
        SimpleType::String_ { width_spec } => PrimitiveType::String {
            width: width_spec.map(|width| width.width),
            fixed: width_spec.is_some_and(|width| width.fixed),
        },
        SimpleType::Binary { width_spec } => PrimitiveType::Binary {
            width: width_spec.map(|width| width.width),
            fixed: width_spec.is_some_and(|width| width.fixed),
        },
    }
}

fn normalize_named_type_ref(
    name: &str,
    entity_names: &BTreeSet<String>,
    type_names: &BTreeSet<String>,
) -> NamedTypeRef {
    let normalized = normalize_name(name);
    let kind = if entity_names.contains(&normalized) {
        NamedTypeKind::Entity
    } else if type_names.contains(&normalized) {
        NamedTypeKind::Type
    } else {
        NamedTypeKind::Unresolved
    };

    NamedTypeRef {
        name: normalized,
        kind,
    }
}

fn normalize_bounds(bound: &Bound) -> AggregateBounds {
    AggregateBounds {
        lower: normalize_bound_value(&bound.lower),
        upper: normalize_bound_value(&bound.upper),
    }
}

fn normalize_bound_value(expr: &Expression) -> BoundValue {
    match expr {
        Expression::Literal(Literal::Real(value)) if *value >= 0.0 && value.fract() == 0.0 => {
            BoundValue::Integer(*value as u32)
        }
        Expression::QualifiableFactor {
            factor: espr::ast::QualifiableFactor::BuiltInConstant(BuiltInConstant::Indeterminate),
            qualifiers,
        } if qualifiers.is_empty() => BoundValue::Unbounded,
        _ => BoundValue::UnsupportedExpression,
    }
}

fn where_rules(where_clause: &Option<WhereClause>) -> Vec<WhereRuleDef> {
    where_clause
        .as_ref()
        .map(|where_clause| {
            where_clause
                .rules
                .iter()
                .map(|rule| WhereRuleDef {
                    label: rule.label.clone(),
                })
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use schema_model::{AggregateKind, NamedTypeKind, TypeDef};

    #[test]
    fn generates_schema_model_from_fixture_schema() {
        let source = r#"
        SCHEMA demo;
          TYPE IfcLabel = STRING(255);
          END_TYPE;

          TYPE IfcWallTypeEnum = ENUMERATION OF
            (MOVABLE
            ,PARAPET
            ,USERDEFINED);
          END_TYPE;

          TYPE IfcThingSelect = SELECT
            (IfcWall
            ,IfcLabel);
          END_TYPE;

          TYPE IfcWallSet = SET [1:?] OF IfcWall;
          END_TYPE;

          ENTITY IfcBuiltElement;
          END_ENTITY;

          ENTITY IfcWall
           SUBTYPE OF (IfcBuiltElement);
            Name : OPTIONAL IfcLabel;
            Related : SET [1:?] OF IfcWall;
            PredefinedType : IfcWallTypeEnum;
          WHERE
            WR1 : EXISTS(Name);
          END_ENTITY;
        END_SCHEMA;
        "#;

        let model = generate_schema_model(source).expect("fixture schema should parse");

        assert_eq!(model.schema_name, "demo");
        assert!(model.type_decl("IFCLABEL").is_some());
        assert!(model.entity("ifcwall").is_some());

        let TypeDef::Alias(label) = model
            .type_decl("IfcLabel")
            .expect("IfcLabel should be present")
        else {
            panic!("IfcLabel should normalize to alias");
        };
        assert_eq!(
            label.target,
            TypeRef::Primitive(PrimitiveType::String {
                width: Some(255),
                fixed: false,
            })
        );

        let TypeDef::Select(select) = model
            .type_decl("IfcThingSelect")
            .expect("IfcThingSelect should be present")
        else {
            panic!("IfcThingSelect should normalize to select");
        };
        assert_eq!(select.options.len(), 2);
        assert_eq!(
            select.options[0],
            TypeRef::Named(NamedTypeRef {
                name: "IFCWALL".to_string(),
                kind: NamedTypeKind::Entity,
            })
        );

        let wall = model.entity("IFCWALL").expect("IfcWall should be present");
        assert_eq!(wall.supertypes, vec!["IFCBUILTELEMENT".to_string()]);
        assert_eq!(wall.where_rules.len(), 1);
        assert_eq!(wall.attributes.len(), 3);
        assert!(wall.derived_attributes.is_empty());
        assert_eq!(wall.attributes[0].name, "Name");
        assert!(wall.attributes[0].optional);

        let related = &wall.attributes[1].ty;
        let TypeRef::Aggregate(aggregate) = related else {
            panic!("Related should normalize to aggregate");
        };
        assert_eq!(aggregate.kind, AggregateKind::Set);
        assert_eq!(
            aggregate.bounds.as_ref().expect("bound").lower,
            BoundValue::Integer(1)
        );
        assert_eq!(
            aggregate.bounds.as_ref().expect("bound").upper,
            BoundValue::Unbounded
        );
    }

    #[test]
    fn sanitize_drops_function_and_rule_blocks() {
        let source = r#"
        SCHEMA demo;
          FUNCTION Foo : LOGICAL;
            RETURN(TRUE);
          END_FUNCTION;
          RULE Bar FOR (IfcWall);
          WHERE
            WR1 : TRUE;
          END_RULE;
          ENTITY IfcWall;
          END_ENTITY;
        END_SCHEMA;
        "#;

        let sanitized = sanitize_express_source(source);

        assert!(!sanitized.contains("FUNCTION Foo"));
        assert!(!sanitized.contains("RULE Bar"));
        assert!(sanitized.contains("ENTITY IfcWall"));
    }

    #[test]
    fn generator_preserves_derived_attribute_overrides() {
        let source = r#"
        SCHEMA demo;
          ENTITY IfcNamedUnit;
            Dimensions : IfcDimensionalExponents;
            UnitType : IfcUnitEnum;
          END_ENTITY;

          ENTITY IfcSIUnit
           SUBTYPE OF (IfcNamedUnit);
            Prefix : OPTIONAL IfcSIPrefix;
            Name : IfcSIUnitName;
          DERIVE
            SELF\IfcNamedUnit.Dimensions : IfcDimensionalExponents := IfcDimensionsForSiUnit(SELF.Name);
          END_ENTITY;
        END_SCHEMA;
        "#;

        let model = generate_schema_model(source).expect("fixture schema should parse");
        let si_unit = model
            .entity("IFCSIUNIT")
            .expect("IfcSIUnit should be present");

        assert_eq!(si_unit.derived_attributes.len(), 1);
        assert_eq!(si_unit.derived_attributes[0].name, "Dimensions");
        assert_eq!(
            si_unit.derived_attributes[0].declared_in.as_deref(),
            Some("IFCNAMEDUNIT")
        );
    }
}
