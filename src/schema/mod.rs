//! Runtime EXPRESS loading and schema lookup.
//! Schema docs are generated in memory from official EXPRESS definitions at startup.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::error::Error;
use std::fmt;

use espr::ast::{
    AttributeDecl, Bound, BuiltInConstant, Entity, EntityAttribute, Expression, Extensibility,
    Literal, SimpleType, SyntaxTree, Type, TypeDecl, WhereClause,
};

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum IfcVersion {
    Ifc2x3Tc1,
    Ifc4Add2Tc1,
    Ifc4x3Add2,
}

impl IfcVersion {
    pub fn supported() -> [Self; 3] {
        [Self::Ifc2x3Tc1, Self::Ifc4Add2Tc1, Self::Ifc4x3Add2]
    }

    pub fn schema_name(self) -> &'static str {
        match self {
            Self::Ifc2x3Tc1 => "IFC2X3",
            Self::Ifc4Add2Tc1 => "IFC4",
            Self::Ifc4x3Add2 => "IFC4X3_ADD2",
        }
    }

    fn express_url(self) -> &'static str {
        match self {
            Self::Ifc2x3Tc1 => {
                "https://standards.buildingsmart.org/IFC/RELEASE/IFC2x3/TC1/EXPRESS/IFC2X3_TC1.exp"
            }
            Self::Ifc4Add2Tc1 => {
                "https://standards.buildingsmart.org/IFC/RELEASE/IFC4/ADD2_TC1/EXPRESS/IFC4.exp"
            }
            Self::Ifc4x3Add2 => {
                "https://standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/IFC4X3_ADD2.exp"
            }
        }
    }

    fn documentation_url(self, entity_name: &str) -> String {
        match self {
            Self::Ifc2x3Tc1 => {
                "https://standards.buildingsmart.org/IFC/RELEASE/IFC2x3/TC1/HTML/alphabeticalorder_entities.htm".to_string()
            }
            Self::Ifc4Add2Tc1 => format!(
                "https://standards.buildingsmart.org/IFC/RELEASE/IFC4/ADD2_TC1/HTML/link/{}.htm",
                entity_name.to_ascii_lowercase()
            ),
            Self::Ifc4x3Add2 => format!(
                "https://standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/lexical/{}.htm",
                entity_name
            ),
        }
    }
}

impl fmt::Display for IfcVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.schema_name())
    }
}

#[derive(Debug, Default)]
pub struct SchemaDocCollection {
    pub docs: HashMap<IfcVersion, SchemaDoc>,
    load_errors: Vec<String>,
}

impl SchemaDocCollection {
    pub fn new() -> Self {
        let mut collection = Self::default();

        for version in IfcVersion::supported() {
            match load_official_schema(version) {
                Ok(schema) => {
                    collection.docs.insert(version, schema);
                }
                Err(error) => {
                    collection
                        .load_errors
                        .push(format!("{}: {}", version, error));
                }
            }
        }

        collection
    }

    #[cfg(test)]
    pub fn empty() -> Self {
        Self::default()
    }

    #[cfg(test)]
    pub fn from_docs(docs: impl IntoIterator<Item = (IfcVersion, SchemaDoc)>) -> Self {
        Self {
            docs: docs.into_iter().collect(),
            load_errors: Vec::new(),
        }
    }

    pub fn get(&self, version: IfcVersion) -> Option<&SchemaDoc> {
        self.docs.get(&version)
    }

    pub fn get_entity_doc(&self, version: IfcVersion, entity_name: &str) -> Option<&EntityDoc> {
        self.get(version)?.entity(entity_name)
    }

    pub fn load_errors(&self) -> &[String] {
        &self.load_errors
    }
}

#[derive(Debug, Clone)]
pub struct SchemaDoc {
    #[allow(dead_code)]
    pub schema_name: String,
    pub entities: HashMap<String, EntityDoc>,
    pub types: HashMap<String, TypeDoc>,
}

