//! Hover feature logic.
//! Resolves the syntax node under the cursor and renders either schema-backed entity information
//! or local reference previews from the current document.
//! Hover stays synchronous by reading only the in-memory document and schema collections.

use tower_lsp::lsp_types::{Hover, HoverContents, MarkupContent, MarkupKind, Position};

use crate::document::{DefinitionInfo, Document};
use crate::schema::{EntityAttributeDoc, EntityDoc, SchemaDocCollection};

pub fn hover(
    document: &Document,
    position: Position,
    schema_docs: &SchemaDocCollection,
) -> Option<Hover> {
    document.tree.as_ref()?;

    if let Some(node) = document.node_at_position(position) {
        if node.kind() == "entity_name" {
            let entity_text = node.utf8_text(document.text.as_bytes()).ok()?;
            if let Some(schema_name) = document.schema_name.as_deref()
                && let Some(entity_doc) = schema_docs.get_entity_doc(schema_name, entity_text)
            {
                return Some(Hover {
                    contents: HoverContents::Markup(MarkupContent {
                        kind: MarkupKind::Markdown,
                        value: render_entity_hover(entity_doc),
                    }),
                    range: None,
                });
            }
        } else if node.kind() == "reference" {
            let reference_text = node.utf8_text(document.text.as_bytes()).ok()?;
            let id = reference_text.trim_start_matches('#').parse::<u32>().ok()?;
            let definition = document.definitions.get(&id)?;

            return Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: render_reference_hover(document, definition)?,
                }),
                range: Some(node_range(&node)),
            });
        }
    }

    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: "Hover over an IFC entity name (like `IFCWALL`) to see documentation."
                .to_string(),
        }),
        range: None,
    })
}

fn render_entity_hover(entity_doc: &EntityDoc) -> String {
    let mut markdown = format!("# {}", entity_doc.name);

    let inherited_attributes: Vec<_> = entity_doc
        .attributes
        .iter()
        .filter(|attribute| attribute.declared_in != entity_doc.name)
        .collect();
    let direct_attributes: Vec<_> = entity_doc
        .attributes
        .iter()
        .filter(|attribute| attribute.declared_in == entity_doc.name)
        .collect();

    if !inherited_attributes.is_empty() {
        markdown.push_str("\n\n## Inherited Attributes");
        markdown.push_str(&render_attribute_table(&inherited_attributes, 1));
    }

    markdown.push_str("\n\n## Attributes Declared In This Entity");
    markdown.push_str(&render_attribute_table(
        &direct_attributes,
        inherited_attributes.len() + 1,
    ));

    markdown.push_str(&format!("\n\n[Official documentation]({})", entity_doc.url));

    markdown
}

fn render_attribute_table(attributes: &[&EntityAttributeDoc], start_index: usize) -> String {
    let mut markdown = String::new();

    markdown.push_str("\n\n| # | Attribute | Type |\n| --- | --- | --- |");
    for (index, attribute) in attributes.iter().enumerate() {
        markdown.push_str(&format!(
            "\n| {} | {} | {} |",
            start_index + index,
            format_attribute_name(&attribute.name),
            format_attribute_type(&attribute.type_name),
        ));
    }

    markdown
}

fn render_reference_hover(document: &Document, definition: &DefinitionInfo) -> Option<String> {
    let preview = extract_range_text(document, definition.entity_range)?;

    Some(format!("```ifc\n{}\n```", preview.trim()))
}

fn extract_range_text(document: &Document, range: tower_lsp::lsp_types::Range) -> Option<&str> {
    let start = offset_at_position(&document.text, range.start)?;
    let end = offset_at_position(&document.text, range.end)?;
    document.text.get(start..end)
}

fn offset_at_position(text: &str, position: Position) -> Option<usize> {
    let mut offset = 0usize;
    let mut lines = text.split('\n');

    for _ in 0..position.line {
        let line = lines.next()?;
        offset += line.len() + 1;
    }

    let line = lines.next()?;
    let character = position.character as usize;
    if character > line.len() {
        return None;
    }

    Some(offset + character)
}

