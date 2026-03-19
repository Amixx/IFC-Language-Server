# IFC-LSP Architecture

**For Human Developers:** Reference this file in your agent instructions (`AGENTS.md`, `.claude/`, etc.)

## Goal

This project should stay small and focused.

The architecture should support the features in [requirements.md](/Users/benedict/repos/ifc-lsp/docs/requirements.md) without introducing unnecessary layers, abstractions, or infrastructure.

The main design goal is:

- parse one IFC document
- build a small in-memory index for that document
- answer LSP feature requests from that index

## Core Design

The system only needs three main concepts:

- `Backend`
  The LSP entrypoint. It receives requests from the editor and forwards them to internal feature logic.
- `Document`
  The parsed state of one open IFC file. It stores the source text, parse tree, schema version, and a small symbol index.
- `SchemaDocs`
  A lookup layer for IFC entity documentation by schema version and entity name.

Everything else should remain a thin helper around these concepts.

## Data Flow

The expected flow is:

1. A document is opened or changed.
2. The backend reparses the full text.
3. A `Document` object is rebuilt from the current text.
4. The `Document` builds a small index for definitions, references, and entity names.
5. Hover, definition, and references requests read from that index.

This keeps all language features consistent and avoids duplicating lookup logic in multiple handlers.

## Document Model

The `Document` type should be the core internal representation of an open IFC file.

It only needs enough information to support the current requirements.

Example shape:

```rust
struct Document {
    text: String,
    tree: tree_sitter::Tree,
    version: IfcVersion,
    definitions: HashMap<String, Range>,
    references: HashMap<String, Vec<Range>>,
}
```

This model is intentionally small.

- `text`
  Needed for hover previews and extracting snippets.
- `tree`
  Needed for tree-sitter based syntax lookup.
- `version`
  Needed for version-specific entity documentation lookup.
- `definitions`
  Maps entity ids such as `#123` to the range where they are defined.
- `references`
  Maps entity ids such as `#123` to all reference locations in the same file.

If needed, entity-name lookup can either be derived from the syntax tree or stored in an additional small structure later. That should only be added when it clearly improves the implementation.

## tree-sitter Integration

This repository includes the IFC grammar in the local [`tree-sitter-ifc`](/Users/benedict/repos/ifc-lsp/tree-sitter-ifc) directory.

The main `ifc-lsp` crate depends on it as a local path dependency through `Cargo.toml`. At runtime, the language server creates a `tree_sitter::Parser`, loads the language from `tree_sitter_ifc`, and parses IFC source text into a `tree_sitter::Tree`.

That syntax tree is then used to:

- identify the node under the cursor
- distinguish entity names from entity references
- support building the small per-document index used by hover, go-to-definition, and find-references

The `tree-sitter-ifc` subdirectory should be treated as the source of truth for IFC syntax parsing, while the main crate should focus on indexing and LSP feature behavior built on top of that parse tree.

## Feature Design

### Hover

Hover should support two cases:

- hovering over an entity definition such as `IFCWALL`
- hovering over an entity id such as `#1234`

Behavior:

- If the cursor is on an entity name, resolve the IFC schema version from the `Document` and fetch documentation from `SchemaDocs`.
- If the cursor is on an entity id, resolve the target definition from `definitions` and render a short preview of the defining line.

### Go To Definition

Go to definition should:

- identify whether the cursor is on an entity reference such as `#123`
- resolve that id using `definitions`
- return the corresponding location

This feature should not implement its own parsing logic beyond finding the relevant token at the cursor.

### Find References

Find references should:

- identify the entity id at the cursor
- resolve all matching locations from `references`
- return those locations

Per the current requirements, this should remain single-document only.

## Project Structure

The codebase should stay close to this structure:

```text
src/
  main.rs
  backend.rs
  document.rs
  schema.rs
  features/
    hover.rs
    definition.rs
    references.rs
```

### File Responsibilities

- `src/main.rs`
  Starts the LSP server and wires the backend.
- `src/backend.rs`
  Contains the `tower-lsp` `LanguageServer` implementation and stores open documents.
- `src/document.rs`
  Parses IFC text and builds the in-memory document index.
- `src/schema.rs`
  Loads and serves entity documentation for supported IFC versions.
- `src/features/hover.rs`
  Contains hover-specific logic.
- `src/features/definition.rs`
  Contains go-to-definition logic.
- `src/features/references.rs`
  Contains find-references logic.

This structure is enough for the current feature set and leaves room for moderate growth without becoming fragmented.

## Schema Documentation

Entity documentation should not be fetched live during hover requests.

Instead, the project should use preprocessed local documentation assets for:

- IFC 2.3.0.1
- IFC 4.0.2.1
- IFC 4.3.2.0

`SchemaDocs` should expose a simple interface like:

```rust
fn get_entity_doc(version: IfcVersion, entity_name: &str) -> Option<EntityDoc>
```

This keeps hover fast, predictable, and independent of network availability.

## Testing Approach

Testing should stay simple and focused on function-level behavior.

- Put unit tests near the relevant implementation files.
- Test parsing helpers, indexing logic, hover rendering, definition resolution, and reference lookup.
- Avoid testing raw JSON-RPC payloads unless there is a clear need later.

If sample-file testing becomes useful, add a small integration test later. It should exercise internal feature functions, not the full protocol transport.

## Non-Goals

The current architecture should avoid introducing:

- cross-file indexing
- background worker systems
- plugin systems
- service registries
- complex domain layering
- runtime downloading of schema documentation

These would add complexity without being required by the current scope.

## Guiding Principle

For now, the project should prefer a direct implementation over a reusable framework.

If a new abstraction does not clearly support one of the documented requirements, it should not be introduced.
