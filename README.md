# IFC-Language-Server

`ifc-language-server` is a lightweight Language Server Protocol server for [IFC STEP P21](https://technical.buildingsmart.org/standards/ifc/ifc-schema-specifications/) files (ISO 10303). It provides schema-aware hover information and local symbol navigation for IFC entity instances.


## Current Capabilities

- Hover on IFC entity names such as `IFCWALL` and `IFCSPACE`
- Hover preview for entity references such as `#123`
- Go-to-definition for local entity references
- Find-references within the current document

## Supported IFC Schema Versions

Bundled entity documentation is currently included for:

- IFC 2.3.0.1
- IFC 4.0.2.1
- IFC 4.3.2.0

## Current Limitations

- Single-document navigation only
- Full-document reparsing on open and change
- No semantic diagnostics yet
- No completion, rename, code actions, or formatting support
- Syntax highlighting depends on editor grammar support such as `tree-sitter-ifc`; it is not provided by the LSP server itself

## Installation

### Editor integrations

Extensions for VSCode and Zed are available for local installation (though still under development):

- [VSCode](github.com/NepomukWolf/vscode-ifc)

- [Zed](github.com/Finradon/zed-ifc)

### Manual

Release binaries are available on the [Releases](https://github.com/NepomukWolf/IFC-Language-Server/releases) page.

After downloading a release:

1. Place the `ifc-language-server` binary somewhere on your `PATH`, or configure your editor to point to the absolute binary path.
2. Register it as the language server for IFC STEP files in your editor or IDE.

## Development

### Prerequisites

- Rust (stable): https://www.rust-lang.org/tools/install
- An editor with LSP client support such as [Helix](https://docs.helix-editor.com/languages.html), [Neovim](https://neovim.io/doc/user/lsp.html), [VS Code](https://code.visualstudio.com/api/language-extensions/language-server-extension-guide), or Zed

### Build

Build a release binary with:

```bash
cargo build --release
```

Run the server directly for debugging:

```bash
cargo run --release
```

The server communicates over `stdin`/`stdout` and is intended to be launched by an LSP client.

### Tests

Run the test suite with:

```bash
cargo test
```

### Grammar And Parser

This project depends on the published [`tree-sitter-ifc`](https://crates.io/crates/tree-sitter-ifc) crate for IFC parsing.

If you are developing the grammar itself, work in the [`tree-sitter-ifc`](https://github.com/NepomukWolf/tree-sitter-ifc) repository and publish a new crate version when needed. During local development, you can temporarily override the crates.io dependency with a `[patch.crates-io]` entry that points at a local checkout.

### Formatting

This project uses:

- `rustfmt` for Rust via `cargo fmt`
- Prettier for Markdown

## Documentation

- [Architecture](./docs/architecture.md)
- [Coding Guidelines](./docs/coding-guidelines.md)
- [Requirements](./docs/requirements.md)

## Contributing

Contributions are welcome. Read [Architecture](./docs/architecture.md) and [Coding Guidelines](./docs/coding-guidelines.md) before submitting a PR.

## License

MIT