fn node_range(node: &tree_sitter::Node<'_>) -> tower_lsp::lsp_types::Range {
    let start = node.start_position();
    let end = node.end_position();

    tower_lsp::lsp_types::Range {
        start: Position::new(start.row as u32, start.column as u32),
        end: Position::new(end.row as u32, end.column as u32),
    }
}

fn escape_table_cell(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace('|', "\\|")
}

fn format_attribute_name(name: &str) -> String {
    format!("*{}*", escape_table_cell(name))
}

fn format_attribute_type(type_name: &str) -> String {
    format!("`{}`", escape_table_cell(type_name).replace('`', "\\`"))
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use super::*;
    use crate::schema::EntityAttributeDoc;

    fn parse_document(text: &str) -> Document {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_ifc::LANGUAGE.into())
            .expect("Error loading IFC parser");

        Document::parse(&mut parser, text.to_string())
    }

    fn position_at(text: &str, needle: &str) -> Position {
        let offset = text.find(needle).expect("needle should exist") as u32;
        Position::new(0, offset)
    }

    fn position_at_last(text: &str, needle: &str) -> Position {
        let offset = text.rfind(needle).expect("needle should exist") as u32;
        let prefix = &text[..offset as usize];
        let line = prefix.bytes().filter(|&b| b == b'\n').count() as u32;
        let column = prefix
            .rsplit_once('\n')
            .map(|(_, tail)| tail.len() as u32)
            .unwrap_or(offset);
        Position::new(line, column)
    }

    fn hover_text(hover: Hover) -> String {
        match hover.contents {
            HoverContents::Markup(markup) => markup.value,
            contents => panic!("unexpected hover contents: {contents:?}"),
        }
    }

    fn empty_schema_docs() -> SchemaDocCollection {
        SchemaDocCollection::empty()
    }

    fn schema_docs_with_wall() -> SchemaDocCollection {
        let source = r#"
        SCHEMA IFC4;
          TYPE IfcGloballyUniqueId = STRING(22) FIXED;
          END_TYPE;
          TYPE IfcWallTypeEnum = ENUMERATION OF (MOVABLE, USERDEFINED);
          END_TYPE;
          ENTITY IfcRoot;
            GlobalId : IfcGloballyUniqueId;
          END_ENTITY;
          ENTITY IfcWall
            SUBTYPE OF (IfcRoot);
            PredefinedType : OPTIONAL IfcWallTypeEnum;
          END_ENTITY;
        END_SCHEMA;
        "#;
        let schema = crate::schema::load_express(crate::schema::IfcVersion::Ifc4Add2Tc1, source)
            .expect("fixture schema should parse");
        SchemaDocCollection::from_docs([("IFC4".to_string(), schema)])
    }

    #[test]
    fn hover_returns_schema_docs_for_entity_names_with_detected_version() {
        let text = "ISO-10303-21;HEADER;FILE_SCHEMA(('IFC4'));ENDSEC;DATA;#1=IFCWALL($);ENDSEC;END-ISO-10303-21;";
        let document = parse_document(text);

        let hover = hover(
            &document,
            position_at(text, "IFCWALL"),
            &schema_docs_with_wall(),
        )
        .expect("hover should exist");
        let value = hover_text(hover);

        assert!(value.contains("# IfcWall"));
        assert!(value.contains("Official documentation"));
    }

    #[test]
    fn hover_returns_generic_message_for_non_entity_name_nodes() {
        let text = "#1=IFCWALL($);";
        let document = parse_document(text);

        let hover =
            hover(&document, position_at(text, "#1"), &empty_schema_docs()).expect("hover exists");
        let value = hover_text(hover);

        assert!(value.contains("Hover over an IFC entity name"));
    }

    #[test]
    fn hover_returns_definition_preview_for_references() {
        let text = "#1=IFCWALL($);\n#2=IFCDOOR(#1);";
        let document = parse_document(text);

        let hover = hover(
            &document,
            position_at_last(text, "#1"),
            &empty_schema_docs(),
        )
        .expect("hover exists");
        let value = hover_text(hover);

        assert!(value.contains("#1=IFCWALL($);"));
    }

    #[test]
    fn hover_does_not_return_definition_preview_for_definition_ids() {
        let text = "#1=IFCWALL($);";
        let document = parse_document(text);

        let hover =
            hover(&document, position_at(text, "#1"), &empty_schema_docs()).expect("hover exists");
        let value = hover_text(hover);

        assert!(value.contains("Hover over an IFC entity name"));
        assert!(!value.contains("Reference target"));
    }

    #[test]
    fn hover_returns_none_without_a_syntax_tree() {
        let document = Document {
            text: "#1=IFCWALL($);".to_string(),
            tree: None,
            schema_name: None,
            version: None,
            definitions: HashMap::new(),
            references: HashMap::new(),
            instances: Vec::new(),
        };

        assert!(hover(&document, Position::new(0, 0), &empty_schema_docs()).is_none());
    }

    #[test]
    fn render_entity_hover_renders_inherited_and_direct_attribute_tables() {
        let entity = EntityDoc {
            name: "IfcWall".to_string(),
            attributes: vec![
                EntityAttributeDoc {
                    name: "GlobalId".to_string(),
                    type_name: "IfcGloballyUniqueId".to_string(),
                    declared_in: "IfcRoot".to_string(),
                    ty: crate::schema::TypeRef::Primitive(crate::schema::PrimitiveType::String {
                        width: None,
                        fixed: false,
                    }),
                    optional: false,
                    allows_omitted: false,
                },
                EntityAttributeDoc {
                    name: "PredefinedType".to_string(),
                    type_name: "OPTIONAL IfcWallTypeEnum".to_string(),
                    declared_in: "IfcWall".to_string(),
                    ty: crate::schema::TypeRef::Named(crate::schema::NamedTypeRef {
                        name: "IFCWALLTYPEENUM".to_string(),
                        kind: crate::schema::NamedTypeKind::Type,
                    }),
                    optional: true,
                    allows_omitted: false,
                },
            ],
            url: "https://example.invalid/IfcWall.htm".to_string(),
            all_supertypes: HashSet::new(),
        };

        let markdown = render_entity_hover(&entity);

        assert!(markdown.contains("## Inherited Attributes"));
        assert!(markdown.contains("| 1 | *GlobalId* | `IfcGloballyUniqueId` |"));
        assert!(markdown.contains("## Attributes Declared In This Entity"));
        assert!(markdown.contains("| 2 | *PredefinedType* | `OPTIONAL IfcWallTypeEnum` |"));
        assert!(!markdown.contains("Declared In |"));
        assert!(markdown.contains("[Official documentation](https://example.invalid/IfcWall.htm)"));
    }

    #[test]
    fn render_entity_hover_omits_empty_inherited_table() {
        let entity = EntityDoc {
            name: "IfcRoot".to_string(),
            attributes: vec![EntityAttributeDoc {
                name: "GlobalId".to_string(),
                type_name: "IfcGloballyUniqueId".to_string(),
                declared_in: "IfcRoot".to_string(),
                ty: crate::schema::TypeRef::Primitive(crate::schema::PrimitiveType::String {
                    width: None,
                    fixed: false,
                }),
                optional: false,
                allows_omitted: false,
            }],
            url: "https://example.invalid/IfcRoot.htm".to_string(),
            all_supertypes: HashSet::new(),
        };

        let markdown = render_entity_hover(&entity);

        assert!(!markdown.contains("## Inherited Attributes"));
        assert!(markdown.contains("## Attributes Declared In This Entity"));
        assert!(markdown.contains("| 1 | *GlobalId* | `IfcGloballyUniqueId` |"));
    }
}
