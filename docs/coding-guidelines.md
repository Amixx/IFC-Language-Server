# IFC-LSP Coding Guidelines

**For Human Developers:** Reference this file in your agent instructions (`AGENTS.md`, `.claude/`, etc.).

## Goals

Generated code should be:

- correct
- small
- readable
- easy to test

Prefer the simplest implementation that satisfies the documented requirement.

## Rust Formatting

All Rust code must be formatted with `rustfmt`.

Before finishing a change, run:

```sh
cargo fmt
```

## General Expectations

- Keep line count low when possible.
- Avoid unnecessary abstractions, indirection, or generic frameworks.
- Prefer straightforward functions and data structures over clever code.
- Keep modules focused and small.
- Reuse existing code before adding new helpers or types.
- Avoid adding crates/dependencies unless they are clearly necessary.
- New crates require explicit approval before being added.

## Code Style

- Prefer explicit, readable code over compact but opaque code.
- Use descriptive names for types, functions, and variables.
- Keep functions narrowly scoped to one job.
- Add comments only when the intent is not obvious from the code itself.
- Avoid dead code, placeholder branches, and unused helpers.

## Error Handling

- Handle errors deliberately; do not ignore them silently.
- Return structured errors where useful.
- Use `expect` only when failure is truly unrecoverable or in tests.

## Testing

- Prefer small unit tests close to the code they exercise.
- Test behavior, not implementation noise.
- Avoid brittle tests tied to incidental formatting or logging.
- For this project, prefer function-level tests over protocol-level tests unless protocol behavior is the specific goal.

## Scope Discipline

- Implement only what is required by the current issue or documented requirement.
- Do not pre-build architecture for hypothetical future features.
- If a design can be simpler, prefer the simpler version.
