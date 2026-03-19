#!/usr/bin/env python3

import concurrent.futures
import html
import json
import re
import subprocess
import sys
from html.parser import HTMLParser
from pathlib import Path

XSD_URL = "https://standards.buildingsmart.org/IFC/RELEASE/IFC4/ADD2_TC1/XML/IFC4.xsd"
LINK_URL = (
    "https://standards.buildingsmart.org/IFC/RELEASE/IFC4/ADD2_TC1/HTML/link/{}.htm"
)
BASE_URL = "https://standards.buildingsmart.org/IFC/RELEASE/IFC4/ADD2_TC1/HTML/"


class HtmlToText(HTMLParser):
    def __init__(self):
        super().__init__()
        self.parts = []
        self.blockquote_depth = 0

    def handle_starttag(self, tag, attrs):
        if tag in {"p", "ul", "ol"}:
            self.parts.append("\n\n")
        elif tag == "li":
            self.parts.append("\n- ")
        elif tag == "br":
            self.parts.append("\n")
        elif tag == "blockquote":
            self.parts.append("\n\n> ")
            self.blockquote_depth += 1

    def handle_endtag(self, tag):
        if tag == "blockquote" and self.blockquote_depth:
            self.blockquote_depth -= 1
            self.parts.append("\n")

    def handle_data(self, data):
        self.parts.append(data)

    def text(self):
        text = html.unescape("".join(self.parts))
        text = text.replace("\xa0", " ")
        text = text.replace("“", '"').replace("”", '"')
        text = text.replace("’", "'").replace("–", "-").replace("—", "-")
        text = re.sub(r"\n{3,}", "\n\n", text)
        text = re.sub(r"[ \t]+", " ", text)
        text = re.sub(r" *\n *", "\n", text)
        return text.strip()


def fetch_text(url):
    result = subprocess.run(
        [
            "curl",
            "-L",
            "--silent",
            "--show-error",
            "--fail",
            "-A",
            "ifc-lsp-doc-generator/0.1 (+https://github.com/NepomukWolf/ifc-lsp)",
            url,
        ],
        check=True,
        capture_output=True,
        text=True,
    )
    return result.stdout


def html_to_text(fragment):
    parser = HtmlToText()
    parser.feed(fragment)
    parser.close()
    return parser.text()


def parse_entity_names(xsd_text):
    return sorted(
        set(re.findall(r'<xs:complexType name="(Ifc[A-Za-z0-9_]+)"', xsd_text))
    )


def resolve_lexical_url(entity_name):
    link_page = fetch_text(LINK_URL.format(entity_name.lower()))
    match = re.search(
        r'<frame src="\.\./([^"]+/lexical/[^"]+\.htm)" name="info"', link_page
    )
    if not match:
        raise RuntimeError(f"failed to resolve lexical page for {entity_name}")
    return BASE_URL + match.group(1)


def extract_section(html_text, summary_title):
    pattern = rf"<summary>{re.escape(summary_title)}</summary>(.*?)</details>"
    match = re.search(pattern, html_text, re.DOTALL | re.IGNORECASE)
    return match.group(1) if match else None


def strip_tags(fragment):
    return re.sub(r"<[^>]+>", "", fragment).strip()


def extract_attribute_rows(section_html):
    table_match = re.search(
        r'<table class="attributes">(.*?)</table>',
        section_html,
        re.DOTALL | re.IGNORECASE,
    )
    if not table_match:
        return []

    rows = re.findall(
        r"<tr>(.*?)</tr>", table_match.group(1), re.DOTALL | re.IGNORECASE
    )
    attributes = []
    for row in rows:
        if "<th>" in row.lower() or "colspan=" in row.lower():
            continue

        columns = re.findall(r"<td\b[^>]*>(.*?)</td>", row, re.DOTALL | re.IGNORECASE)
        if len(columns) < 5:
            continue

        number = strip_tags(columns[0])
        if not number.isdigit():
            continue

        attributes.append(
            {
                "name": html_to_text(columns[1]),
                "type_name": html_to_text(columns[2]),
                "description": html_to_text(columns[4]),
            }
        )

    return attributes


def fetch_entity_doc(entity_name):
    lexical_url = resolve_lexical_url(entity_name)
    lexical_page = fetch_text(lexical_url)

    definition_section = extract_section(lexical_page, "Entity definition")
    if definition_section is None:
        raise ValueError(f"missing entity definition section for {entity_name}")

    attributes_section = extract_section(lexical_page, "Attribute definitions") or ""

    return (
        entity_name.upper(),
        {
            "name": entity_name,
            "summary": html_to_text(definition_section),
            "attributes": extract_attribute_rows(attributes_section),
            "url": lexical_url,
        },
    )


def main():
    repo_root = Path(__file__).resolve().parent.parent
    output_path = repo_root / "data" / "schema-docs" / "ifc4_add2_tc1.json"
    output_path.parent.mkdir(parents=True, exist_ok=True)

    xsd_text = fetch_text(XSD_URL)
    entity_names = parse_entity_names(xsd_text)

    docs = {}
    skipped = []
    with concurrent.futures.ThreadPoolExecutor(max_workers=12) as executor:
        future_map = {
            executor.submit(fetch_entity_doc, entity_name): entity_name
            for entity_name in entity_names
        }
        for future in concurrent.futures.as_completed(future_map):
            entity_name = future_map[future]
            try:
                key, value = future.result()
            except ValueError as exc:
                skipped.append(entity_name)
                print(f"skipped {entity_name}: {exc}", file=sys.stderr)
                continue
            docs[key] = value
            print(f"fetched {value['name']}", file=sys.stderr)

    ordered_docs = dict(sorted(docs.items()))
    output_path.write_text(
        json.dumps(ordered_docs, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    print(f"wrote {len(ordered_docs)} entities to {output_path}", file=sys.stderr)
    print(f"skipped {len(skipped)} non-entity pages", file=sys.stderr)


if __name__ == "__main__":
    main()
