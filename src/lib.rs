use std::fs;
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

        // Check if Node.js is available
        if worktree.which("node").is_none() {
            return Err("Node.js is required to run the View.Tree LSP server. Please install Node.js.".into());
        }

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
            if fs::metadata(&server_path).map_or(false, |stat| stat.is_file()) {
                return Ok(ViewTreeBinary {
                    path: "node".to_string(),
                    args: Some(vec![server_path, "--stdio".to_string()]),
                });
            }
        }

        // Download the server from GitHub releases
        zed::set_language_server_installation_status(
            language_server_id,
            &zed::LanguageServerInstallationStatus::CheckingForUpdate,
        );
        let release = zed::latest_github_release(
            VIEW_TREE_LSP_GITHUB_REPO,
            zed::GithubReleaseOptions {
                require_assets: true,
                pre_release: false,
            },
        )?;

        // Use source code tarball URL from GitHub
        let source_tarball_url = format!("https://github.com/{}/archive/refs/tags/v{}.tar.gz", VIEW_TREE_LSP_GITHUB_REPO, release.version);

        let version_dir = format!("view-tree-lsp-{}", release.version);
        let source_dir = format!("{}/lsp-view.tree-{}", version_dir, release.version);
        fs::create_dir_all(&version_dir)
            .map_err(|err| format!("failed to create directory '{version_dir}': {err}"))?;

        let server_path = format!("{}/lib/server.js", source_dir);
        if !fs::metadata(&server_path).map_or(false, |stat| stat.is_file()) {
            zed::set_language_server_installation_status(
                language_server_id,
                &zed::LanguageServerInstallationStatus::Downloading,
            );

            // Download and extract the source code
            let tarball_path = format!("{}/source.tar.gz", version_dir);
            zed::download_file(
                &source_tarball_url,
                &tarball_path,
                zed::DownloadedFileType::GzipTar,
            )
            .map_err(|err| format!("failed to download source: {err}"))?;

            // Build the LSP server
            zed::set_language_server_installation_status(
                language_server_id,
                &zed::LanguageServerInstallationStatus::Downloading,
            );

            // Run npm install and build in the source directory
            let npm_install_status = std::process::Command::new("npm")
                .args(&["install"])
                .current_dir(&source_dir)
                .status()
                .map_err(|err| format!("failed to run npm install: {err}"))?;

            if !npm_install_status.success() {
                return Err("npm install failed".into());
            }

            let npm_build_status = std::process::Command::new("npm")
                .args(&["run", "build"])
                .current_dir(&source_dir)
                .status()
                .map_err(|err| format!("failed to run npm build: {err}"))?;

            if !npm_build_status.success() {
                return Err("npm run build failed".into());
            }

            // Clean up old versions
            let entries = fs::read_dir(".")
                .map_err(|err| format!("failed to list working directory {err}"))?;
            for entry in entries {
                let entry = entry.map_err(|err| format!("failed to load directory entry {err}"))?;
                if entry.file_name().to_str() != Some(&version_dir)
                    && entry.file_name().to_str().map_or(false, |name| {
                        name.starts_with("view-tree-lsp-")
                    })
                {
                    fs::remove_dir_all(entry.path()).ok();
                }
            }
        }

        self.cached_binary_path = Some(source_dir.clone());
        let final_server_path = format!("{}/lib/server.js", source_dir);
        
        Ok(ViewTreeBinary {
            path: "node".to_string(),
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