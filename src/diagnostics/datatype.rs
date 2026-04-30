//! Datatype validation diagnostics for IFC entity attributes.

use schema_model::{BoundValue, NamedTypeKind, PrimitiveType, SelectTypeDef, TypeDef, TypeRef};
use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity};

use crate::document::{Document, ParameterValue};
use crate::schema_model::{ResolvedEntity, ResolvedSchema, unwrap_named_type};

pub fn collect(document: &Document, schema: &ResolvedSchema) -> Vec<Diagnostic> {
    let mut diagnostics = collect_syntax_diagnostics(document);

    for instance in &document.instances {
        let Some(entity) = schema.entity(&instance.entity_name) else {
            continue;
        };

        validate_instance(document, schema, entity, instance, &mut diagnostics);
    }

    diagnostics
}

fn validate_instance(
    document: &Document,
    schema: &ResolvedSchema,
    entity: &ResolvedEntity,
    instance: &crate::document::EntityInstanceInfo,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if instance.parameters.len() != entity.all_attributes.len() {
        diagnostics.push(Diagnostic {
            range: instance
                .parameter_list_range
                .unwrap_or(instance.entity_range),
            severity: Some(DiagnosticSeverity::ERROR),
            message: format!(
                "{} expects {} attributes but found {}",
                entity.name,
                entity.all_attributes.len(),
                instance.parameters.len()
            ),
            ..Default::default()
        });
    }

    for (value, attribute) in instance.parameters.iter().zip(&entity.all_attributes) {
        if let Some(message) = validate_value(document, schema, value, attribute) {
            diagnostics.push(Diagnostic {
                range: value.range(),
                severity: Some(DiagnosticSeverity::ERROR),
                message: format!("{}: {}", attribute.name, message),
                ..Default::default()
            });
        }
    }
}

fn validate_value(
    document: &Document,
    schema: &ResolvedSchema,
    value: &ParameterValue,
    attribute: &crate::schema_model::ResolvedAttribute,
) -> Option<String> {
    match value {
        ParameterValue::Null { .. } => {
            if attribute.optional {
                None
            } else {
                Some("attribute is required and does not allow `$`".to_string())
            }
        }
        ParameterValue::Omitted { .. } => {
            if attribute.allows_omitted {
                None
            } else {
                Some("`*` is not supported for this attribute".to_string())
            }
        }
        _ => validate_non_null_value(document, schema, value, &attribute.ty),
    }
}

fn validate_non_null_value(
    document: &Document,
    schema: &ResolvedSchema,
    value: &ParameterValue,
    expected: &TypeRef,
) -> Option<String> {
    match expected {
        TypeRef::Primitive(primitive) => validate_primitive(value, primitive),
        TypeRef::Aggregate(aggregate) => validate_aggregate(document, schema, value, aggregate),
        TypeRef::GenericEntity { .. } | TypeRef::Generic { .. } => None,
        TypeRef::Named(named) => match named.kind {
            NamedTypeKind::Entity => {
                validate_entity_reference(document, schema, value, &named.name)
            }
            NamedTypeKind::Type | NamedTypeKind::Unresolved => {
                validate_named_type(document, schema, value, &named.name)
            }
        },
    }
}

fn validate_named_type(
    document: &Document,
    schema: &ResolvedSchema,
    value: &ParameterValue,
    type_name: &str,
) -> Option<String> {
    let type_def = unwrap_named_type(schema, type_name)?;
    match type_def {
        TypeDef::Alias(alias) => {
            if let ParameterValue::Typed {
                type_name: inline_name,
                inner,
                ..
            } = value
            {
                if inline_name != type_name {
                    return Some(format!(
                        "expected typed value `{}` but found `{}`",
                        type_name, inline_name
                    ));
                }

                if inner.len() != 1 {
                    Some(format!(
                        "typed value `{}` should contain exactly one argument",
                        type_name
                    ))
                } else {
                    validate_non_null_value(document, schema, &inner[0], &alias.target)
                }
            } else {
                validate_non_null_value(document, schema, value, &alias.target)
            }
        }
        TypeDef::Enumeration(enum_def) => {
            if let ParameterValue::Typed {
                type_name: inline_name,
                inner,
                ..
            } = value
            {
                if inline_name != type_name {
                    return Some(format!(
                        "expected typed value `{}` but found `{}`",
                        type_name, inline_name
                    ));
                }

                if inner.len() != 1 {
                    Some(format!(
                        "typed value `{}` should contain exactly one argument",
                        type_name
                    ))
                } else {
                    validate_enum_value(&inner[0], &enum_def.items)
                }
            } else {
                validate_enum_value(value, &enum_def.items)
            }
        }
        TypeDef::Select(select) => validate_select(document, schema, value, select),
    }
}

