#!/usr/bin/env python3

import json
import re
import subprocess
import sys
from pathlib import Path

EXPRESS_URL = (
    "https://standards.buildingsmart.org/IFC/RELEASE/IFC2x3/TC1/EXPRESS/IFC2X3_TC1.exp"
)
INDEX_URL = (
    "https://standards.buildingsmart.org/IFC/RELEASE/IFC2x3/TC1/HTML/"
    "alphabeticalorder_entities.htm"
)
BASE_URL = "https://standards.buildingsmart.org/IFC/RELEASE/IFC2x3/TC1/HTML/"
OUTPUT_FILE = "ifc2x3_tc1_express_docs.json"
SECTION_KEYWORDS = {"DERIVE", "INVERSE", "UNIQUE", "WHERE"}


def fetch_text(url):
    result = subprocess.run(
        [
            "curl",
            "-L",
            "--silent",
            "--show-error",
            "--fail",
            "-A",
            "ifc-language-server-doc-generator/0.1 (+https://github.com/NepomukWolf/IFC-Language-Server)",
            url,
        ],
        check=True,
        capture_output=True,
        text=True,
    )
    return result.stdout


def normalize_whitespace(value):
    return re.sub(r"\s+", " ", value).strip()


def split_statements(body):
    section = "explicit"
    pending = []

    for raw_line in body.splitlines():
        line = raw_line.strip()
        if not line:
            continue

        upper_line = line.upper()
        if upper_line in SECTION_KEYWORDS:
            if pending:
                raise ValueError(f"dangling statement before {upper_line}")
            section = upper_line.lower()
            continue

        pending.append(line)
        if line.endswith(";"):
            yield section, normalize_whitespace(" ".join(pending))
            pending = []

    if pending:
        raise ValueError("unterminated statement in entity body")


def parse_parent_name(statements):
    for section, statement in statements:
        if section != "explicit":
            continue

        match = re.search(r"SUBTYPE OF\s*\(([^)]+)\)", statement, re.IGNORECASE)
        if not match:
            continue

        names = re.findall(r"[A-Za-z][A-Za-z0-9_]*", match.group(1))
        if not names:
            continue
        return names[0]

    return None


def build_summary(statements):
    clauses = []
    for section, statement in statements:
        if section != "explicit":
            continue
        if re.match(r"[A-Za-z][A-Za-z0-9_]*\s*:", statement):
            continue
        clauses.append(statement.rstrip(";"))

    return normalize_whitespace(" ".join(clauses))


def parse_attribute(section, statement, entity_name):
    if section not in {"explicit", "inverse", "derive"}:
        return None

    if section == "derive":
        match = re.match(
            r"([A-Za-z][A-Za-z0-9_]*)\s*:\s*(.*?)\s*:=\s*.*;$",
            statement,
            re.IGNORECASE,
        )
    else:
        match = re.match(
            r"([A-Za-z][A-Za-z0-9_]*)\s*:\s*(.*);$",
            statement,
            re.IGNORECASE,
        )

    if not match:
        return None

    return {
        "name": match.group(1),
        "type_name": normalize_whitespace(match.group(2)),
        "declared_in": entity_name,
    }


def parse_entity_urls(index_html):
    entries = re.findall(
        r'<A HREF="([^"]+)"[^>]*>(Ifc[A-Za-z0-9_]+)</A>',
        index_html,
        re.IGNORECASE,
    )
    return {name: BASE_URL + href for href, name in entries}


def parse_entities(express_text, entity_urls):
    entities = {}
    pattern = re.compile(
        r"(?ms)^ENTITY\s+([A-Za-z0-9_]+)\b(.*?)^END_ENTITY;",
    )

    for match in pattern.finditer(express_text):
        entity_name = match.group(1)
        body = match.group(2)
        statements = list(split_statements(body))
        direct_attributes = []
        for section, statement in statements:
            attribute = parse_attribute(section, statement, entity_name)
            if attribute is not None:
                direct_attributes.append(attribute)

        url = entity_urls.get(entity_name)
        if url is None:
            raise ValueError(f"missing IFC2x3 HTML url for {entity_name}")

        entities[entity_name] = {
            "name": entity_name,
            "summary": build_summary(statements),
            "parent": parse_parent_name(statements),
            "direct_attributes": direct_attributes,
            "url": url,
        }

    return entities


def collect_attributes(entity_name, entities, cache, visiting):
    if entity_name in cache:
        return cache[entity_name]

    if entity_name in visiting:
        raise ValueError(f"cyclic inheritance detected for {entity_name}")

    visiting.add(entity_name)
    entity = entities[entity_name]
    inherited = []
    if entity["parent"] is not None:
        inherited = collect_attributes(entity["parent"], entities, cache, visiting)

    merged = [dict(attribute) for attribute in inherited]
    for attribute in entity["direct_attributes"]:
        merged = [item for item in merged if item["name"] != attribute["name"]]
        merged.append(dict(attribute))

    cache[entity_name] = merged
    visiting.remove(entity_name)
    return merged


def build_docs(entities):
    attribute_cache = {}
    docs = {}

    for entity_name in sorted(entities):
        entity = entities[entity_name]
        docs[entity_name.upper()] = {
            "name": entity["name"],
            "summary": entity["summary"],
            "attributes": collect_attributes(
                entity_name, entities, attribute_cache, set()
            ),
            "url": entity["url"],
        }

    return docs


def main():
    repo_root = Path(__file__).resolve().parent.parent
    output_path = repo_root / "data" / "schema-docs" / OUTPUT_FILE
    output_path.parent.mkdir(parents=True, exist_ok=True)

    express_text = fetch_text(EXPRESS_URL)
    entity_urls = parse_entity_urls(fetch_text(INDEX_URL))
    entities = parse_entities(express_text, entity_urls)
    docs = build_docs(entities)

    output_path.write_text(
        json.dumps(docs, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    print(f"wrote {len(docs)} entities to {output_path}", file=sys.stderr)


if __name__ == "__main__":
    main()
