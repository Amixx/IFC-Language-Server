# IFC-Language-Server Coding Guidelines

**For Human Developers:** Reference this file in your agent instructions (`AGENTS.md`, `.claude/`, etc.).

## Goals

Code in this repository should be:

- correct
- small
- readable
- deterministic
- easy to test

Prefer the simplest implementation that matches the documented current scope in `requirements.md`.

## Rust Formatting

All Rust code must be formatted with `rustfmt`.

Before finishing a Rust change, run:

```sh
cargo fmt
```

## Architectural Discipline

- Keep changes aligned with the current `Backend` / `Document` / `SchemaDocs` split.
- Prefer feature logic that operates on `&Document` instead of pushing more logic into the LSP trait implementation.
- Preserve the current single-document model unless the work explicitly requires widening scope.
- Prefer full-document reparsing over incremental parsing complexity unless there is a demonstrated need.
- Keep per-document indexes small and derived from the syntax tree.

## Code Style

- Prefer explicit, readable code over compact but opaque code.
- Use descriptive names for types, functions, and variables.
- Keep functions narrowly scoped to one job.
- Reuse existing helpers before adding new ones.
- Add comments only when the intent is not obvious from the code itself.
- Avoid dead code, placeholder branches, and speculative abstractions.

## Dependencies

- Avoid adding crates unless they are clearly necessary.
- New crates require explicit approval before being added.
- Prefer the standard library or existing dependencies for small tasks.

## Error Handling

- Handle errors deliberately; do not ignore them silently.
- Prefer graceful fallbacks for unsupported schema versions, missing docs, and absent syntax nodes.
- Use `expect` only when failure is truly unrecoverable or in tests.
- Avoid panics in normal LSP request handling paths.

## Testing

- Prefer small unit tests close to the code they exercise.
- Test behavior, not implementation noise.
- When changing parsing, indexing, version detection, schema loading, or hover rendering, update tests in the same module.
- Add targeted tests for definition and references behavior when changing those modules, since current coverage there is thin.
- Use short IFC snippets in tests unless a repository sample file is clearly more useful.

## Schema Doc Assets

- Treat `data/schema-docs/*.json` as generated artifacts.
- When schema-doc structure or extraction logic changes, update the corresponding script in `scripts/` and regenerate the JSON instead of hand-editing generated output.
- Keep generator scripts small and deterministic.
- Avoid adding Python dependencies for the generator scripts without explicit approval.

## Scope Discipline

- Implement only what is required by the current issue or the documented current scope.
- Do not silently broaden the project into diagnostics, workspace indexing, or additional LSP features.
- If a design can be simpler, prefer the simpler version.