fn validate_select(
    document: &Document,
    schema: &ResolvedSchema,
    value: &ParameterValue,
    select: &SelectTypeDef,
) -> Option<String> {
    if select
        .options
        .iter()
        .any(|option| validate_non_null_value(document, schema, value, option).is_none())
    {
        None
    } else {
        Some(format!(
            "value does not match any option of `{}`",
            select.name
        ))
    }
}

fn validate_enum_value(value: &ParameterValue, items: &[String]) -> Option<String> {
    match value {
        ParameterValue::Enumeration { value, .. } => {
            if items.contains(value) {
                None
            } else {
                Some(format!(
                    "expected one of {:?} but found `.{}.`",
                    items, value
                ))
            }
        }
        _ => Some(format!(
            "expected enumeration value but found {}",
            value.kind_name()
        )),
    }
}

fn validate_aggregate(
    document: &Document,
    schema: &ResolvedSchema,
    value: &ParameterValue,
    aggregate: &schema_model::AggregateTypeRef,
) -> Option<String> {
    let ParameterValue::List { items, .. } = value else {
        return Some(format!(
            "expected {} aggregate but found {}",
            aggregate.kind.as_str(),
            value.kind_name()
        ));
    };

    if let Some(bounds) = &aggregate.bounds {
        if let BoundValue::Integer(lower) = bounds.lower
            && items.len() < lower as usize
        {
            return Some(format!(
                "expected at least {} items but found {}",
                lower,
                items.len()
            ));
        }
        if let BoundValue::Integer(upper) = bounds.upper
            && items.len() > upper as usize
        {
            return Some(format!(
                "expected at most {} items but found {}",
                upper,
                items.len()
            ));
        }
    }

    for item in items {
        if let Some(message) = validate_non_null_value(document, schema, item, &aggregate.item) {
            return Some(message);
        }
    }

    None
}

fn validate_entity_reference(
    document: &Document,
    schema: &ResolvedSchema,
    value: &ParameterValue,
    expected_entity: &str,
) -> Option<String> {
    let ParameterValue::Reference { id, .. } = value else {
        return Some(format!(
            "expected reference to `{}` but found {}",
            expected_entity,
            value.kind_name()
        ));
    };

    let Some(definition) = document.definitions.get(id) else {
        return Some(format!(
            "reference `#{}` does not resolve to a local entity",
            id
        ));
    };

    if schema.is_entity_compatible(&definition.entity_name, expected_entity) {
        None
    } else {
        Some(format!(
            "expected reference to `{}` but `#{}` points to `{}`",
            expected_entity, id, definition.entity_name
        ))
    }
}

fn validate_primitive(value: &ParameterValue, primitive: &PrimitiveType) -> Option<String> {
    match primitive {
        PrimitiveType::Integer => match value {
            ParameterValue::Number { text, .. } if is_integer(text) => None,
            ParameterValue::Number { .. } => Some("expected integer number".to_string()),
            _ => Some(format!("expected integer but found {}", value.kind_name())),
        },
        PrimitiveType::Real | PrimitiveType::Number => match value {
            ParameterValue::Number { .. } => None,
            _ => Some(format!(
                "expected numeric value but found {}",
                value.kind_name()
            )),
        },
        PrimitiveType::String { .. } => match value {
            ParameterValue::String { .. } => None,
            _ => Some(format!("expected string but found {}", value.kind_name())),
        },
        PrimitiveType::Logical | PrimitiveType::Boolean => match value {
            ParameterValue::Enumeration { value, .. }
                if matches!(
                    value.as_str(),
                    "TRUE" | "FALSE" | "UNKNOWN" | "T" | "F" | "U"
                ) =>
            {
                None
            }
            _ => Some(format!(
                "expected logical/boolean enumeration but found {}",
                value.kind_name()
            )),
        },
        PrimitiveType::Binary { .. } => match value {
            ParameterValue::String { .. } => None,
            _ => Some(format!(
                "expected binary literal but found {}",
                value.kind_name()
            )),
        },
    }
}

fn is_integer(text: &str) -> bool {
    !text.contains(['.', 'E', 'e'])
}

