#!/usr/bin/env python3

import concurrent.futures
import html
import json
import re
import subprocess
import sys
from html.parser import HTMLParser
from pathlib import Path

INDEX_URL = (
    "https://standards.buildingsmart.org/IFC/RELEASE/IFC2x3/TC1/HTML/"
    "alphabeticalorder_entities.htm"
)
BASE_URL = "https://standards.buildingsmart.org/IFC/RELEASE/IFC2x3/TC1/HTML/"


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
        text = re.sub(r"[ \t]+", " ", text)
        text = re.sub(r" *\n *", "\n", text)
        text = re.sub(r"\n>\s*\n*", "\n> ", text)
        text = re.sub(r">\s*\n+", "> ", text)
        text = re.sub(r"\n{3,}", "\n\n", text)
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
    )
    try:
        return result.stdout.decode("utf-8")
    except UnicodeDecodeError:
        return result.stdout.decode("latin1")


def html_to_text(fragment):
    parser = HtmlToText()
    parser.feed(fragment)
    parser.close()
    return parser.text()


def parse_index(index_html):
    entries = re.findall(
        r'<A HREF="([^"]+)"[^>]*>(Ifc[A-Za-z0-9_]+)</A>',
        index_html,
        re.IGNORECASE,
    )
    return sorted({name: BASE_URL + href for href, name in entries}.items())


def extract_summary(page_html):
    building_smart = re.search(
        r"Definition from buildingSMART</u>:(.*?)(?:</p>|<blockquote)",
        page_html,
        re.DOTALL | re.IGNORECASE,
    )
    if building_smart:
        return clean_summary_text(building_smart.group(1))

    iso = re.search(
        r"Definition from ISO.*?</u>:(.*?)(?:</p>|<blockquote)",
        page_html,
        re.DOTALL | re.IGNORECASE,
    )
    if iso:
        return clean_summary_text(iso.group(1))

    fallback = re.search(
        r'<p CLASS="object-heading">.*?</p>(.*?)<a name="definition">',
        page_html,
        re.DOTALL | re.IGNORECASE,
    )
    if fallback:
        return clean_summary_text(fallback.group(1))

    raise ValueError("missing summary")


def clean_summary_text(fragment):
    cleaned = re.sub(
        r"<blockquote\b[^>]*>.*?</blockquote>",
        "",
        fragment,
        flags=re.DOTALL | re.IGNORECASE,
    )
    cleaned = re.sub(
        r"<ul\b[^>]*>.*?</ul>", "", cleaned, flags=re.DOTALL | re.IGNORECASE
    )
    cleaned = re.sub(r"<p>\s*<u><b>.*", "", cleaned, flags=re.DOTALL | re.IGNORECASE)
    text = html_to_text(cleaned)
    text = re.sub(r"\s*\n\s*", " ", text)
    text = re.sub(r"\s{2,}", " ", text)
    return text.strip()


def extract_direct_attribute_types(page_html):
    definition_match = re.search(
        r'<a name="definition">ENTITY</a></SPAN>\s+([A-Za-z0-9_]+)(.*?)END_ENTITY',
        page_html,
        re.DOTALL | re.IGNORECASE,
    )
    if not definition_match:
        return {}

    definition_block = definition_match.group(2)
    rows = re.findall(
        r'<td width="20%" nowrap>(.*?)</td>\s*'
        r'<td width="1%">.*?</td>\s*'
        r"<td>(.*?)</td>",
        definition_block,
        re.DOTALL | re.IGNORECASE,
    )

    attributes = {}
    for raw_name, raw_type in rows:
        name = html_to_text(raw_name)
        type_name = html_to_text(raw_type).rstrip(";")
        if name:
            attributes[name] = type_name

    return attributes


def extract_attribute_descriptions(page_html):
    section_match = re.search(
        r'<a name="attribute_description">Attribute definitions:</a></p>(.*?)'
        r'<p CLASS="inheritance-heading"><a name="inheritance">',
        page_html,
        re.DOTALL | re.IGNORECASE,
    )
    if not section_match:
        return {}

    section = section_match.group(1)
    descriptions = {}
    rows = re.findall(r"<tr[^>]*>(.*?)</tr>", section, re.DOTALL | re.IGNORECASE)
    for row in rows:
        if "attribute-description-name" not in row.lower():
            continue

        columns = re.findall(r"<td\b[^>]*>(.*?)</td>", row, re.DOTALL | re.IGNORECASE)
        if len(columns) < 3:
            continue

        raw_name = columns[0]
        raw_description = columns[2]
        name = html_to_text(raw_name)
        description = html_to_text(raw_description)
        if name:
            descriptions[name] = description

    return descriptions


def fetch_entity_doc(item):
    entity_name, url = item
    page_html = fetch_text(url)

    types = extract_direct_attribute_types(page_html)
    descriptions = extract_attribute_descriptions(page_html)
    attributes = []
    for name, type_name in types.items():
        attributes.append(
            {
                "name": name,
                "type_name": type_name,
                "description": descriptions.get(name, ""),
            }
        )

    return (
        entity_name.upper(),
        {
            "name": entity_name,
            "summary": extract_summary(page_html),
            "attributes": attributes,
            "url": url,
        },
    )


def main():
    repo_root = Path(__file__).resolve().parent.parent
    output_path = repo_root / "data" / "schema-docs" / "ifc2x3_tc1.json"
    output_path.parent.mkdir(parents=True, exist_ok=True)

    entries = parse_index(fetch_text(INDEX_URL))

    docs = {}
    with concurrent.futures.ThreadPoolExecutor(max_workers=12) as executor:
        future_map = {
            executor.submit(fetch_entity_doc, entry): entry[0] for entry in entries
        }
        for future in concurrent.futures.as_completed(future_map):
            entity_name = future_map[future]
            try:
                key, value = future.result()
            except Exception as exc:
                raise RuntimeError(f"failed to fetch {entity_name}") from exc
            docs[key] = value
            print(f"fetched {value['name']}", file=sys.stderr)

    ordered_docs = dict(sorted(docs.items()))
    output_path.write_text(
        json.dumps(ordered_docs, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    print(f"wrote {len(ordered_docs)} entities to {output_path}", file=sys.stderr)


if __name__ == "__main__":
    main()
