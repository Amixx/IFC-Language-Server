# IFC-LSP Requirements

**For Human Developers:** Reference this file in your agent instructions (`AGENTS.md`, `.claude/`, etc.)

## Scope

### Supported IFC Schema Versions

See [IFC Schema Specifications](https://technical.buildingsmart.org/standards/ifc/ifc-schema-specifications/):

- IFC 2.3.0.1  
  https://standards.buildingsmart.org/IFC/RELEASE/IFC2x3/TC1/XML/IFC2X3.xsd
- IFC 4.0.2.1  
  https://standards.buildingsmart.org/IFC/RELEASE/IFC4/ADD2_TC1/XML/IFC4.xsd
- IFC 4.3.2.0  
  https://standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/IFC4X3_ADD2.exp

## Features

### IFC STEP P21 Syntax Highlighting

The language server should support syntax highlighting for IFC STEP P21 files.

### Hover

#### Entity Definition Hover

Hovering over an entity definition such as `IFCWALL` should display documentation for that entity.

Preferably, the hover should include:

- a short description of the entity
- a list or table of attributes, including:
  name, type, and description
- a link to the official web documentation for the entity

#### Entity ID Hover

Hovering over an entity id such as `#1234` should display a preview of the line in the file where that entity is defined.

### Go to Definition

When a user invokes Go to Definition, for example through an editor context menu or keyboard shortcut, the language server should:

- identify the IFC entity reference at the cursor position, such as `#123`
- resolve that reference to its corresponding entity definition within the same document
- return the precise target location, including URI and range

### Find References

The language server should implement find references functionality for IFC symbols.

For the current scope, this should be limited to a single document. Cross-file indexing is not required.

### Semantic Checking

When an IFC STEP P21 file violates IFC or STEP structural or semantic rules, the language server should report diagnostics for the affected source range.

Examples include:

- missing required STEP envelope sections such as `ISO-10303-21`, `HEADER`, `DATA`, or `END-ISO-10303-21`
- invalid or inconsistent entity references
- values that do not match the expected type or role for a referenced attribute

Diagnostics should highlight the most relevant range in the document and distinguish errors from less severe issues where appropriate.