impl SchemaDoc {
    fn from_raw(version: IfcVersion, raw: RawSchema) -> Self {
        let mut entities = HashMap::new();

        for name in raw.entities.keys() {
            let attributes = resolve_all_attributes(&raw.entities, name, &mut HashSet::new());
            let all_supertypes = resolve_all_supertypes(&raw.entities, name, &mut HashSet::new());
            let declared = raw
                .entities
                .get(name)
                .expect("entity key should resolve to entity");

            entities.insert(
                name.clone(),
                EntityDoc {
                    name: declared.name.clone(),
                    attributes,
                    all_supertypes,
                    url: version.documentation_url(&declared.name),
                },
            );
        }

        Self {
            schema_name: raw.schema_name,
            entities,
            types: raw.types.into_iter().collect(),
        }
    }

    pub fn entity(&self, name: &str) -> Option<&EntityDoc> {
        self.entities.get(&normalize_name(name))
    }

    pub fn type_decl(&self, name: &str) -> Option<&TypeDoc> {
        self.types.get(&normalize_name(name))
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
pub struct EntityDoc {
    pub name: String,
    pub attributes: Vec<EntityAttributeDoc>,
    pub url: String,
    pub all_supertypes: HashSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityAttributeDoc {
    pub name: String,
    pub type_name: String,
    pub declared_in: String,
    pub ty: TypeRef,
    pub optional: bool,
    pub allows_omitted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeDoc {
    Alias(AliasTypeDef),
    Enumeration(EnumerationTypeDef),
    Select(SelectTypeDef),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AliasTypeDef {
    pub name: String,
    pub target: TypeRef,
    pub where_rules: Vec<WhereRuleDef>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumerationTypeDef {
    pub name: String,
    pub items: Vec<String>,
    pub extensible: bool,
    pub where_rules: Vec<WhereRuleDef>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectTypeDef {
    pub name: String,
    pub options: Vec<TypeRef>,
    pub extensible: bool,
    pub generic_entity: bool,
    pub where_rules: Vec<WhereRuleDef>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhereRuleDef {
    pub label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeRef {
    Primitive(PrimitiveType),
    Named(NamedTypeRef),
    Aggregate(AggregateTypeRef),
    GenericEntity { label: Option<String> },
    Generic { label: Option<String> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedTypeRef {
    pub name: String,
    pub kind: NamedTypeKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NamedTypeKind {
    Entity,
    Type,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AggregateTypeRef {
    pub kind: AggregateKind,
    pub bounds: Option<AggregateBounds>,
    pub item: Box<TypeRef>,
    pub unique: bool,
    pub optional: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AggregateKind {
    Set,
    Bag,
    List,
    Array,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AggregateBounds {
    pub lower: BoundValue,
    pub upper: BoundValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundValue {
    Integer(u32),
    Unbounded,
    UnsupportedExpression,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrimitiveType {
    Number,
    Real,
    Integer,
    Logical,
    Boolean,
    String { width: Option<usize>, fixed: bool },
    Binary { width: Option<usize>, fixed: bool },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EntityDef {
    pub name: String,
    pub attributes: Vec<AttributeDef>,
    pub derived_attributes: Vec<DerivedAttributeDef>,
    pub supertypes: Vec<String>,
    pub where_rules: Vec<WhereRuleDef>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AttributeDef {
    pub name: String,
    pub ty: TypeRef,
    pub type_name: String,
    pub optional: bool,
    pub position: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DerivedAttributeDef {
    pub name: String,
    pub declared_in: Option<String>,
}

#[derive(Debug)]
pub enum LoadExpressError {
    Parse(String),
    MissingSchema,
}

impl fmt::Display for LoadExpressError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(message) => write!(f, "failed to parse EXPRESS schema: {message}"),
            Self::MissingSchema => write!(f, "EXPRESS input does not contain a schema"),
        }
    }
}

impl Error for LoadExpressError {}

#[derive(Debug)]
enum SchemaLoadError {
    Fetch { url: &'static str, message: String },
    Parse(LoadExpressError),
}

impl fmt::Display for SchemaLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Fetch { url, message } => {
                write!(f, "failed to fetch EXPRESS schema from {url}: {message}")
            }
            Self::Parse(error) => write!(f, "{error}"),
        }
    }
}

impl Error for SchemaLoadError {}

struct RawSchema {
    schema_name: String,
    entities: BTreeMap<String, EntityDef>,
    types: BTreeMap<String, TypeDoc>,
}

pub fn load_express(version: IfcVersion, source: &str) -> Result<SchemaDoc, LoadExpressError> {
    let raw = parse_express_source(source)?;
    Ok(SchemaDoc::from_raw(version, raw))
}

fn load_official_schema(version: IfcVersion) -> Result<SchemaDoc, SchemaLoadError> {
    let url = version.express_url();
    let source = ureq::get(url)
        .call()
        .map_err(|error| SchemaLoadError::Fetch {
            url,
            message: error.to_string(),
        })?
        .body_mut()
        .read_to_string()
        .map_err(|error| SchemaLoadError::Fetch {
            url,
            message: error.to_string(),
        })?;

    load_express(version, &source).map_err(SchemaLoadError::Parse)
}

fn parse_express_source(source: &str) -> Result<RawSchema, LoadExpressError> {
    let sanitized = sanitize_express_source(source);
    let syntax_tree = SyntaxTree::parse(&sanitized)
        .map_err(|error| LoadExpressError::Parse(format!("{error:?}")))?;
    let schema = syntax_tree
        .schemas
        .first()
        .ok_or(LoadExpressError::MissingSchema)?;

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

    let mut entities = BTreeMap::new();
    let mut types = BTreeMap::new();

    for entity in &schema.entities {
        entities.insert(
            normalize_name(&entity.name),
            normalize_entity(entity, &entity_names, &type_names),
        );
    }

    for type_decl in &schema.types {
        types.insert(
            normalize_name(&type_decl.type_id),
            normalize_type_decl(type_decl, &entity_names, &type_names),
        );
    }

    Ok(RawSchema {
        schema_name: schema.name.clone(),
        entities,
        types,
    })
}

fn sanitize_express_source(source: &str) -> String {
    // Global algorithms/rules are not used by current hover or datatype diagnostics.
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
        type_name: format_attribute_type(&attr.ty, attr.optional),
        optional: attr.optional,
        position,
    })
}

fn normalize_type_decl(
    type_decl: &TypeDecl,
    entity_names: &BTreeSet<String>,
    type_names: &BTreeSet<String>,
) -> TypeDoc {
    match &type_decl.underlying_type {
        Type::Enumeration {
            extensibility,
            items,
        } => TypeDoc::Enumeration(EnumerationTypeDef {
            name: type_decl.type_id.clone(),
            items: items.iter().map(|item| normalize_name(item)).collect(),
            extensible: !matches!(extensibility, Extensibility::None),
            where_rules: where_rules(&type_decl.where_clause),
        }),
        Type::Select {
            extensibility,
            types,
        } => TypeDoc::Select(SelectTypeDef {
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
        ty => TypeDoc::Alias(AliasTypeDef {
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

fn format_attribute_type(ty: &Type, optional: bool) -> String {
    let type_name = format_type(ty);
    if optional {
        format!("OPTIONAL {type_name}")
    } else {
        type_name
    }
}

fn format_type(ty: &Type) -> String {
    match ty {
        Type::Simple(simple) => format_simple_type(simple),
        Type::Named(name) => name.clone(),
        Type::Set { base, bound } => {
            format!("SET{} OF {}", format_bound(bound), format_type(base))
        }
        Type::Bag { base, bound } => {
            format!("BAG{} OF {}", format_bound(bound), format_type(base))
        }
        Type::List {
            base,
            bound,
            unique,
        } => {
            let unique = if *unique { " UNIQUE" } else { "" };
            format!(
                "LIST{} OF{} {}",
                format_bound(bound),
                unique,
                format_type(base)
            )
        }
        Type::Array {
            base,
            bound,
            unique,
            optional,
        } => {
            let unique = if *unique { " UNIQUE" } else { "" };
            let optional = if *optional { " OPTIONAL" } else { "" };
            format!(
                "ARRAY{} OF{}{} {}",
                format_bound(bound),
                optional,
                unique,
                format_type(base)
            )
        }
        Type::Enumeration { items, .. } => format!("ENUMERATION OF ({})", items.join(", ")),
        Type::Select { types, .. } => format!("SELECT ({})", types.join(", ")),
        Type::Aggregate { base, .. } => format!("AGGREGATE OF {}", format_type(base)),
        Type::GenericEntity(label) => format_generic_type("GENERIC_ENTITY", label),
        Type::Generic(label) => format_generic_type("GENERIC", label),
    }
}

fn format_simple_type(simple: &SimpleType) -> String {
    match simple {
        SimpleType::Number => "NUMBER".to_string(),
        SimpleType::Real => "REAL".to_string(),
        SimpleType::Integer => "INTEGER".to_string(),
        SimpleType::Logical => "LOGICAL".to_string(),
        SimpleType::Boolen => "BOOLEAN".to_string(),
        SimpleType::String_ { width_spec } => format_width_type("STRING", width_spec),
        SimpleType::Binary { width_spec } => format_width_type("BINARY", width_spec),
    }
}

fn format_width_type(name: &str, width_spec: &Option<espr::ast::WidthSpec>) -> String {
    match width_spec {
        Some(width_spec) if width_spec.fixed => format!("{name}({}) FIXED", width_spec.width),
        Some(width_spec) => format!("{name}({})", width_spec.width),
        None => name.to_string(),
    }
}

fn format_bound(bound: &Option<Bound>) -> String {
    bound
        .as_ref()
        .map(|bound| {
            format!(
                " [{}:{}]",
                format_bound_value(&bound.lower),
                format_bound_value(&bound.upper)
            )
        })
        .unwrap_or_default()
}

fn format_bound_value(expr: &Expression) -> String {
    match normalize_bound_value(expr) {
        BoundValue::Integer(value) => value.to_string(),
        BoundValue::Unbounded | BoundValue::UnsupportedExpression => "?".to_string(),
    }
}

fn format_generic_type(name: &str, label: &Option<String>) -> String {
    label
        .as_ref()
        .map(|label| format!("{name}:{label}"))
        .unwrap_or_else(|| name.to_string())
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

fn resolve_all_attributes(
    entities: &BTreeMap<String, EntityDef>,
    name: &str,
    visiting: &mut HashSet<String>,
) -> Vec<EntityAttributeDoc> {
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

    attributes.extend(entity.attributes.iter().map(|attr| EntityAttributeDoc {
        name: attr.name.clone(),
        type_name: attr.type_name.clone(),
        declared_in: entity.name.clone(),
        ty: attr.ty.clone(),
        optional: attr.optional,
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
    attribute: &EntityAttributeDoc,
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
    entities: &BTreeMap<String, EntityDef>,
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

pub fn normalize_name(name: &str) -> String {
    name.to_ascii_uppercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_schema() -> &'static str {
        r#"
        SCHEMA DEMO;
          TYPE IfcLabel = STRING(255);
          END_TYPE;

          TYPE IfcWallTypeEnum = ENUMERATION OF (MOVABLE, USERDEFINED);
          END_TYPE;

          TYPE IfcObjectReferenceSelect = SELECT (IfcRoot, IfcLabel);
          END_TYPE;

          ENTITY IfcRoot;
            GlobalId : IfcLabel;
          END_ENTITY;

          ENTITY IfcElement
            SUBTYPE OF (IfcRoot);
            Tag : OPTIONAL IfcLabel;
          END_ENTITY;

          ENTITY IfcWall
            SUBTYPE OF (IfcElement);
            PredefinedType : OPTIONAL IfcWallTypeEnum;
            RelatedObjects : SET [1:?] OF IfcRoot;
          END_ENTITY;
        END_SCHEMA;
        "#
    }

    #[test]
    fn load_express_builds_schema_doc_from_fixture() {
        let schema =
            load_express(IfcVersion::Ifc4Add2Tc1, fixture_schema()).expect("schema should parse");
        let wall = schema.entity("IFCWALL").expect("IfcWall should resolve");

        assert_eq!(schema.schema_name, "DEMO");
        assert_eq!(wall.name, "IfcWall");
        assert_eq!(wall.attributes.len(), 4);
        assert_eq!(wall.attributes[0].name, "GlobalId");
        assert_eq!(wall.attributes[0].declared_in, "IfcRoot");
        assert_eq!(wall.attributes[2].type_name, "OPTIONAL IfcWallTypeEnum");
        assert_eq!(wall.attributes[3].type_name, "SET [1:?] OF IfcRoot");
        assert!(wall.url.contains("ifcwall.htm"));

        let TypeDoc::Select(select) = schema
            .type_decl("IfcObjectReferenceSelect")
            .expect("select type should resolve")
        else {
            panic!("expected select type");
        };
        assert_eq!(select.options.len(), 2);
    }

    #[test]
    fn schema_doc_knows_subtype_compatibility() {
        let schema =
            load_express(IfcVersion::Ifc4Add2Tc1, fixture_schema()).expect("schema should parse");

        assert!(schema.is_entity_compatible("IFCWALL", "IFCROOT"));
        assert!(schema.is_entity_compatible("IFCWALL", "IFCELEMENT"));
        assert!(!schema.is_entity_compatible("IFCROOT", "IFCWALL"));
    }

    #[test]
    fn load_express_preserves_derived_attribute_overrides() {
        let source = r#"
        SCHEMA DEMO;
          TYPE IfcUnitEnum = ENUMERATION OF (LENGTHUNIT);
          END_TYPE;
          TYPE IfcSIPrefix = ENUMERATION OF (MILLI);
          END_TYPE;
          TYPE IfcSIUnitName = ENUMERATION OF (METRE);
          END_TYPE;
          ENTITY IfcDimensionalExponents;
          END_ENTITY;
          ENTITY IfcNamedUnit;
            Dimensions : IfcDimensionalExponents;
            UnitType : IfcUnitEnum;
          END_ENTITY;
          ENTITY IfcSIUnit
            SUBTYPE OF (IfcNamedUnit);
            Prefix : OPTIONAL IfcSIPrefix;
            Name : IfcSIUnitName;
          DERIVE
            SELF\IfcNamedUnit.Dimensions : IfcDimensionalExponents := ?;
          END_ENTITY;
        END_SCHEMA;
        "#;

        let schema = load_express(IfcVersion::Ifc4Add2Tc1, source).expect("schema should parse");
        let unit = schema
            .entity("IFCSIUNIT")
            .expect("IfcSIUnit should resolve");
        let dimensions = unit
            .attributes
            .iter()
            .find(|attribute| attribute.name == "Dimensions")
            .expect("Dimensions should be inherited");

        assert!(dimensions.allows_omitted);
    }

    #[test]
    fn sanitize_drops_global_algorithm_blocks() {
        let source = r#"
        SCHEMA DEMO;
          TYPE IfcLabel = STRING;
          END_TYPE;
          FUNCTION UnsupportedFunction(Value : IfcLabel) : BOOLEAN;
            RETURN(TRUE);
          END_FUNCTION;
          RULE UnsupportedRule FOR (IfcRoot);
            WHERE WR1 : TRUE;
          END_RULE;
          ENTITY IfcRoot;
            GlobalId : IfcLabel;
          END_ENTITY;
        END_SCHEMA;
        "#;

        let schema = load_express(IfcVersion::Ifc4Add2Tc1, source).expect("schema should parse");

        assert!(schema.entity("IfcRoot").is_some());
    }

    #[test]
    #[ignore = "requires network access to buildingSMART"]
    fn load_official_schemas_from_buildingsmart() {
        let collection = SchemaDocCollection::new();

        assert!(
            collection.load_errors().is_empty(),
            "{:?}",
            collection.load_errors()
        );
        for version in IfcVersion::supported() {
            assert!(
                collection
                    .get(version)
                    .and_then(|schema| schema.entity("IFCWALL"))
                    .is_some(),
                "missing IfcWall for {version}"
            );
        }
    }
}
