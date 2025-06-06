use zed_extension_api::settings::LspSettings;
use zed_extension_api::{self as zed, LanguageServerId, Result};

// Constants for the View.Tree LSP
const VIEW_TREE_LSP_GITHUB_REPO: &str = "Dev-cmyser/lsp-view.tree";
const EXECUTABLE_NAME: &str = "lsp-view-tree";

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
        language_server_id: &LanguageServerId,
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

        // Check if Node.js is available and get its path
        let node_path = worktree.which("node").ok_or_else(|| {
            "Node.js is required to run the View.Tree LSP server. Please install Node.js."
        })?;

        // If the server is available as npm package, use that
        if let Some(path) = worktree.which(EXECUTABLE_NAME) {
            return Ok(ViewTreeBinary {
                path,
                args: binary_args,
            });
        }

        // If we've already downloaded the server, use that
        if let Some(path) = &self.cached_binary_path {
            let server_path = format!("{}/lib/server.js", path);
            return Ok(ViewTreeBinary {
                path: node_path.clone(),
                args: Some(vec![server_path, "--stdio".to_string()]),
            });
        }

        // Download directly from GitHub tag
        zed::set_language_server_installation_status(
            language_server_id,
            &zed::LanguageServerInstallationStatus::CheckingForUpdate,
        );

        zed::set_language_server_installation_status(
            language_server_id,
            &zed::LanguageServerInstallationStatus::Downloading,
        );

        // Download source archive from GitHub
        let source_tarball_url = format!(
            "https://github.com/{}/archive/refs/heads/main.tar.gz",
            VIEW_TREE_LSP_GITHUB_REPO
        );

        zed::download_file(
            &source_tarball_url,
            "view-tree-lsp.tar.gz",
            zed::DownloadedFileType::GzipTar,
        )
        .map_err(|err| format!("failed to download source: {err}"))?;

        // Debug: Check current working directory and files
        if let Ok(current_dir) = std::env::current_dir() {
            eprintln!("LSP Debug: Current working directory: {:?}", current_dir);
        }
        
        // List files in current directory
        if let Ok(entries) = std::fs::read_dir(".") {
            eprintln!("LSP Debug: Files in current directory:");
            for entry in entries {
                if let Ok(entry) = entry {
                    eprintln!("LSP Debug: - {:?}", entry.file_name());
                }
            }
        }

        // After extraction, the directory structure should be: lsp-view.tree-main/
        let source_dir = "lsp-view.tree-main";
        let final_server_path = format!("{}/lib/server.js", source_dir);
        
        eprintln!("LSP Debug: Looking for server at: {}", final_server_path);
        
        // Check if the server file exists
        match std::fs::metadata(&final_server_path) {
            Ok(metadata) => eprintln!("LSP Debug: Server file found, size: {} bytes", metadata.len()),
            Err(err) => eprintln!("LSP Debug: Server file not found: {}", err),
        }

        self.cached_binary_path = Some(source_dir.to_string());

        Ok(ViewTreeBinary {
            path: node_path,
            args: Some(vec![final_server_path, "--stdio".to_string()]),
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
