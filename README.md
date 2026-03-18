# IFC-LSP

**IFC-LSP** is a lightweight Language Server Protocol (LSP) server for **IFC STEP** files (ISO 10303). It provides syntax-aware tooling powered by a Tree-sitter parser. It is currently at a very early development stage and most of the desired features are not yet implemented.

## Desired Features

- [ ] Go-To Definition (jump to the line in file where an entity is defined)
- [ ] Find References (find and present all occurences of a symbol)
- [ ] Hover preview for references (e.g. #123)
- [ ] Hover info for IFC entity names (e.g. `IFCWALL`, `IFCSPACE`, etc.)
- [ ] Syntax highlighting (based on tree-sitter)
- [ ] Semantic validation

## Prerequisites

- Rust (stable) — https://www.rust-lang.org/tools/install
- If you want to test the LSP: A compatible editor with LSP client support (e.g. Helix, Neovim, VS Code)

## Build & Run

From the repository root:

```sh
cargo build --release
```

To run the server directly (useful for debugging):

```sh
cargo run --release
```

> Note: The language server communicates over stdin/stdout, so it is meant to be launched by an LSP client (editor).

## Using with an Editor

### VS Code (example)

You can use a generic LSP client extension such as [vscode-languageclient](https://code.visualstudio.com/api/language-extensions/language-server-extension-guide) or [`LSP`](https://marketplace.visualstudio.com/items?itemName=kabouzeid.nougat).

A minimal `package.json` command configuration for a VS Code extension might look like:

```json
"server": {
  "command": "cargo",
  "args": ["run", "--release"],
  "transport": "stdio"
}
```

### NeoVim (nvim-lspconfig) Example

```lua
require('lspconfig').ifc_lsp.setup({
  cmd = { "cargo", "run", "--release" },
  filetypes = { "ifc" },
  root_dir = require('lspconfig.util').root_pattern('.git'),
})
```

> Adjust the `cmd` path if you build the binary somewhere other than the workspace root.

## Development

### Grammar & Parser

This project uses the [tree-sitter-ifc](./tree-sitter-ifc) grammar. If you modify the grammar, you can regenerate the parser bindings with the standard `tree-sitter` build workflow.

### Running tests

Comming Soon!

## Contributing

Contributions are welcome!
Check out the open issues on GitHub! Please familiarize yourself with the project architecture (./docs/Architecture.md) and follow the formatting guidelines.

### Formatting

This project uses the following formatters:

- Prettier for markdown
- rustfmt for rust

## License

MIT
