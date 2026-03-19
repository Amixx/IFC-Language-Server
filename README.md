# IFC-LSP

**IFC-LSP** is a lightweight Language Server Protocol (LSP) server for **IFC STEP** files (ISO 10303). It provides syntax-aware tooling powered by a Tree-sitter parser.

> ⚠️ Early development — most features are not yet implemented.

## Desired Features

- [ ] Hover
  - [ ] Preview for references (e.g. `#123`)
  - [ ] Info for IFC entity names (e.g. `IFCWALL`, `IFCSPACE`)
- [ ] Go-To Definition (jump to the line where an entity is defined)
- [ ] Find References (find and list all occurrences of a symbol)
- [ ] Syntax highlighting (based on Tree-sitter)
- [ ] Semantic validation

## Prerequisites

- Rust (stable) — https://www.rust-lang.org/tools/install
- A compatible editor with LSP client support (e.g. [Helix](https://docs.helix-editor.com/languages.html), [Neovim](https://neovim.io/doc/user/lsp.html), [VS Code](https://code.visualstudio.com/api/language-extensions/language-server-extension-guide))

## Build & Run

From the repository root:

```sh
cargo build --release
```

To run the server directly (useful for debugging):

```sh
cargo run --release
```

> Note: The language server communicates over stdin/stdout and is meant to be launched by an LSP client (editor).

## Using with an Editor

Coming soon!

## Documentation

- [Architecture](./docs/architecture.md) — overview of project structure and design decisions
- [Coding Guidelines](./docs/coding-guidelines.md) — formatting, style, and conventions
- [Requirements](./docs/requirements.md) — detailed feature requirements and scope

## Development

### Grammar & Parser

This project uses the [tree-sitter-ifc](https://github.com/NepomukWolf/tree-sitter-ifc) grammar. If you modify the grammar, regenerate the parser bindings with the standard `tree-sitter` build workflow.

### Running Tests

Coming soon!

## Contributing

Contributions are welcome! Check out the open issues on GitHub.

Please read the [Architecture](./docs/architecture.md) and [Coding Guidelines](./docs/coding-guidelines.md) before submitting a PR.

### Formatting

This project uses the following formatters:

- `rustfmt` for Rust
- `Prettier` for Markdown

## License

MIT
