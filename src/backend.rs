//! LSP backend and document store.
//! This is the `tower-lsp` entry point: it owns the open-document map, the shared tree-sitter
//! parser, and the in-memory schema docs used by hover and datatype diagnostics.
//! Request handlers stay thin here and delegate document-specific work to the feature modules.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};
use tree_sitter::Parser;

use crate::config::{ServerConfig, expand_schema_candidates, parse_server_config};
use crate::diagnostics::datatype;
use crate::document::Document;
use crate::features::{definition, hover, references};
use crate::schema::{
    SchemaDocCollection, inspect_local_schema_name, load_local_schema, normalize_name,
};

#[derive(Debug, Default)]
struct SchemaConfigState {
    forced_schema_name: Option<String>,
    additional_schema_paths: HashMap<String, std::path::PathBuf>,
    pending_init_config: Option<ServerConfig>,
}

pub struct Backend {
    client: Client,
    documents: Arc<RwLock<HashMap<Url, Document>>>,
    parser: Arc<RwLock<Parser>>,
    schema_docs: Arc<RwLock<SchemaDocCollection>>,
    schema_config: Arc<RwLock<SchemaConfigState>>,
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
            schema_docs: Arc::new(RwLock::new(SchemaDocCollection::new())),
            schema_config: Arc::new(RwLock::new(SchemaConfigState::default())),
        }
    }

    async fn check_schema_support(&self, document: &Document) {
        let forced_schema_name = self.schema_config.read().await.forced_schema_name.clone();

        if let Some(forced_schema_name) = forced_schema_name.as_deref()
            && let Some(document_schema_name) = document.schema_name.as_deref()
        {
            let normalized_document_schema_name = normalize_name(document_schema_name);
            if normalized_document_schema_name != forced_schema_name {
                self.client
                    .show_message(
                        MessageType::WARNING,
                        format!(
                            "This IFC file declares schema `{}`, but the server is configured to force schema `{}`. Diagnostics and hover information use the forced schema.",
                            normalized_document_schema_name,
                            forced_schema_name
                        ),
                    )
                    .await;
            }
        }

        if self.selected_schema_name(document).await.is_none() {
            self.client
                .show_message(
                    MessageType::WARNING,
                    "This IFC file uses an unknown or unsupported schema version. Schema-aware diagnostics and hover information may be incomplete.",
                )
                .await;
        }
    }

    async fn collect_diagnostics(&self, document: &Document) -> Vec<Diagnostic> {
        let selected_schema_name = self.selected_schema_name(&document).await;
        if let Some(schema_name) = selected_schema_name.as_deref() {
            let schema_docs = self.schema_docs.read().await;
            schema_docs
                .get(schema_name)
                .map(|schema| {
                    datatype::collect_with_schema_name(&document, schema, Some(schema_name))
                })
                .unwrap_or_default()
        } else {
            Vec::new()
        }
    }

    async fn publish_document_diagnostics(&self, uri: &Url, diagnostics: Vec<Diagnostic>) {
        self.client
            .publish_diagnostics(uri.clone(), diagnostics, None)
            .await;
    }

    async fn load_text_as_active_document(&self, uri: &Url, text: String) -> Vec<Diagnostic> {
        let mut documents = self.documents.write().await;
        for (document_uri, document) in documents.iter_mut() {
            if document_uri != uri {
                document.unload_parse_state();
            }
        }

        let document = documents
            .entry(uri.clone())
            .or_insert_with(|| Document::new_unloaded(String::new()));
        document.unload_parse_state();
        document.text = text;

        {
            let mut parser = self.parser.write().await;
            document.reload_parse_state(&mut parser);
        }

        self.check_schema_support(document).await;
        self.collect_diagnostics(document).await
    }

    async fn ensure_document_loaded(
        &self,
        documents: &mut HashMap<Url, Document>,
        uri: &Url,
    ) -> Option<Option<Vec<Diagnostic>>> {
        if documents.get(uri)?.is_parse_state_loaded() {
            return Some(None);
        }

        for (document_uri, document) in documents.iter_mut() {
            if document_uri != uri {
                document.unload_parse_state();
            }
        }

        {
            let mut parser = self.parser.write().await;
            documents.get_mut(uri)?.reload_parse_state(&mut parser);
        }

        let diagnostics = {
            let document = documents.get(uri)?;
            self.collect_diagnostics(document).await
        };

        Some(Some(diagnostics))
    }

    async fn request_time_diagnostics(&self, diagnostics: Option<Vec<Diagnostic>>, uri: &Url) {
        if let Some(diagnostics) = diagnostics {
            self.publish_document_diagnostics(uri, diagnostics).await;
        }
    }

    async fn selected_schema_name(&self, document: &Document) -> Option<String> {
        let (forced_schema_name, additional_path) = {
            let schema_config = self.schema_config.read().await;
            let forced_schema_name = schema_config.forced_schema_name.clone();
            let additional_path = document.schema_name.as_ref().and_then(|schema_name| {
                schema_config
                    .additional_schema_paths
                    .get(&normalize_name(schema_name))
                    .cloned()
            });
            (forced_schema_name, additional_path)
        };

        if let Some(schema_name) = forced_schema_name {
            return Some(schema_name);
        }

        let schema_name = document.schema_name.as_ref()?;
        let normalized = normalize_name(schema_name);

        {
            let schema_docs = self.schema_docs.read().await;
            if schema_docs.get(&normalized).is_some() {
                return Some(normalized);
            }
        }

        let path = additional_path?;
        match load_local_schema(&path) {
            Ok((loaded_schema_name, schema)) => {
                let normalized_loaded = normalize_name(&loaded_schema_name);
                let mut schema_docs = self.schema_docs.write().await;
                if schema_docs.get(&normalized_loaded).is_none() {
                    schema_docs.insert(&loaded_schema_name, schema);
                }
                Some(normalized_loaded)
            }
            Err(error) => {
                self.client
                    .log_message(MessageType::WARNING, error.to_string())
                    .await;
                None
            }
        }
    }

    async fn apply_config(&self, config: ServerConfig) {
        let mut schema_docs = SchemaDocCollection::new();
        let mut next_state = SchemaConfigState::default();

        if let Some(path) = config.overwrite_exp_schema_with_local.as_ref() {
            match load_local_schema(path) {
                Ok((schema_name, schema)) => {
                    next_state.forced_schema_name = Some(normalize_name(&schema_name));
                    schema_docs.insert(&schema_name, schema);
                }
                Err(error) => {
                    self.client
                        .log_message(MessageType::WARNING, error.to_string())
                        .await;
                }
            }
        }

        for path in &config.add_local_schema_to_selection {
            for candidate in expand_schema_candidates(path) {
                match inspect_local_schema_name(&candidate) {
                    Ok(schema_name) => {
                        let normalized = normalize_name(&schema_name);
                        if let Some(previous) = next_state
                            .additional_schema_paths
                            .insert(normalized.clone(), candidate.clone())
                        {
                            self.client
                                .log_message(
                                    MessageType::WARNING,
                                    format!(
                                        "Duplicate local schema `{}` configured at `{}` and `{}`; using `{}`",
                                        normalized,
                                        previous.display(),
                                        candidate.display(),
                                        candidate.display()
                                    ),
                                )
                                .await;
                        }
                    }
                    Err(error) => {
                        self.client
                            .log_message(MessageType::WARNING, error.to_string())
                            .await;
                    }
                }
            }
        }

        *self.schema_docs.write().await = schema_docs;
        *self.schema_config.write().await = next_state;
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        let pending_init_config = params
            .initialization_options
            .as_ref()
            .map(parse_server_config)
            .unwrap_or_default();

        let mut schema_config = self.schema_config.write().await;
        schema_config.pending_init_config = Some(pending_init_config);

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

        let load_errors = self.schema_docs.read().await.load_errors().to_vec();
        for error in load_errors {
            self.client.log_message(MessageType::WARNING, error).await;
        }

        let pending_init_config = {
            let mut schema_config = self.schema_config.write().await;
            schema_config.pending_init_config.take()
        };

        if let Some(config) = pending_init_config {
            self.apply_config(config).await;
        }
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

        let diagnostics = self.load_text_as_active_document(&uri, text).await;
        self.publish_document_diagnostics(&uri, diagnostics).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        if let Some(change) = params.content_changes.into_iter().next() {
            let diagnostics = self.load_text_as_active_document(&uri, change.text).await;
            self.publish_document_diagnostics(&uri, diagnostics).await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;

        let mut documents = self.documents.write().await;
        documents.remove(&uri);
        drop(documents);

        self.client.publish_diagnostics(uri, Vec::new(), None).await;
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        let forced_schema_name = self.schema_config.read().await.forced_schema_name.clone();

        let mut documents = self.documents.write().await;
        let diagnostics = match self.ensure_document_loaded(&mut documents, &uri).await {
            Some(diagnostics) => diagnostics,
            None => return Ok(None),
        };
        let document = match documents.get(&uri) {
            Some(document) => document,
            None => return Ok(None),
        };

        let node_message = document.node_at_position(position).map(|node| {
            format!(
                "Node at cursor: kind={}, text={}",
                node.kind(),
                node.utf8_text(document.text.as_bytes()).unwrap_or("?")
            )
        });
        let schema_docs = self.schema_docs.read().await;

        let result = hover::hover(
            document,
            position,
            &schema_docs,
            forced_schema_name.as_deref(),
        );
        drop(schema_docs);
        drop(documents);

        if let Some(message) = node_message {
            self.client.log_message(MessageType::INFO, message).await;
        }
        self.request_time_diagnostics(diagnostics, &uri).await;

        Ok(result)
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        let mut documents = self.documents.write().await;
        let diagnostics = match self.ensure_document_loaded(&mut documents, &uri).await {
            Some(diagnostics) => diagnostics,
            None => return Ok(None),
        };
        let document = match documents.get(&uri) {
            Some(document) => document,
            None => return Ok(None),
        };

        let result = definition::goto_definition(&uri, document, position);
        drop(documents);

        self.request_time_diagnostics(diagnostics, &uri).await;

        Ok(result)
    }

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;

        let mut documents = self.documents.write().await;
        let diagnostics = match self.ensure_document_loaded(&mut documents, &uri).await {
            Some(diagnostics) => diagnostics,
            None => return Ok(None),
        };
        let document = match documents.get(&uri) {
            Some(document) => document,
            None => return Ok(None),
        };

        let result = references::find_references(&uri, document, position);
        drop(documents);

        self.request_time_diagnostics(diagnostics, &uri).await;

        Ok(result)
    }
}
