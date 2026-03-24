//! LSP backend and document store.
//! Receives protocol requests and forwards them to document and feature logic.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};
use tree_sitter::Parser;

use crate::document::Document;
use crate::features::{definition, hover, references};
use crate::schema::SchemaDocs;

pub struct Backend {
    client: Client,
    documents: Arc<RwLock<HashMap<Url, Document>>>,
    parser: Arc<RwLock<Parser>>,
    schema_docs: SchemaDocs,
}

impl Backend {
    pub fn new(client: Client) -> Self {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_ifc::LANGUAGE.into())
            .expect("Error loading IFC parser");

        Self {
            client,
            documents: Arc::new(RwLock::new(HashMap::new())),
            parser: Arc::new(RwLock::new(parser)),
            schema_docs: SchemaDocs::new(),
        }
    }

    async fn parse_document(&self, uri: &Url, text: String) {
        let mut parser = self.parser.write().await;
        let document = Document::parse(&mut parser, text);

        let mut documents = self.documents.write().await;
        documents.insert(uri.clone(), document);
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                definition_provider: Some(OneOf::Left(true)),
                references_provider: Some(OneOf::Left(true)),
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
        let document = match documents.get(&uri) {
            Some(document) => document,
            None => return Ok(None),
        };

        if let Some(node) = document.node_at_position(position) {
            self.client
                .log_message(
                    MessageType::INFO,
                    format!(
                        "Node at cursor: kind={}, text={}",
                        node.kind(),
                        node.utf8_text(document.text.as_bytes()).unwrap_or("?")
                    ),
                )
                .await;
        }

        Ok(hover::hover(document, position, &self.schema_docs))
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        let documents = self.documents.read().await;
        let document = match documents.get(&uri) {
            Some(document) => document,
            None => return Ok(None),
        };

        Ok(definition::goto_definition(&uri, document, position))
    }

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;

        let documents = self.documents.read().await;
        let document = match documents.get(&uri) {
            Some(document) => document,
            None => return Ok(None),
        };

        Ok(references::find_references(&uri, document, position))
    }
}
