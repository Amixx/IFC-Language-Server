#!/usr/bin/env python3

import concurrent.futures
import html
import json
import re
import subprocess
import sys
from html.parser import HTMLParser
from pathlib import Path

EXPRESS_URL = (
    "https://standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/IFC4X3_ADD2.exp"
)
RESOURCE_URL = "https://ifc43-docs.standards.buildingsmart.org/api/v0/resource/{}"
ENTITY_URL = "https://ifc43-docs.standards.buildingsmart.org/IFC/RELEASE/IFC4x3/HTML/lexical/{}.htm"


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


def normalize_text(value):
    if isinstance(value, str):
        return value
    if isinstance(value, list):
        return " = ".join(normalize_text(item) for item in value)
    return str(value)


def fetch_entity_doc(name):
    try:
        payload = json.loads(fetch_text(RESOURCE_URL.format(name)))
    except subprocess.CalledProcessError as exc:
        raise RuntimeError(f"failed to fetch {name}: {exc.returncode}") from exc

    attributes = []
    for attribute in payload.get("attributes", []):
        _, attribute_name, type_name, description = attribute
        attributes.append(
            {
                "name": attribute_name,
                "type_name": normalize_text(type_name),
                "description": html_to_text(normalize_text(description)),
            }
        )

    return (
        name.upper(),
        {
            "name": payload["resource"],
            "summary": html_to_text(normalize_text(payload.get("definition", ""))),
            "attributes": attributes,
            "url": ENTITY_URL.format(payload["resource"]),
        },
    )


def parse_entity_names(express_text):
    return sorted(set(re.findall(r"(?m)^ENTITY\s+([A-Za-z0-9_]+)\b", express_text)))


def main():
    repo_root = Path(__file__).resolve().parent.parent
    output_path = repo_root / "data" / "schema-docs" / "ifc4x3_add2.json"
    output_path.parent.mkdir(parents=True, exist_ok=True)

    express_text = fetch_text(EXPRESS_URL)
    entity_names = parse_entity_names(express_text)

    docs = {}
    with concurrent.futures.ThreadPoolExecutor(max_workers=12) as executor:
        future_map = {
            executor.submit(fetch_entity_doc, name): name for name in entity_names
        }
        for future in concurrent.futures.as_completed(future_map):
            key, value = future.result()
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
