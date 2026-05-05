# IFC-Language-Server Architecture

**For Human Developers:** Reference this file in your agent instructions (`AGENTS.md`, `.claude/`, etc.)

## Goal

This project currently stays small and focused.

The implemented architecture is built around one open IFC document, one tree-sitter parse, a small in-memory document index, and generated in-memory schema documentation that support the currently shipped LSP features:

- hover
- go-to-definition
- find-references
- schema-aware diagnostics

## Core Design

The codebase currently revolves around four main runtime concepts:

- `Backend`
  The `tower-lsp` entrypoint. It owns the open-document map, a shared `tree_sitter::Parser`, bundled schema docs, and the resolved schema-model store used for diagnostics.
- `Document`
  The parsed state of one IFC file. It stores the source text, optional syntax tree, detected schema version, definitions, references, and parsed entity-instance arguments.
- `SchemaDocCollection` & `SchemaDoc`
  A synchronous in-memory lookup for bundled IFC entity & type documentation by schema version and entity name. 

Feature modules in `src/features/` stay thin and operate on `&Document`.
Diagnostics operate on `&Document` plus `&SchemaDoc`/`SchemaDocCollection`.

## Data Flow

The current flow is:

1. An editor opens a document or sends a full-text change.
2. `Backend` reparses the full text with the shared tree-sitter parser.
3. `Document::parse` detects the schema version and rebuilds the per-document indexes.
4. If the schema version is supported, the matching `SchemaDoc` is used to collect diagnostics & entity hover info.
5. `Backend` publishes diagnostics to the LSP client.
6. The rebuilt `Document` replaces the previous entry in the backend's `HashMap<Url, Document>`.
7. Hover, definition, references, and diagnostics all read from that stored `Document`.

There is no incremental parsing, background indexing, or cross-document state.

## Backend

`src/backend.rs` currently owns:

- `client: Client`
- `documents: Arc<RwLock<HashMap<Url, Document>>>`
- `parser: Arc<RwLock<Parser>>`
- `schema_docs: SchemaDocCollection`

The server currently advertises these capabilities:

- `textDocument/hover`
- full text document sync
- `textDocument/definition`
- `textDocument/references`

Diagnostics are published via `textDocument/publishDiagnostics` on open and change.

## Document Model

The `Document` type in `src/document.rs` is the central internal representation:

```rust
pub struct Document {
    pub text: String,
    pub tree: Option<tree_sitter::Tree>,
    pub version: Option<IfcVersion>,
    pub definitions: HashMap<u32, DefinitionInfo>,
    pub references: HashMap<u32, Vec<Range>>,
    pub instances: Vec<EntityInstanceInfo>,
}

pub struct DefinitionInfo {
    pub id_range: Range,
    pub entity_range: Range,
    pub entity_name: String,
}
```

Important details:

- `text`
  Used for hover rendering and node text extraction.
- `tree`
  Stored as `Option<Tree>` because parsing may fail.
- `version`
  Detected with a simple `FILE_SCHEMA` text scan. Unknown schemas remain `None`.
- `definitions`
  Maps numeric ids such as `123` to the `instance_id` range, full entity-instance range, and defining entity name.
- `references`
  Maps numeric ids to all `reference` ranges in the same document.
- `instances`
  Stores parsed `entity_instance` values, including entity name, argument ranges, and structured parameter values used by diagnostics.

The document index is built by traversing the syntax tree and recording:

- `entity_instance` nodes for definitions
- `reference` nodes for references
- parameter values such as strings, numbers, references, enumerations, `$`, `*`, lists, and inline typed values

Ids are stored as `u32`, not as raw `#123` strings.

## Schema Documentation

The `SchemaDoc`and `SchemaDocCollection`structs are defined as follows:

```rust
pub struct EntityDoc {
    pub name: String,
    pub attributes: Vec<EntityAttributeDoc>,
    pub url: String,
}

pub enum TypeDoc {
    Alias(AliasTypeDef),
    Enumeration(EnumerationTypeDef),
    Select(SelectTypeDef),
}

pub struct SchemaDoc {
    pub entities: HashMap<String, EntityDoc>,
    pub types: HashMap<String, TypeDoc>,

pub struct SchemaDocCollection {
    pub docs: HashMap<IfcVersion, SchemaDoc>,
}
```

