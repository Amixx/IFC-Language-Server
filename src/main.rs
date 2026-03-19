//! Binary entrypoint for the IFC language server.
//! Wires the backend into `tower-lsp` and starts serving requests.

mod backend;
mod document;
mod features;
mod schema;

use backend::Backend;
use tower_lsp::{LspService, Server};

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(Backend::new);

    Server::new(stdin, stdout, socket).serve(service).await;
}
