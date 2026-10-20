//! Lightweight STEP and schema-aware completions.
//! Completion context is derived from document text and schema docs without loading AST state.

use std::collections::HashSet;

use tower_lsp::lsp_types::{
    CompletionItem, CompletionItemKind, CompletionResponse, CompletionTextEdit, InsertTextFormat,
    Position, Range, TextEdit,
};

use crate::document::Document;
use crate::schema::{NamedTypeKind, SchemaDoc, TypeDoc, TypeRef, normalize_name};

use super::ifc_guid::{IFC_GUID_ALPHABET, new_guid};
use super::step_scan::{entity_argument_context, scan_definition_entity_name};

pub fn completions(
    document: &Document,
    position: Position,
    schema: Option<&SchemaDoc>,
) -> Option<CompletionResponse> {
    let offset = document.position_to_offset(position)?;
    let mut items = Vec::new();

    if let Some(item) = next_id_completion(document, position, offset) {
        items.push(item);
    }

    if let Some(schema) = schema {
        items.extend(entity_name_completions(document, position, offset, schema));
        items.extend(argument_completions(document, position, offset, schema));
    }

    (!items.is_empty()).then_some(CompletionResponse::Array(items))
}

fn next_id_completion(
    document: &Document,
    position: Position,
    offset: usize,
) -> Option<CompletionItem> {
    let line_start = *document.line_offsets.get(position.line as usize)?;
    let line_end = document.line_end_offset(position.line as usize)?;
    let line_prefix = document.text.get(line_start..offset)?;
    let line_suffix = document.text.get(offset..line_end)?;
    if !line_suffix.trim().is_empty() {
        return None;
    }
    let candidate = line_prefix.trim_start();
    let replace_start = if candidate.is_empty() {
        offset
    } else if candidate
        .strip_prefix('#')
        .is_some_and(|digits| digits.bytes().all(|byte| byte.is_ascii_digit()))
    {
        line_start + line_prefix.len().checked_sub(candidate.len())?
    } else {
        return None;
    };

    let next_id = document
        .definitions
        .keys()
        .copied()
        .max()
        .map_or(Some(1), |id| id.checked_add(1))?;
    let range = document.range_for_offsets(replace_start, offset)?;
    let mut item = completion_item(
        format!("#{next_id}="),
        "Next STEP instance id",
        CompletionItemKind::VALUE,
        format!("#{next_id}="),
        range,
        "000_next_id".to_string(),
    );
    item.filter_text = Some(format!("#{next_id}"));
    item.insert_text_format = Some(InsertTextFormat::PLAIN_TEXT);
    Some(item)
}

fn entity_name_completions(
    document: &Document,
    position: Position,
    offset: usize,
    schema: &SchemaDoc,
) -> Vec<CompletionItem> {
    let Some((prefix, range)) = entity_name_request(document, position, offset) else {
        return Vec::new();
    };

    let mut names = schema.entities.keys().cloned().collect::<Vec<_>>();
    names.sort();
    names
        .into_iter()
        .filter(|name| starts_with_ignore_ascii_case(name, &prefix))
        .enumerate()
        .map(|(index, name)| {
            completion_item(
                name.clone(),
                "IFC entity",
                CompletionItemKind::CLASS,
                name,
                range,
                format!("{index:06}_entity"),
            )
        })
        .collect()
}

fn entity_name_request(
    document: &Document,
    position: Position,
    offset: usize,
) -> Option<(String, Range)> {
    let line_start = *document.line_offsets.get(position.line as usize)?;
    let bytes = document.text.as_bytes();
    let mut prefix_start = offset;
    while prefix_start > line_start && is_identifier_part(bytes[prefix_start - 1]) {
        prefix_start -= 1;
    }
    let prefix = document
        .text
        .get(prefix_start..offset)?
        .to_ascii_uppercase();

    let mut before = prefix_start;
    while before > line_start && bytes[before - 1].is_ascii_whitespace() {
        before -= 1;
    }
    if before == line_start || bytes[before - 1] != b'=' {
        return None;
    }
    before -= 1;
    while before > line_start && bytes[before - 1].is_ascii_whitespace() {
        before -= 1;
    }
    let digit_end = before;
    while before > line_start && bytes[before - 1].is_ascii_digit() {
        before -= 1;
    }
    if before == digit_end || before == line_start || bytes[before - 1] != b'#' {
        return None;
    }

    Some((prefix, document.range_for_offsets(prefix_start, offset)?))
}

