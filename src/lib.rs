use zed_extension_api::settings::LspSettings;
use zed_extension_api::{self as zed, LanguageServerId, Result};

struct ViewTreeBinary {
    path: String,
    args: Option<Vec<String>>,
}

struct ViewTreeLSPExtension;

impl ViewTreeLSPExtension {
    fn language_server_binary(
        &mut self,
        _language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<ViewTreeBinary> {
        eprintln!("[ViewTree LSP] Starting language_server_binary function");
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

        // Find Node.js executable, including NVM installations
        let node_path = if let Some(path) = worktree.which("node") {
            eprintln!("[ViewTree LSP] Found node at: {}", path);
            path
        } else {
            eprintln!("[ViewTree LSP] Node not found via worktree.which, using fallback");
            // For now, just use "node" and let the system PATH resolve it
            // This should work for most installations including NVM
            "node".to_string()
        };
        eprintln!("[ViewTree LSP] Using node path: {}", node_path);

        // Try to find the LSP server in common locations
        let server_paths = vec![
            "lsp-view.tree/lib/server.js",
            "../lsp-view.tree/lib/server.js",
            "./lsp-view.tree/lib/server.js",
        ];

        eprintln!("[ViewTree LSP] Trying server paths: {:?}", server_paths);
        for server_path in server_paths {
            eprintln!("[ViewTree LSP] Attempting to use server path: {}", server_path);
            // Use the found Node.js path
            return Ok(ViewTreeBinary {
                path: node_path.clone(),
                args: Some(vec![server_path.to_string(), "--stdio".to_string()]),
            });
        }

        // Fallback: just try to run with a default path
        eprintln!("[ViewTree LSP] Using fallback path");
        Ok(ViewTreeBinary {
            path: node_path,
            args: Some(vec!["lsp-view.tree/lib/server.js".to_string(), "--stdio".to_string()]),
        })
    }
}

impl zed::Extension for ViewTreeLSPExtension {
    fn new() -> Self {
        Self
    }

    fn language_server_command(
        &mut self,
        language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<zed::Command> {
        eprintln!("[ViewTree LSP] language_server_command called");
        let binary = self.language_server_binary(language_server_id, worktree)?;
        let command = zed::Command {
            command: binary.path.clone(),
            args: binary.args.clone().unwrap_or_else(|| vec!["--stdio".to_string()]),
            env: Default::default(),
        };
        eprintln!("[ViewTree LSP] Final command: {} with args: {:?}", binary.path, binary.args);
        Ok(command)
    }
}

zed::register_extension!(ViewTreeLSPExtension);