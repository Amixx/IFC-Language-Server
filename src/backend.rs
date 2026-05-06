//! LSP backend and document store.
//! This is the `tower-lsp` entry point: it owns the open-document map, the shared tree-sitter
//! parser, and the in-memory schema docs used by hover and datatype diagnostics.
//! Request handlers stay thin here and delegate document-specific work to the feature modules.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::sync::RwLock;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};
use tree_sitter::Parser;

use crate::diagnostics::datatype;
use crate::document::Document;
use crate::features::{definition, hover, references};
use crate::schema::{
    SchemaDocCollection, inspect_local_schema_name, load_local_schema, normalize_name,
};

#[derive(Clone, Debug, Default)]
struct ServerConfig {
    overwrite_exp_schema_with_local: Option<PathBuf>,
    add_local_schema_to_selection: Vec<PathBuf>,
}

#[derive(Debug, Default)]
struct SchemaConfigState {
    forced_schema_name: Option<String>,
    additional_schema_paths: HashMap<String, PathBuf>,
    workspace_config_supported: bool,
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

    async fn store_document(&self, document: Document, uri: &Url) {
        let mut documents = self.documents.write().await;
        documents.insert(uri.clone(), document);
    }

    async fn check_schema_support(&self, document: &Document) {
        if self.selected_schema_name(document).await.is_none() {
            self.client
                .show_message(
                    MessageType::WARNING,
                    "This IFC file uses an unknown or unsupported schema version. Schema-aware diagnostics and hover information may be incomplete.",
                )
                .await;
        }
    }

    async fn parse_document(&self, uri: &Url, text: String) -> Document {
        let mut parser = self.parser.write().await;
        let document = Document::parse(&mut parser, text);
        drop(parser);
        let selected_schema_name = self.selected_schema_name(&document).await;
        let diagnostics = if let Some(schema_name) = selected_schema_name.as_deref() {
            let schema_docs = self.schema_docs.read().await;
            schema_docs
                .get(schema_name)
                .map(|schema| datatype::collect(&document, schema))
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        self.client
            .publish_diagnostics(uri.clone(), diagnostics, None)
            .await;
        document
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
        let workspace_config_supported = {
            let schema_config = self.schema_config.read().await;
            schema_config.workspace_config_supported
        };

        let mut schema_docs = SchemaDocCollection::new();
        let mut next_state = SchemaConfigState {
            workspace_config_supported,
            ..Default::default()
        };

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

    async fn fetch_workspace_config(&self) -> Option<ServerConfig> {
        let items = vec![ConfigurationItem {
            section: Some("ifcLsp".to_string()),
            ..Default::default()
        }];
        let values = self.client.configuration(items).await.ok()?;
        let value = values.into_iter().next()?;
        Some(parse_server_config(&value))
    }

    async fn revalidate_open_documents(&self) {
        let documents = {
            let documents = self.documents.read().await;
            documents
                .iter()
                .map(|(uri, document)| (uri.clone(), document.text.clone()))
                .collect::<Vec<_>>()
        };

        for (uri, text) in documents {
            let document = self.parse_document(&uri, text).await;
            self.check_schema_support(&document).await;
            self.store_document(document, &uri).await;
        }
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        let workspace_config_supported = params
            .capabilities
            .workspace
            .as_ref()
            .and_then(|workspace| workspace.configuration)
            .unwrap_or(false);
        let pending_init_config = params
            .initialization_options
            .as_ref()
            .map(parse_server_config)
            .unwrap_or_default();

        let mut schema_config = self.schema_config.write().await;
        schema_config.workspace_config_supported = workspace_config_supported;
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

        let (pending_init_config, workspace_config_supported) = {
            let mut schema_config = self.schema_config.write().await;
            (
                schema_config.pending_init_config.take(),
                schema_config.workspace_config_supported,
            )
        };

        if let Some(config) = pending_init_config {
            self.apply_config(config).await;
        }

        if workspace_config_supported
            && let Some(workspace_config) = self.fetch_workspace_config().await
        {
            self.apply_config(workspace_config).await;
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

        let document = self.parse_document(&uri, text).await;
        self.check_schema_support(&document).await;
        self.store_document(document, &uri).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        if let Some(change) = params.content_changes.into_iter().next() {
            let document = self.parse_document(&uri, change.text).await;
            self.check_schema_support(&document).await;
            self.store_document(document, &uri).await;
        }
    }

    async fn did_change_configuration(&self, params: DidChangeConfigurationParams) {
        let workspace_config_supported = self.schema_config.read().await.workspace_config_supported;
        let config = if workspace_config_supported {
            self.fetch_workspace_config().await
        } else {
            Some(parse_server_config(&params.settings))
        };

        if let Some(config) = config {
            self.apply_config(config).await;
            self.revalidate_open_documents().await;
        }
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        let forced_schema_name = self.schema_config.read().await.forced_schema_name.clone();

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
        let schema_docs = self.schema_docs.read().await;

        Ok(hover::hover(
            document,
            position,
            &schema_docs,
            forced_schema_name.as_deref(),
        ))
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

fn parse_server_config(value: &LSPAny) -> ServerConfig {
    let Some(object) = value.as_object() else {
        return ServerConfig::default();
    };

    ServerConfig {
        overwrite_exp_schema_with_local: object
            .get("overwriteExpSchemaWithLocal")
            .and_then(|value| value.as_str())
            .filter(|value| !value.is_empty())
            .map(PathBuf::from),
        add_local_schema_to_selection: object
            .get("addLocalSchemaToSelection")
            .map(parse_path_list)
            .unwrap_or_default(),
    }
}

fn parse_path_list(value: &LSPAny) -> Vec<PathBuf> {
    if let Some(path) = value.as_str() {
        return vec![PathBuf::from(path)];
    }

    value
        .as_array()
        .into_iter()
        .flat_map(|items| items.iter())
        .filter_map(|item| item.as_str())
        .filter(|item| !item.is_empty())
        .map(PathBuf::from)
        .collect()
}

fn expand_schema_candidates(path: &Path) -> Vec<PathBuf> {
    if path.is_file() {
        return vec![path.to_path_buf()];
    }

    if !path.is_dir() {
        return Vec::new();
    }

    let Ok(entries) = fs::read_dir(path) else {
        return Vec::new();
    };

    entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|candidate| {
            candidate.is_file()
                && candidate
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("exp"))
        })
        .collect()
}