fn argument_completions(
    document: &Document,
    _position: Position,
    offset: usize,
    schema: &SchemaDoc,
) -> Vec<CompletionItem> {
    let Some(context) = entity_argument_context(&document.text, offset) else {
        return Vec::new();
    };
    let Some(entity) = schema.entity(&context.entity_name) else {
        return Vec::new();
    };
    let Some(attribute) = entity.attributes.get(context.active_parameter) else {
        return Vec::new();
    };
    let Some(token) = argument_token(document, context.value_start, offset) else {
        return Vec::new();
    };

    if type_is_guid(
        schema,
        &attribute.ty,
        context.aggregate_depth,
        &mut HashSet::new(),
    ) {
        return guid_completions(token);
    }

    if let Some(values) = enumeration_values(
        schema,
        &attribute.ty,
        context.aggregate_depth,
        &mut HashSet::new(),
    ) {
        return enumeration_completions(values, token);
    }

    let mut expected_entities = HashSet::new();
    let mut accepts_any_entity = false;
    collect_entity_constraints(
        schema,
        &attribute.ty,
        context.aggregate_depth,
        &mut HashSet::new(),
        &mut expected_entities,
        &mut accepts_any_entity,
    );
    reference_completions(
        document,
        schema,
        &expected_entities,
        accepts_any_entity,
        token,
    )
}

#[derive(Clone, Copy)]
struct ArgumentToken<'a> {
    text: &'a str,
    range: Range,
}

fn argument_token(
    document: &Document,
    value_start: usize,
    cursor_offset: usize,
) -> Option<ArgumentToken<'_>> {
    let raw = document.text.get(value_start..cursor_offset)?;
    let leading = raw.len().checked_sub(raw.trim_start().len())?;
    let token_start = value_start + leading;
    let text = document.text.get(token_start..cursor_offset)?;
    if text.bytes().any(|byte| byte.is_ascii_whitespace()) {
        return None;
    }

    Some(ArgumentToken {
        text,
        range: document.range_for_offsets(token_start, cursor_offset)?,
    })
}

fn guid_completions(token: ArgumentToken<'_>) -> Vec<CompletionItem> {
    let valid_partial = token.text.is_empty()
        || token.text.strip_prefix('\'').is_some_and(|value| {
            !value.contains('\'') && value.bytes().all(|byte| IFC_GUID_ALPHABET.contains(&byte))
        });
    if !valid_partial {
        return Vec::new();
    }

    let guid = format!("'{}'", new_guid());
    vec![completion_item(
        "Generate fresh IFC GUID".to_string(),
        "Fresh compressed UUID v4",
        CompletionItemKind::VALUE,
        guid,
        token.range,
        "000_guid".to_string(),
    )]
}

fn enumeration_completions(values: &[String], token: ArgumentToken<'_>) -> Vec<CompletionItem> {
    let prefix = if token.text.is_empty() {
        ""
    } else if let Some(value) = token.text.strip_prefix('.') {
        if value.contains('.') || !value.bytes().all(is_identifier_part) {
            return Vec::new();
        }
        value
    } else {
        return Vec::new();
    };

    let mut values = values.to_vec();
    values.sort();
    values
        .into_iter()
        .filter(|value| starts_with_ignore_ascii_case(value, prefix))
        .enumerate()
        .map(|(index, value)| {
            let step_value = format!(".{value}.");
            completion_item(
                step_value.clone(),
                "IFC enumeration value",
                CompletionItemKind::ENUM_MEMBER,
                step_value,
                token.range,
                format!("{index:06}_enum"),
            )
        })
        .collect()
}