trait AggregateKindDisplay {
    fn as_str(&self) -> &'static str;
}

impl AggregateKindDisplay for schema_model::AggregateKind {
    fn as_str(&self) -> &'static str {
        match self {
            schema_model::AggregateKind::Set => "set",
            schema_model::AggregateKind::Bag => "bag",
            schema_model::AggregateKind::List => "list",
            schema_model::AggregateKind::Array => "array",
        }
    }
}

fn collect_syntax_diagnostics(document: &Document) -> Vec<Diagnostic> {
    let Some(tree) = &document.tree else {
        return Vec::new();
    };

    let mut diagnostics = Vec::new();
    let mut cursor = tree.root_node().walk();
    collect_error_nodes(&mut cursor, &mut diagnostics);
    diagnostics
}

fn collect_error_nodes(
    cursor: &mut tree_sitter::TreeCursor<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    loop {
        let node = cursor.node();

        if node.is_error() {
            diagnostics.push(Diagnostic {
                range: node_range(&node),
                severity: Some(DiagnosticSeverity::ERROR),
                message: "Invalid IFC STEP syntax".to_string(),
                ..Default::default()
            });
        }

        if cursor.goto_first_child() {
            collect_error_nodes(cursor, diagnostics);
            cursor.goto_parent();
        }

        if !cursor.goto_next_sibling() {
            break;
        }
    }
}

