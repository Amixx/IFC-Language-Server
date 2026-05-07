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

## Architectural Discipline

- Keep changes aligned with the current `Backend` / `Document` / `SchemaDocCollection` split.
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
- When creating a new module, add descriptive comments explaining its purpose.

## Approval Boundaries

- New crates require explicit approval before being added.
  - Avoid adding crates unless they are clearly necessary.
  - Prefer the standard library or existing dependencies for small tasks.
- Major architectural changes require approval.
- Large refactors require approval.
- No modifications to `./docs` without specific instructions by the user.
- No deleting, resetting, or reverting unrelated work without approval.
- Ask the user when encountering ambiguous requirements, conflicting docs, or risky edits.

## Error Handling

- Handle errors deliberately; do not ignore them silently.
- Prefer graceful fallbacks for unsupported schema versions, missing docs, and absent syntax nodes.
- Use `expect` only when failure is truly unrecoverable or in tests.
- Avoid panics in normal LSP request handling paths.

## Testing

- Prefer small unit tests close to the code they exercise.
- Test behavior, not implementation noise.
- When changing parsing, indexing, version detection, schema loading, or hover rendering, update tests in the same module.
- Use short IFC snippets in tests unless a repository sample file is clearly more useful.
- Note: Builds require internet access due to the EXPRESS schema pulling at compile time in `build.rs`.

## Scope Discipline

- Implement only what is required by the current issue or the documented current scope.
- Do not silently broaden the project into diagnostics, workspace indexing, or additional LSP features.
- If a design can be simpler, prefer the simpler version.

## Verification Gate

The following commands must pass for a change to be verified:

```bash
cargo fmt --check
cargo test --locked
cargo build --release --locked
```

If the tests fail repeatedly when implementing a new feature, the run should be halted and the issues presented to the user.