fn reference_completions(
    document: &Document,
    schema: &SchemaDoc,
    expected_entities: &HashSet<String>,
    accepts_any_entity: bool,
    token: ArgumentToken<'_>,
) -> Vec<CompletionItem> {
    if !accepts_any_entity && expected_entities.is_empty() {
        return Vec::new();
    }
    let prefix = if token.text.is_empty() {
        ""
    } else if let Some(value) = token.text.strip_prefix('#') {
        if !value.bytes().all(|byte| byte.is_ascii_digit()) {
            return Vec::new();
        }
        value
    } else {
        return Vec::new();
    };

    let mut definitions = document
        .definitions
        .iter()
        .map(|(id, offset)| (*id, *offset))
        .collect::<Vec<_>>();
    definitions.sort_by_key(|(id, _)| *id);

    definitions
        .into_iter()
        .filter_map(|(id, offset)| {
            let id_text = id.to_string();
            if !id_text.starts_with(prefix) {
                return None;
            }
            let entity_name =
                scan_definition_entity_name(&document.text, offset, document.text.len())?;
            if !accepts_any_entity
                && !expected_entities
                    .iter()
                    .any(|expected| schema.is_entity_compatible(&entity_name, expected))
            {
                return None;
            }

            let reference = format!("#{id}");
            Some(completion_item(
                reference.clone(),
                &entity_name,
                CompletionItemKind::REFERENCE,
                reference,
                token.range,
                format!("{id:010}_reference"),
            ))
        })
        .collect()
}

fn type_is_guid(
    schema: &SchemaDoc,
    ty: &TypeRef,
    aggregate_depth: usize,
    visiting: &mut HashSet<String>,
) -> bool {
    match ty {
        TypeRef::Aggregate(aggregate) if aggregate_depth > 0 => {
            type_is_guid(schema, &aggregate.item, aggregate_depth - 1, visiting)
        }
        TypeRef::Aggregate(_) => false,
        TypeRef::Named(named) => {
            let name = normalize_name(&named.name);
            if aggregate_depth == 0 && name == "IFCGLOBALLYUNIQUEID" {
                return true;
            }
            if !visiting.insert(name.clone()) {
                return false;
            }
            let result = match schema.type_decl(&name) {
                Some(TypeDoc::Alias(alias)) => {
                    type_is_guid(schema, &alias.target, aggregate_depth, visiting)
                }
                _ => false,
            };
            visiting.remove(&name);
            result
        }
        _ => false,
    }
}

fn enumeration_values<'a>(
    schema: &'a SchemaDoc,
    ty: &'a TypeRef,
    aggregate_depth: usize,
    visiting: &mut HashSet<String>,
) -> Option<&'a [String]> {
    match ty {
        TypeRef::Aggregate(aggregate) if aggregate_depth > 0 => {
            enumeration_values(schema, &aggregate.item, aggregate_depth - 1, visiting)
        }
        TypeRef::Aggregate(_) => None,
        TypeRef::Named(named) if named.kind != NamedTypeKind::Entity => {
            let name = normalize_name(&named.name);
            if !visiting.insert(name.clone()) {
                return None;
            }
            let result = match schema.type_decl(&name) {
                Some(TypeDoc::Alias(alias)) => {
                    enumeration_values(schema, &alias.target, aggregate_depth, visiting)
                }
                Some(TypeDoc::Enumeration(enumeration)) if aggregate_depth == 0 => {
                    Some(enumeration.items.as_slice())
                }
                _ => None,
            };
            visiting.remove(&name);
            result
        }
        _ => None,
    }
}

