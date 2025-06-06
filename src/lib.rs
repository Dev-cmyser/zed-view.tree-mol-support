use std::fs;
use zed_extension_api::settings::LspSettings;
use zed_extension_api::{self as zed, LanguageServerId, Result};

// Constants for the View.Tree LSP
const VIEW_TREE_LSP_GITHUB_REPO: &str = "Dev-cmyser/lsp-view.tree";
const SERVER_PATH: &str = "lib/server.js";

struct ViewTreeBinary {
    path: String,
    args: Option<Vec<String>>,
}

struct ViewTreeLSPExtension {
    cached_binary_path: Option<String>,
}

impl ViewTreeLSPExtension {
    fn language_server_binary(
        &mut self,
        _language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<ViewTreeBinary> {
        let binary_settings = LspSettings::for_worktree("view-tree-lsp", worktree)
            .ok()
            .and_then(|lsp_settings| lsp_settings.binary);
        let binary_args = binary_settings
            .as_ref()
            .and_then(|binary_settings| binary_settings.arguments.clone());

        // If the user has specified a custom path to the language server, use that
        if let Some(path) = binary_settings.and_then(|binary_settings| binary_settings.path) {
            return Ok(ViewTreeBinary {
                path,
                args: binary_args,
            });
        }

        // Check if Node.js is available
        if worktree.which("node").is_none() {
            return Err("Node.js is required to run the View.Tree LSP server. Please install Node.js.".into());
        }

        // Try to find the LSP server in common locations
        let server_paths = vec![
            "lsp-view.tree/lib/server.js",
            "../lsp-view.tree/lib/server.js",
            "./lsp-view.tree/lib/server.js",
        ];

        for server_path in server_paths {
            // For now, just try the path and let Node.js handle the error if file doesn't exist
            return Ok(ViewTreeBinary {
                path: "node".to_string(),
                args: Some(vec![server_path.to_string(), "--stdio".to_string()]),
            });
        }

        // Fallback: just try to run with a default path
        Ok(ViewTreeBinary {
            path: "node".to_string(),
            args: Some(vec!["lsp-view.tree/lib/server.js".to_string(), "--stdio".to_string()]),
        })
    }
}

impl zed::Extension for ViewTreeLSPExtension {
    fn new() -> Self {
        Self {
            cached_binary_path: None,
        }
    }

    fn language_server_command(
        &mut self,
        language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<zed::Command> {
        let binary = self.language_server_binary(language_server_id, worktree)?;
        Ok(zed::Command {
            command: binary.path,
            args: binary.args.unwrap_or_else(|| vec!["--stdio".to_string()]),
            env: Default::default(),
        })
    }
}

zed::register_extension!(ViewTreeLSPExtension);