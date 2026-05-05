# IFC-Language-Server Requirements

**For Human Developers:** Reference this file in your agent instructions (`AGENTS.md`, `.claude/`, etc.)

## Scope

### Included Support for IFC Schema Versions:

See [IFC Schema Specifications](https://technical.buildingsmart.org/standards/ifc/ifc-schema-specifications/):
- IFC 2.3.0.1  
  https://standards.buildingsmart.org/IFC/RELEASE/IFC2x3/TC1/EXPRESS/IFC2X3_TC1.exp
- IFC 4.0.2.1  
  https://standards.buildingsmart.org/IFC/RELEASE/IFC4/ADD2_TC1/EXPRESS/IFC4.exp
- IFC 4.3.2.0  
  https://standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/IFC4X3_ADD2.exp

For these officially supported versions:

- the repository should not store local EXPRESS files
- the official EXPRESS definitions should be fetched at compile time
- the fetched EXPRESS source should be embedded into the binary
- language-server startup and restart should not require network access to load official schemas

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

### Schema Diagnostics

The LS should provide schema-aware diagnostics for things like:

- Datatype checking (e.g., a reference points to IFCOWNERHITORY, even though it should point to an IFCWALL)
- Unsupported IfcVersions (e.g. display message to user)
- Entity Schema Compliance (e.g., IFCALIGNMENT is not part of IFC2x3)

Appropriate user-facing information (underlining, hover text) is part of the diagnostics.