fn node_range(node: &tree_sitter::Node<'_>) -> tower_lsp::lsp_types::Range {
    let start = node.start_position();
    let end = node.end_position();

    tower_lsp::lsp_types::Range {
        start: tower_lsp::lsp_types::Position::new(start.row as u32, start.column as u32),
        end: tower_lsp::lsp_types::Position::new(end.row as u32, end.column as u32),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use schema_model::{
        AttributeDef, DerivedAttributeDef, EntityDef, NamedTypeRef, PrimitiveType, SchemaModel,
        normalize_name,
    };
    use tower_lsp::lsp_types::{Position, Range};
    use tree_sitter::Parser;

    use super::*;
    use crate::document::Document;
    use crate::schema_model::ResolvedSchema;

    fn parse_document(text: &str) -> Document {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_ifc::LANGUAGE.into())
            .expect("Error loading IFC parser");
        Document::parse(&mut parser, text.to_string())
    }

    fn test_schema() -> ResolvedSchema {
        let mut schema = SchemaModel::new("IFC4");
        schema.types.insert(
            normalize_name("IfcLabel"),
            TypeDef::Alias(schema_model::AliasTypeDef {
                name: "IfcLabel".to_string(),
                target: TypeRef::Primitive(PrimitiveType::String {
                    width: Some(255),
                    fixed: false,
                }),
                where_rules: Vec::new(),
            }),
        );
        schema.types.insert(
            normalize_name("IfcWallTypeEnum"),
            TypeDef::Enumeration(schema_model::EnumerationTypeDef {
                name: "IfcWallTypeEnum".to_string(),
                items: vec!["MOVABLE".to_string(), "USERDEFINED".to_string()],
                extensible: false,
                where_rules: Vec::new(),
            }),
        );
        schema.entities.insert(
            normalize_name("IfcRoot"),
            EntityDef {
                name: "IfcRoot".to_string(),
                attributes: vec![AttributeDef {
                    name: "GlobalId".to_string(),
                    ty: TypeRef::Named(NamedTypeRef {
                        name: "IFCLABEL".to_string(),
                        kind: NamedTypeKind::Type,
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
            normalize_name("IfcWall"),
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
                supertypes: vec!["IFCROOT".to_string()],
                where_rules: Vec::new(),
            },
        );

        ResolvedSchema::from_model(schema)
    }

    #[test]
    fn datatype_validator_reports_mismatched_attribute_type() {
        let doc = parse_document("#1=IFCWALL(123,.MOVABLE.);");
        let diagnostics = collect(&doc, &test_schema());

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("GlobalId"));
        assert!(diagnostics[0].message.contains("expected string"));
    }

    #[test]
    fn datatype_validator_accepts_valid_values() {
        let doc = parse_document("#1=IFCWALL('gid',.MOVABLE.);");
        let diagnostics = collect(&doc, &test_schema());

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn datatype_validator_reports_unresolved_reference() {
        let document = Document {
            text: "#1=IFCWALL('gid',.MOVABLE.);".to_string(),
            tree: None,
            version: None,
            definitions: HashMap::new(),
            references: HashMap::new(),
            instances: vec![crate::document::EntityInstanceInfo {
                id: Some(1),
                id_range: Some(Range {
                    start: Position::new(0, 0),
                    end: Position::new(0, 2),
                }),
                entity_name: "IFCWALL".to_string(),
                entity_name_range: Range {
                    start: Position::new(0, 3),
                    end: Position::new(0, 10),
                },
                entity_range: Range {
                    start: Position::new(0, 0),
                    end: Position::new(0, 27),
                },
                parameter_list_range: Some(Range {
                    start: Position::new(0, 10),
                    end: Position::new(0, 27),
                }),
                parameters: vec![
                    ParameterValue::String {
                        range: Range {
                            start: Position::new(0, 11),
                            end: Position::new(0, 16),
                        },
                    },
                    ParameterValue::Reference {
                        id: 2,
                        range: Range {
                            start: Position::new(0, 17),
                            end: Position::new(0, 19),
                        },
                    },
                ],
            }],
        };

        let mut schema = SchemaModel::new("IFC4");
        schema.entities.insert(
            normalize_name("IfcRoot"),
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
            normalize_name("IfcWall"),
            EntityDef {
                name: "IfcWall".to_string(),
                attributes: vec![AttributeDef {
                    name: "Parent".to_string(),
                    ty: TypeRef::Named(NamedTypeRef {
                        name: "IFCWALL".to_string(),
                        kind: NamedTypeKind::Entity,
                    }),
                    optional: false,
                    position: 0,
                }],
                derived_attributes: Vec::new(),
                supertypes: vec!["IFCROOT".to_string()],
                where_rules: Vec::new(),
            },
        );

        let diagnostics = collect(&document, &ResolvedSchema::from_model(schema));

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("does not resolve"));
    }

    #[test]
    fn datatype_validator_allows_omitted_for_derived_inherited_attribute() {
        let mut schema = SchemaModel::new("IFC4");
        schema.types.insert(
            normalize_name("IfcUnitEnum"),
            TypeDef::Enumeration(schema_model::EnumerationTypeDef {
                name: "IfcUnitEnum".to_string(),
                items: vec!["LENGTHUNIT".to_string()],
                extensible: false,
                where_rules: Vec::new(),
            }),
        );
        schema.types.insert(
            normalize_name("IfcSIPrefix"),
            TypeDef::Enumeration(schema_model::EnumerationTypeDef {
                name: "IfcSIPrefix".to_string(),
                items: vec!["MILLI".to_string()],
                extensible: false,
                where_rules: Vec::new(),
            }),
        );
        schema.types.insert(
            normalize_name("IfcSIUnitName"),
            TypeDef::Enumeration(schema_model::EnumerationTypeDef {
                name: "IfcSIUnitName".to_string(),
                items: vec!["METRE".to_string()],
                extensible: false,
                where_rules: Vec::new(),
            }),
        );
        schema.entities.insert(
            normalize_name("IfcDimensionalExponents"),
            EntityDef {
                name: "IfcDimensionalExponents".to_string(),
                attributes: Vec::new(),
                derived_attributes: Vec::new(),
                supertypes: Vec::new(),
                where_rules: Vec::new(),
            },
        );
        schema.entities.insert(
            normalize_name("IfcNamedUnit"),
            EntityDef {
                name: "IfcNamedUnit".to_string(),
                attributes: vec![
                    AttributeDef {
                        name: "Dimensions".to_string(),
                        ty: TypeRef::Named(NamedTypeRef {
                            name: "IFCDIMENSIONALEXPONENTS".to_string(),
                            kind: NamedTypeKind::Entity,
                        }),
                        optional: false,
                        position: 0,
                    },
                    AttributeDef {
                        name: "UnitType".to_string(),
                        ty: TypeRef::Named(NamedTypeRef {
                            name: "IFCUNITENUM".to_string(),
                            kind: NamedTypeKind::Type,
                        }),
                        optional: false,
                        position: 1,
                    },
                ],
                derived_attributes: Vec::new(),
                supertypes: Vec::new(),
                where_rules: Vec::new(),
            },
        );
        schema.entities.insert(
            normalize_name("IfcSIUnit"),
            EntityDef {
                name: "IfcSIUnit".to_string(),
                attributes: vec![
                    AttributeDef {
                        name: "Prefix".to_string(),
                        ty: TypeRef::Named(NamedTypeRef {
                            name: "IFCSIPREFIX".to_string(),
                            kind: NamedTypeKind::Type,
                        }),
                        optional: true,
                        position: 0,
                    },
                    AttributeDef {
                        name: "Name".to_string(),
                        ty: TypeRef::Named(NamedTypeRef {
                            name: "IFCSIUNITNAME".to_string(),
                            kind: NamedTypeKind::Type,
                        }),
                        optional: false,
                        position: 1,
                    },
                ],
                derived_attributes: vec![DerivedAttributeDef {
                    name: "Dimensions".to_string(),
                    declared_in: Some("IFCNAMEDUNIT".to_string()),
                }],
                supertypes: vec!["IFCNAMEDUNIT".to_string()],
                where_rules: Vec::new(),
            },
        );

        let doc = parse_document("#15=IFCSIUNIT(*,.LENGTHUNIT.,.MILLI.,.METRE.);");
        let diagnostics = collect(&doc, &ResolvedSchema::from_model(schema));

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn datatype_validator_reports_invalid_step_syntax() {
        let doc = parse_document(r#"#14=IFCUNITASSIGNMENT((#15,#16,#17, "test"));"#);
        let diagnostics = collect(&doc, &test_schema());

        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("Invalid IFC STEP syntax")),
            "{diagnostics:?}"
        );
    }

    #[test]
    fn datatype_validator_accepts_inline_typed_values_for_selects() {
        let mut schema = SchemaModel::new("IFC4");
        schema.types.insert(
            normalize_name("IfcLabel"),
            TypeDef::Alias(schema_model::AliasTypeDef {
                name: "IfcLabel".to_string(),
                target: TypeRef::Primitive(PrimitiveType::String {
                    width: Some(255),
                    fixed: false,
                }),
                where_rules: Vec::new(),
            }),
        );
        schema.types.insert(
            normalize_name("IfcLengthMeasure"),
            TypeDef::Alias(schema_model::AliasTypeDef {
                name: "IfcLengthMeasure".to_string(),
                target: TypeRef::Primitive(PrimitiveType::Real),
                where_rules: Vec::new(),
            }),
        );
        schema.types.insert(
            normalize_name("IfcValue"),
            TypeDef::Select(schema_model::SelectTypeDef {
                name: "IfcValue".to_string(),
                options: vec![
                    TypeRef::Named(NamedTypeRef {
                        name: "IFCLABEL".to_string(),
                        kind: NamedTypeKind::Type,
                    }),
                    TypeRef::Named(NamedTypeRef {
                        name: "IFCLENGTHMEASURE".to_string(),
                        kind: NamedTypeKind::Type,
                    }),
                ],
                extensible: false,
                generic_entity: false,
                where_rules: Vec::new(),
            }),
        );
        schema.entities.insert(
            normalize_name("IfcRoot"),
            EntityDef {
                name: "IfcRoot".to_string(),
                attributes: vec![AttributeDef {
                    name: "Name".to_string(),
                    ty: TypeRef::Named(NamedTypeRef {
                        name: "IFCLABEL".to_string(),
                        kind: NamedTypeKind::Type,
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
            normalize_name("IfcPropertySingleValue"),
            EntityDef {
                name: "IfcPropertySingleValue".to_string(),
                attributes: vec![
                    AttributeDef {
                        name: "Description".to_string(),
                        ty: TypeRef::Named(NamedTypeRef {
                            name: "IFCLABEL".to_string(),
                            kind: NamedTypeKind::Type,
                        }),
                        optional: true,
                        position: 0,
                    },
                    AttributeDef {
                        name: "NominalValue".to_string(),
                        ty: TypeRef::Named(NamedTypeRef {
                            name: "IFCVALUE".to_string(),
                            kind: NamedTypeKind::Type,
                        }),
                        optional: true,
                        position: 1,
                    },
                ],
                derived_attributes: Vec::new(),
                supertypes: vec!["IFCROOT".to_string()],
                where_rules: Vec::new(),
            },
        );

        let doc = parse_document(
            "#1=IFCPROPERTYSINGLEVALUE('Name',$,IFCLABEL('Living Room'));\n#2=IFCPROPERTYSINGLEVALUE('Offset',$,IFCLENGTHMEASURE(2.6));",
        );
        let diagnostics = collect(&doc, &ResolvedSchema::from_model(schema));

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }
}