A `SchemaDoc` contains information about:
- Entities:
  - attributes
  - attribute types
  - where attributes have been declared
  - inheritance
- Types:
  - Alias Types (e.g., a wrapper around a primitive)
  - Enumeration Types (e.g. one of a set of values)
  - Select Types (can be one of many types)

The `SchemaDoc`is the single-source of truth for all information about entities and types of a IfcVersion, with all relevant hover and diagnostics information sourced from it. A `SchemaDocCollection` is simply collection of those docs by IfcVersion. 

The Docs for the officially supported IfcVersions are generated at runtime during startup of the LS, and are created by pulling the the official EXPRESS definitions and using the `eprs` crate. Support for the loading of custom IFC EXPRESS definitions is also possible in the future. 


## tree-sitter Integration

The server depends on the published `tree-sitter-ifc` crate from crates.io.

At runtime:

- `Backend::new` creates one `tree_sitter::Parser`
- the parser is configured with `tree_sitter_ifc::LANGUAGE`
- document text is parsed into a `tree_sitter::Tree`
- feature handlers use `Document::node_at_position` to inspect the node under the cursor

The tree-sitter grammar remains the source of truth for IFC syntax recognition. This crate is responsible for indexing and LSP behavior on top of that parse tree.

## Feature Behavior

### Hover

`src/features/hover.rs` currently supports:

- entity-name hover for `entity_name` nodes when `Document::version` is known and bundled `SchemaDoc` exist
- reference hover for `reference` nodes by rendering the full defining entity instance as an IFC code block
- a generic instructional hover for other node kinds

Entity hover currently renders:

- the entity name
- an inherited attribute table
- a direct-attribute table
- a link to the official documentation page

Although the `SchemaDoc` include more data, the runtime `EntityDoc` currently consumes only:

- `name`
- `attributes`
- `url`

### Go To Definition

`src/features/definition.rs` only resolves `reference` nodes.

It returns the `id_range` of the matching local definition. It does not jump from an `instance_id` token to itself, and it does not perform cross-file lookup.

### Find References

`src/features/references.rs` accepts either:

- an `instance_id`
- a `reference`

It returns:

- the definition location if present
- every indexed reference location for the same id in the same document

### Diagnostics

`src/diagnostics/datatype.rs` currently validates IFC entity instance arguments against a generated schema documentation. 

The current diagnostics provider supports:

- wrong local reference target types
- unresolved local references
- primitive datatype mismatches
- enumeration mismatches
- argument-count mismatches
- invalid `$` usage for required attributes
- invalid `*` usage except where an inherited attribute is derived in a subtype
- aggregate cardinality/type mismatches
- `SELECT` branch validation
- inline typed values such as `IFCLABEL('Name')`

The current provider does not yet evaluate general EXPRESS `WHERE` rules.


## Project Structure

The current codebase is intentionally small:

```text
src/
  main.rs
  backend.rs
  diagnostics/
    mod.rs
    datatype.rs
  document.rs
  schema/
    *.rs
  features/
    mod.rs
    hover.rs
    definition.rs
    references.rs
samples/
  *.ifc
```

## Testing Approach

Tests currently live next to the implementation in the same Rust modules.

Existing tests mainly cover:

- document parsing and index building
- schema-doc loading
- schema-model resolution
- datatype diagnostics behavior
- hover rendering behavior

There is not yet dedicated test coverage for the definition and references modules, even though those features are implemented.

## Non-Goals For The Current Architecture

The current project should continue to avoid:

- cross-file indexing
- incremental parsing infrastructure
- background worker systems
- large abstraction layers or service registries
- premature support for unimplemented LSP features

## Guiding Principle

Keep the implementation direct.

If a new abstraction does not clearly support the currently shipped feature set, it probably does not belong here yet.