fn collect_entity_constraints(
    schema: &SchemaDoc,
    ty: &TypeRef,
    aggregate_depth: usize,
    visiting: &mut HashSet<String>,
    expected: &mut HashSet<String>,
    accepts_any: &mut bool,
) {
    match ty {
        TypeRef::Aggregate(aggregate) if aggregate_depth > 0 => collect_entity_constraints(
            schema,
            &aggregate.item,
            aggregate_depth - 1,
            visiting,
            expected,
            accepts_any,
        ),
        TypeRef::Aggregate(_) => {}
        TypeRef::GenericEntity { .. } if aggregate_depth == 0 => *accepts_any = true,
        TypeRef::Named(named) if aggregate_depth == 0 => {
            let name = normalize_name(&named.name);
            if named.kind == NamedTypeKind::Entity || schema.entity(&name).is_some() {
                expected.insert(name);
                return;
            }
            if !visiting.insert(name.clone()) {
                return;
            }
            match schema.type_decl(&name) {
                Some(TypeDoc::Alias(alias)) => collect_entity_constraints(
                    schema,
                    &alias.target,
                    0,
                    visiting,
                    expected,
                    accepts_any,
                ),
                Some(TypeDoc::Select(select)) => {
                    if select.generic_entity {
                        *accepts_any = true;
                    }
                    for option in &select.options {
                        collect_entity_constraints(
                            schema,
                            option,
                            0,
                            visiting,
                            expected,
                            accepts_any,
                        );
                    }
                }
                _ => {}
            }
            visiting.remove(&name);
        }
        _ => {}
    }
}

fn completion_item(
    label: String,
    detail: &str,
    kind: CompletionItemKind,
    new_text: String,
    range: Range,
    sort_text: String,
) -> CompletionItem {
    let mut item = CompletionItem::new_simple(label, detail.to_string());
    item.kind = Some(kind);
    item.sort_text = Some(sort_text);
    item.text_edit = Some(CompletionTextEdit::Edit(TextEdit::new(range, new_text)));
    item
}

fn starts_with_ignore_ascii_case(value: &str, prefix: &str) -> bool {
    value
        .get(..prefix.len())
        .is_some_and(|start| start.eq_ignore_ascii_case(prefix))
}

