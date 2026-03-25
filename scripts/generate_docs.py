#!/usr/bin/env python3

import json
from pathlib import Path

import ifcopenshell.ifcopenshell_wrapper as ifc


SCHEMA_NAMES = [
    "IFC2X3",
    "IFC4",
    "IFC4X3_ADD2",
]

REPO_ROOT = Path(__file__).resolve().parent.parent
OUTPUT_DIR = REPO_ROOT / "data" / "schema-docs"


def main():
    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)

    for schema_name in SCHEMA_NAMES:
        output_path = OUTPUT_DIR / output_file_name(schema_name)
        existing_urls = load_existing_urls(output_path)
        docs = build_docs(schema_name, existing_urls)
        output_path.write_text(json.dumps(docs, indent=2, sort_keys=True) + "\n")
        print(f"wrote {len(docs)} entities to {output_path}")


def build_docs(schema_name, existing_urls):
    schema = ifc.schema_by_name(schema_name)
    docs = {}
    attribute_cache = {}

    for entity in sorted(schema.entities(), key=lambda entity: entity.name()):
        entity_name = entity.name()
        docs[entity_name.upper()] = {
            "name": entity_name,
            "attributes": collect_attributes(entity, attribute_cache),
            "url": entity_url(schema_name, entity_name, existing_urls),
        }

    return docs


def collect_attributes(entity, attribute_cache):
    entity_name = entity.name()
    if entity_name in attribute_cache:
        return [dict(attribute) for attribute in attribute_cache[entity_name]]

    attributes = []
    parent = entity.supertype()
    if parent is not None:
        attributes.extend(collect_attributes(parent, attribute_cache))

    for attribute in entity.attributes():
        attributes = [item for item in attributes if item["name"] != attribute.name()]
        attributes.append(
            {
                "name": attribute.name(),
                "type_name": format_attribute_type(attribute),
                "declared_in": entity_name,
            }
        )

    attribute_cache[entity_name] = [dict(attribute) for attribute in attributes]
    return [dict(attribute) for attribute in attributes]


def format_attribute_type(attribute):
    type_name = format_type(attribute.type_of_attribute())
    if attribute.optional():
        return f"OPTIONAL {type_name}"
    return type_name


def format_type(type_decl):
    if isinstance(type_decl, str):
        return type_decl.upper()

    aggregate = getattr(type_decl, "as_aggregation_type", lambda: None)()
    if aggregate is not None:
        lower = format_bound(aggregate.bound1())
        upper = format_bound(aggregate.bound2())
        element_type = format_type(aggregate.type_of_element())
        kind = aggregate.type_of_aggregation_string().upper()
        return f"{kind} [{lower}:{upper}] OF {element_type}"

    if not hasattr(type_decl, "name") and hasattr(type_decl, "declared_type"):
        return format_type(type_decl.declared_type())

    if hasattr(type_decl, "name"):
        return type_decl.name()

    raise TypeError(f"unsupported type declaration: {type_decl!r}")


def format_bound(bound):
    if bound < 0:
        return "?"
    return str(bound)


def output_file_name(schema_name):
    if schema_name == "IFC2X3":
        return "ifc2x3_tc1_express_docs.json"
    if schema_name == "IFC4":
        return "ifc4_add2_tc1_express_docs.json"
    if schema_name == "IFC4X3_ADD2":
        return "ifc4x3_add2_express_docs.json"
    raise ValueError(f"unsupported schema name: {schema_name}")


def entity_url(schema_name, entity_name, existing_urls):
    cached_url = existing_urls.get(entity_name.upper())
    if cached_url is not None:
        return cached_url

    if schema_name == "IFC4":
        return (
            "https://standards.buildingsmart.org/IFC/RELEASE/IFC4/ADD2_TC1/HTML/link/"
            f"{entity_name.lower()}.htm"
        )

    if schema_name == "IFC4X3_ADD2":
        return (
            "https://standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/lexical/"
            f"{entity_name}.htm"
        )

    raise ValueError(f"could not determine URL for {schema_name}.{entity_name}")


def load_existing_urls(output_path):
    if not output_path.exists():
        return {}

    docs = json.loads(output_path.read_text())
    return {
        entity_name: entity_doc["url"]
        for entity_name, entity_doc in docs.items()
        if "url" in entity_doc
    }


if __name__ == "__main__":
    main()
