use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};
use tree_sitter::{Parser, Point};

#[derive(Debug)]
struct Document {
    text: String,
    tree: Option<tree_sitter::Tree>,
}

struct IfcLspBackend {
    client: Client,
    documents: Arc<RwLock<HashMap<Url, Document>>>,
    parser: Arc<RwLock<Parser>>,
}

impl IfcLspBackend {
    fn new(client: Client) -> Self {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_ifc::LANGUAGE.into())
            .expect("Error loading IFC parser");

        Self {
            client,
            documents: Arc::new(RwLock::new(HashMap::new())),
            parser: Arc::new(RwLock::new(parser)),
        }
    }

    async fn parse_document(&self, uri: &Url, text: String) {
        let mut parser = self.parser.write().await;
        let tree = parser.parse(&text, None);

        let mut documents = self.documents.write().await;
        documents.insert(uri.clone(), Document { text, tree });
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for IfcLspBackend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "IFC LSP server initialized!")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let text = params.text_document.text;

        self.client
            .log_message(MessageType::INFO, format!("Document opened: {}", uri))
            .await;

        self.parse_document(&uri, text).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        if let Some(change) = params.content_changes.into_iter().next() {
            self.parse_document(&uri, change.text).await;
        }
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        let documents = self.documents.read().await;
        let doc = match documents.get(&uri) {
            Some(doc) => doc,
            None => return Ok(None),
        };

        let tree = match &doc.tree {
            Some(tree) => tree,
            None => return Ok(None),
        };

        // Convert LSP position to tree-sitter point
        let point = Point {
            row: position.line as usize,
            column: position.character as usize,
        };

        // Find the node at cursor position
        let node = tree.root_node().descendant_for_point_range(point, point);

        if let Some(node) = node {
            self.client
                .log_message(
                    MessageType::INFO,
                    format!(
                        "Node at cursor: kind={}, text={}",
                        node.kind(),
                        node.utf8_text(doc.text.as_bytes()).unwrap_or("?")
                    ),
                )
                .await;

            // Check if we're on an entity_name node
            if node.kind() == "entity_name" {
                let entity_text = node.utf8_text(doc.text.as_bytes()).unwrap_or("");

                // Return dummy documentation for now
                let hover_text = format!(
                    "**{}**\n\n\
                    This is an IFC entity.\n\n\
                    *Documentation coming soon!*\n\n\
                    (Node type: `{}`)",
                    entity_text,
                    node.kind()
                );

                return Ok(Some(Hover {
                    contents: HoverContents::Markup(MarkupContent {
                        kind: MarkupKind::Markdown,
                        value: hover_text,
                    }),
                    range: None,
                }));
            }
        }

        // Return generic info if not on an entity name
        Ok(Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: "Hover over an IFC entity name (like `IFCWALL`) to see documentation."
                    .to_string(),
            }),
            range: None,
        }))
    }
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(|client| IfcLspBackend::new(client));

    Server::new(stdin, stdout, socket).serve(service).await;
}