fn is_identifier_part(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use crate::schema::{
        AggregateKind, AggregateTypeRef, AliasTypeDef, EntityAttributeDoc, EntityDoc,
        EnumerationTypeDef, NamedTypeRef, SelectTypeDef,
    };

    use super::*;

    fn named(name: &str, kind: NamedTypeKind) -> TypeRef {
        TypeRef::Named(NamedTypeRef {
            name: name.to_string(),
            kind,
        })
    }

    fn attribute(name: &str, ty: TypeRef) -> EntityAttributeDoc {
        EntityAttributeDoc {
            name: name.to_string(),
            type_name: name.to_string(),
            declared_in: "IfcWall".to_string(),
            ty,
            optional: false,
            allows_omitted: false,
        }
    }

    fn entity(name: &str, attributes: Vec<EntityAttributeDoc>, supers: &[&str]) -> EntityDoc {
        EntityDoc {
            name: name.to_string(),
            attributes,
            url: String::new(),
            all_supertypes: supers.iter().map(|name| normalize_name(name)).collect(),
        }
    }

    fn schema() -> SchemaDoc {
        let aggregate = TypeRef::Aggregate(AggregateTypeRef {
            kind: AggregateKind::Set,
            bounds: None,
            item: Box::new(named("IfcRoot", NamedTypeKind::Entity)),
            unique: false,
            optional: false,
        });
        SchemaDoc {
            entities: HashMap::from([
                (
                    "IFCWALL".to_string(),
                    entity(
                        "IfcWall",
                        vec![
                            attribute(
                                "GlobalId",
                                named("IfcGloballyUniqueId", NamedTypeKind::Type),
                            ),
                            attribute(
                                "OwnerHistory",
                                named("IfcOwnerHistory", NamedTypeKind::Entity),
                            ),
                            attribute(
                                "PredefinedType",
                                named("IfcWallTypeEnum", NamedTypeKind::Type),
                            ),
                            attribute("Related", aggregate),
                            attribute("Choice", named("IfcRootSelect", NamedTypeKind::Type)),
                        ],
                        &["IfcRoot"],
                    ),
                ),
                (
                    "IFCWALLSTANDARDCASE".to_string(),
                    entity("IfcWallStandardCase", Vec::new(), &["IfcWall", "IfcRoot"]),
                ),
                (
                    "IFCDOOR".to_string(),
                    entity("IfcDoor", Vec::new(), &["IfcRoot"]),
                ),
                (
                    "IFCOWNERHISTORY".to_string(),
                    entity("IfcOwnerHistory", Vec::new(), &[]),
                ),
            ]),
            types: HashMap::from([
                (
                    "IFCGLOBALLYUNIQUEID".to_string(),
                    TypeDoc::Alias(AliasTypeDef {
                        name: "IfcGloballyUniqueId".to_string(),
                        target: TypeRef::Primitive(crate::schema::PrimitiveType::String {
                            width: Some(22),
                            fixed: true,
                        }),
                    }),
                ),
                (
                    "IFCWALLTYPEENUM".to_string(),
                    TypeDoc::Enumeration(EnumerationTypeDef {
                        name: "IfcWallTypeEnum".to_string(),
                        items: vec!["USERDEFINED".to_string(), "SOLIDWALL".to_string()],
                        extensible: true,
                    }),
                ),
                (
                    "IFCROOTSELECT".to_string(),
                    TypeDoc::Select(SelectTypeDef {
                        name: "IfcRootSelect".to_string(),
                        options: vec![named("IfcWall", NamedTypeKind::Entity)],
                        extensible: false,
                        generic_entity: false,
                    }),
                ),
            ]),
        }
    }

    fn items(text: &str, schema: Option<&SchemaDoc>) -> Vec<CompletionItem> {
        let document = Document::new_unloaded(text.to_string());
        let position = document
            .offset_to_position(text.len())
            .expect("end position should be valid");
        match completions(&document, position, schema) {
            Some(CompletionResponse::Array(items)) => items,
            _ => Vec::new(),
        }
    }

    fn new_text(item: &CompletionItem) -> &str {
        match item.text_edit.as_ref().expect("expected text edit") {
            CompletionTextEdit::Edit(edit) => &edit.new_text,
            CompletionTextEdit::InsertAndReplace(_) => panic!("expected plain text edit"),
        }
    }

    #[test]
    fn suggests_next_definition_prefix_on_empty_line() {
        let results = items("#1=IFCWALL($);\n#4=IFCDOOR($);\n", None);
        assert_eq!(new_text(&results[0]), "#5=");
    }

    #[test]
    fn suggests_first_definition_prefix_without_definitions() {
        let results = items("", None);
        assert_eq!(new_text(&results[0]), "#1=");
    }

    #[test]
    fn replaces_typed_id_prefix_without_adding_whitespace() {
        for prefix in ["#", "#1"] {
            let text = format!("#1=IFCWALL($);\n#16=IFCDOOR($);\n  {prefix}");
            let results = items(&text, None);

            assert_eq!(results.len(), 1);
            assert_eq!(new_text(&results[0]), "#17=");
            assert!(!new_text(&results[0]).contains(' '));
            let range = match results[0].text_edit.as_ref().unwrap() {
                CompletionTextEdit::Edit(edit) => edit.range,
                CompletionTextEdit::InsertAndReplace(_) => panic!("expected plain text edit"),
            };
            assert_eq!(range.start.character, 2);
        }
    }

    #[test]
    fn completes_entity_names_in_definition_position() {
        let results = items("#1=ifcw", Some(&schema()));
        let edits = results.iter().map(new_text).collect::<Vec<_>>();
        assert_eq!(edits, ["IFCWALL", "IFCWALLSTANDARDCASE"]);
    }

    #[test]
    fn entity_completion_edit_range_uses_utf16_positions() {
        let text = "/* 😀 */ #1=IFCW";
        let results = items(text, Some(&schema()));
        let range = match results[0].text_edit.as_ref().unwrap() {
            CompletionTextEdit::Edit(edit) => edit.range,
            CompletionTextEdit::InsertAndReplace(_) => panic!("expected plain text edit"),
        };

        assert_eq!(range.start.character, 12);
        assert_eq!(range.end.character, 16);
    }

    #[test]
    fn generates_quoted_guid_for_empty_and_partial_values() {
        for text in ["#1=IFCWALL(", "#1=IFCWALL('abc"] {
            let results = items(text, Some(&schema()));
            let value = new_text(&results[0]);
            assert_eq!(value.len(), 24);
            assert!(value.starts_with('\'') && value.ends_with('\''));
            assert!(
                value[1..23]
                    .bytes()
                    .all(|byte| IFC_GUID_ALPHABET.contains(&byte))
            );
        }
    }

    #[test]
    fn does_not_replace_complete_or_null_guid_values() {
        assert!(items("#1=IFCWALL('0123456789012345678901'", Some(&schema())).is_empty());
        assert!(items("#1=IFCWALL($", Some(&schema())).is_empty());
    }

    #[test]
    fn completes_enumeration_prefix() {
        let results = items("#1=IFCWALL('done',$,.SO", Some(&schema()));
        assert_eq!(results.len(), 1);
        assert_eq!(new_text(&results[0]), ".SOLIDWALL.");
    }

    #[test]
    fn completes_after_a_leading_argument_comment() {
        let results = items(
            "#1=IFCWALL('done',$, /* explanation */ .SO",
            Some(&schema()),
        );
        assert_eq!(results.len(), 1);
        assert_eq!(new_text(&results[0]), ".SOLIDWALL.");
    }

    #[test]
    fn completes_compatible_references_inside_aggregate() {
        let text = "#1=IFCWALL($);#2=IFCDOOR($);#3=IFCOWNERHISTORY($);#4=IFCWALL($,$,$,(#";
        let results = items(text, Some(&schema()));
        let labels = results
            .iter()
            .map(|item| item.label.as_str())
            .collect::<Vec<_>>();
        assert_eq!(labels, ["#1", "#2", "#4"]);
    }

    #[test]
    fn filters_select_references_to_compatible_entities() {
        let text = "#1=IFCWALL($);#2=IFCDOOR($);#3=IFCWALL($,$,$,$,#";
        let results = items(text, Some(&schema()));
        assert_eq!(
            results.iter().map(new_text).collect::<Vec<_>>(),
            ["#1", "#3"]
        );
    }

    #[test]
    fn suggests_owner_history_after_comma_and_hash_trigger() {
        for suffix in ["#17=IFCWALL('fresh',", "#17=IFCWALL('fresh', #"] {
            let text = format!("#1=IFCPROJECT($);#2=IFCOWNERHISTORY($);#3=IFCDOOR($);{suffix}");
            let results = items(&text, Some(&schema()));

            assert_eq!(results.len(), 1);
            assert_eq!(results[0].label, "#2");
            assert_eq!(new_text(&results[0]), "#2");
            assert!(!new_text(&results[0]).contains(' '));
        }
    }

    #[test]
    fn bundled_ifc4x3_schema_suggests_owner_history_for_wall() {
        let schemas = crate::schema::SchemaDocCollection::new();
        let schema = schemas
            .get("IFC4X3_ADD2")
            .expect("bundled IFC4X3 schema should load");
        let text = "#2=IFCOWNERHISTORY($,$,$,$,$,$,$,$);#17=IFCWALL('fresh', #";
        let results = items(text, Some(schema));

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].label, "#2");
        assert_eq!(new_text(&results[0]), "#2");
    }

    #[test]
    fn hash_inside_argument_does_not_offer_next_definition_id() {
        let text = "#2=IFCOWNERHISTORY($);\n#17=IFCWALL('fresh', #";
        let results = items(text, Some(&schema()));
        let edits = results.iter().map(new_text).collect::<Vec<_>>();

        assert_eq!(edits, ["#2"]);
        assert!(!edits.contains(&"#18="));
    }

    #[test]
    fn schema_aware_completions_do_not_require_ast() {
        let mut document = Document::new_unloaded("#1=IFCWALL(".to_string());
        document.ast_skipped = true;
        let position = document.offset_to_position(document.text.len()).unwrap();
        assert!(completions(&document, position, Some(&schema())).is_some());
    }

    #[test]
    fn ignores_ids_in_strings_when_computing_next_id() {
        let results = items("'#99='\n", None);
        assert_eq!(new_text(&results[0]), "#1=");
    }

    #[test]
    fn test_schema_has_no_accidental_duplicate_supertypes() {
        let schema = schema();
        let supers = &schema.entity("IfcWall").unwrap().all_supertypes;
        assert_eq!(supers, &HashSet::from(["IFCROOT".to_string()]));
    }
}
