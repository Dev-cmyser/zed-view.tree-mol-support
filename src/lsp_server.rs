use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use lsp_types::{
    CompletionItem, CompletionItemKind, CompletionParams, CompletionResponse,
    DidChangeTextDocumentParams, DidOpenTextDocumentParams, InitializeParams, InitializeResult,
    InitializedParams, Position, ServerCapabilities, TextDocumentSyncCapability,
    TextDocumentSyncKind, Url, CompletionOptions,
};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tower_lsp::jsonrpc::Result;
use tower_lsp::{Client, LanguageServer, LspService, Server};
use walkdir::WalkDir;
use regex::Regex;

/// Project data structure for autocompletion
#[derive(Debug, Clone, Default)]
pub struct ProjectData {
    /// Set of discovered components ($my_component, $mol_view)
    pub components: HashSet<String>,
    /// Properties for each component
    pub component_properties: HashMap<String, HashSet<String>>,
}

/// Main LSP server structure
#[derive(Debug)]
pub struct ViewTreeLspServer {
    client: Client,
    project_data: Arc<RwLock<ProjectData>>,
    workspace_root: Arc<RwLock<Option<PathBuf>>>,
}

impl ViewTreeLspServer {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            project_data: Arc::new(RwLock::new(ProjectData::default())),
            workspace_root: Arc::new(RwLock::new(None)),
        }
    }

    /// Scan the workspace for .view.tree and .ts files
    async fn scan_project(&self) -> Result<()> {
        let workspace_root = self.workspace_root.read().await;
        let root_path = match workspace_root.as_ref() {
            Some(path) => path,
            None => {
                self.client
                    .log_message(lsp_types::MessageType::WARNING, "No workspace root found")
                    .await;
                return Ok(());
            }
        };

        self.client
            .log_message(lsp_types::MessageType::INFO, "Starting project scan...")
            .await;

        let mut data = ProjectData::default();

        // Scan .view.tree files
        for entry in WalkDir::new(root_path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            let path = entry.path();
            
            if let Some(extension) = path.extension() {
                if extension == "tree" && path.to_string_lossy().contains(".view.tree") {
                    self.parse_view_tree_file(path, &mut data).await;
                } else if extension == "ts" && !path.to_string_lossy().ends_with(".d.ts") {
                    self.parse_ts_file(path, &mut data).await;
                }
            }
        }

        let components_count = data.components.len();
        let properties_count = data.component_properties.len();
        
        *self.project_data.write().await = data;
        
        self.client
            .log_message(
                lsp_types::MessageType::INFO,
                format!("Scan complete: {} components, {} components with properties", 
                       components_count, properties_count),
            )
            .await;

        Ok(())
    }

    /// Parse .view.tree file to extract components and properties
    async fn parse_view_tree_file(&self, path: &Path, data: &mut ProjectData) {
        match tokio::fs::read_to_string(path).await {
            Ok(content) => {
                let lines: Vec<&str> = content.lines().collect();
                let mut current_component: Option<String> = None;

                for line in lines {
                    let trimmed = line.trim();

                    // Component definition (line without indent starting with $)
                    if !line.starts_with('\t') && !line.starts_with(' ') && trimmed.starts_with('$') {
                        if let Some(first_word) = trimmed.split_whitespace().next() {
                            if first_word.starts_with('$') {
                                current_component = Some(first_word.to_string());
                                data.components.insert(first_word.to_string());
                                data.component_properties
                                    .entry(first_word.to_string())
                                    .or_insert_with(HashSet::new);
                            }
                        }
                    }

                    // Properties (lines with indent)
                    if let Some(ref component) = current_component {
                        // Property definition
                        let property_regex = Regex::new(r"^\s+([a-zA-Z_][a-zA-Z0-9_?*]*)").unwrap();
                        if let Some(caps) = property_regex.captures(line) {
                            if !trimmed.contains("<=") && !trimmed.contains("<=>") {
                                let property = caps[1].to_string();
                                if !property.starts_with('$') && 
                                   property != "null" && property != "true" && property != "false" {
                                    if let Some(props) = data.component_properties.get_mut(component) {
                                        props.insert(property);
                                    }
                                }
                            }
                        }

                        // Properties in bindings: <= PropertyName
                        let binding_regex = Regex::new(r"<=\s+([a-zA-Z_][a-zA-Z0-9_?*]*)").unwrap();
                        if let Some(caps) = binding_regex.captures(trimmed) {
                            let property = caps[1].to_string();
                            if !property.starts_with('$') {
                                if let Some(props) = data.component_properties.get_mut(component) {
                                    props.insert(property);
                                }
                            }
                        }
                    }
                }
            }
            Err(err) => {
                self.client
                    .log_message(
                        lsp_types::MessageType::ERROR,
                        format!("Error reading file {:?}: {}", path, err),
                    )
                    .await;
            }
        }
    }

    /// Parse .ts file to find components
    async fn parse_ts_file(&self, path: &Path, data: &mut ProjectData) {
        if let Ok(content) = tokio::fs::read_to_string(path).await {
            let component_regex = Regex::new(r"\$\w+").unwrap();
            for cap in component_regex.find_iter(&content) {
                data.components.insert(cap.as_str().to_string());
            }
        }
    }

    /// Get current component based on cursor position
    fn get_current_component(&self, lines: &[&str], current_line: usize) -> Option<String> {
        // Look upward from current line to find component definition
        for i in (0..=current_line).rev() {
            if let Some(line) = lines.get(i) {
                let trimmed = line.trim();
                if !line.starts_with('\t') && !line.starts_with(' ') && trimmed.starts_with('$') {
                    if let Some(first_word) = trimmed.split_whitespace().next() {
                        if first_word.starts_with('$') {
                            return Some(first_word.to_string());
                        }
                    }
                }
            }
        }
        None
    }

    /// Determine completion context based on cursor position
    fn get_completion_context(&self, line_text: &str, character: usize) -> String {
        let before_cursor = &line_text[..character.min(line_text.len())];
        let trimmed = before_cursor.trim();
        let indent_level = before_cursor.len() - before_cursor.trim_start().len();

        if trimmed.starts_with('$') {
            return "component_name".to_string();
        }

        if indent_level == 0 && !trimmed.contains(' ') {
            return "component_name".to_string();
        }

        if indent_level == 0 && trimmed.contains(' ') {
            return "component_extends".to_string();
        }

        if trimmed.contains("<=") {
            return "property_binding".to_string();
        }

        if indent_level > 0 {
            return "property_name".to_string();
        }

        "value".to_string()
    }

    /// Create completion items based on context
    async fn create_completion_items(
        &self,
        context: &str,
        current_component: Option<String>,
    ) -> Vec<CompletionItem> {
        let data = self.project_data.read().await;
        let mut items = Vec::new();

        match context {
            "component_name" | "component_extends" => {
                // Add project components
                for component in &data.components {
                    items.push(CompletionItem {
                        label: component.clone(),
                        kind: Some(CompletionItemKind::CLASS),
                        insert_text: Some(component.clone()),
                        sort_text: Some(format!("1{}", component)),
                        ..Default::default()
                    });
                }
            }
            "property_name" => {
                // Add properties for current component
                if let Some(ref component) = current_component {
                    if let Some(properties) = data.component_properties.get(component) {
                        for property in properties {
                            items.push(CompletionItem {
                                label: property.clone(),
                                kind: Some(CompletionItemKind::PROPERTY),
                                detail: Some(format!("Property of {}", component)),
                                insert_text: Some(property.clone()),
                                sort_text: Some(format!("1{}", property)),
                                ..Default::default()
                            });
                        }
                    }
                }

                // Add general properties if no specific component
                if current_component.is_none() {
                    let mut all_properties = HashSet::new();
                    for properties in data.component_properties.values() {
                        all_properties.extend(properties.iter().cloned());
                    }
                    for property in all_properties {
                        items.push(CompletionItem {
                            label: property.clone(),
                            kind: Some(CompletionItemKind::PROPERTY),
                            insert_text: Some(property.clone()),
                            sort_text: Some(format!("2{}", property)),
                            ..Default::default()
                        });
                    }
                }

                // Add special list marker
                items.push(CompletionItem {
                    label: "/".to_string(),
                    kind: Some(CompletionItemKind::OPERATOR),
                    detail: Some("Empty list".to_string()),
                    insert_text: Some("/".to_string()),
                    sort_text: Some("0/".to_string()),
                    ..Default::default()
                });
            }
            "property_binding" => {
                // Add binding operators
                let operators = [
                    ("<=", "One-way binding"),
                    ("<=>", "Two-way binding"),
                    ("^", "Override"),
                    ("*", "Multi-property marker"),
                ];

                for (op, detail) in &operators {
                    items.push(CompletionItem {
                        label: op.to_string(),
                        kind: Some(CompletionItemKind::OPERATOR),
                        detail: Some(detail.to_string()),
                        insert_text: Some(op.to_string()),
                        ..Default::default()
                    });
                }
            }
            "value" => {
                // Add special values
                let special_values = [
                    ("null", "Null value", None),
                    ("true", "Boolean true", None),
                    ("false", "Boolean false", None),
                    ("\\", "String literal", Some("\\\n\t\\")),
                    ("@\\", "Localized string", Some("@\\\n\t\\")),
                    ("*", "Dictionary marker", None),
                ];

                for (value, detail, insert_text) in &special_values {
                    items.push(CompletionItem {
                        label: value.to_string(),
                        kind: Some(CompletionItemKind::VALUE),
                        detail: Some(detail.to_string()),
                        insert_text: Some(insert_text.unwrap_or(value).to_string()),
                        ..Default::default()
                    });
                }

                // Add components as values too
                for component in &data.components {
                    items.push(CompletionItem {
                        label: component.clone(),
                        kind: Some(CompletionItemKind::CLASS),
                        insert_text: Some(component.clone()),
                        sort_text: Some(format!("3{}", component)),
                        ..Default::default()
                    });
                }
            }
            _ => {}
        }

        items
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for ViewTreeLspServer {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        // Store workspace root
        if let Some(workspace_folders) = params.workspace_folders {
            if let Some(folder) = workspace_folders.first() {
                if let Ok(path) = folder.uri.to_file_path() {
                    *self.workspace_root.write().await = Some(path);
                }
            }
        } else if let Some(root_uri) = params.root_uri {
            if let Ok(path) = root_uri.to_file_path() {
                *self.workspace_root.write().await = Some(path);
            }
        }

        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                completion_provider: Some(CompletionOptions {
                    resolve_provider: Some(true),
                    trigger_characters: Some(vec!["$".to_string(), "_".to_string(), " ".to_string(), "\t".to_string()]),
                    ..Default::default()
                }),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(lsp_types::MessageType::INFO, "ViewTree LSP server initialized!")
            .await;

        // Start project scanning
        if let Err(err) = self.scan_project().await {
            self.client
                .log_message(
                    lsp_types::MessageType::ERROR,
                    format!("Project scan failed: {:?}", err),
                )
                .await;
        }
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, _params: DidOpenTextDocumentParams) {
        // Could implement file-specific logic here
    }

    async fn did_change(&self, _params: DidChangeTextDocumentParams) {
        // Could implement incremental updates here
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let position = params.text_document_position.position;
        let uri = &params.text_document_position.text_document.uri;

        self.client
            .log_message(
                lsp_types::MessageType::INFO,
                format!("Completion request at {}:{}", position.line, position.character),
            )
            .await;

        // In a real implementation, you'd need to track document state
        // For this example, we'll create a mock document content
        // In practice, you'd store document contents in the server state
        
        // Mock: assume we have document content
        let mock_lines = vec!["$my_component $mol_view", "\tsub /", "\t\ttitle \\Hello"];
        let current_line = position.line as usize;
        
        if current_line < mock_lines.len() {
            let line_text = mock_lines[current_line];
            let context = self.get_completion_context(line_text, position.character as usize);
            let current_component = self.get_current_component(&mock_lines, current_line);
            
            self.client
                .log_message(
                    lsp_types::MessageType::INFO,
                    format!("Context: {}, Component: {:?}", context, current_component),
                )
                .await;
            
            let items = self.create_completion_items(&context, current_component).await;
            
            return Ok(Some(CompletionResponse::Array(items)));
        }

        Ok(Some(CompletionResponse::Array(vec![])))
    }
}

/// Create and start the LSP server
pub async fn start_server() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(|client| ViewTreeLspServer::new(client));
    
    Server::new(stdin, stdout, socket).serve(service).await;
}