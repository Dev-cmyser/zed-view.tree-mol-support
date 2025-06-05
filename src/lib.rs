use std::fs;
use zed_extension_api::{
    self as zed, settings::LspSettings, LanguageServerId, Result, Worktree,
};

pub mod lsp_server;

/// Main Zed extension structure
struct ViewTreeLspExtension {
    cached_binary_path: Option<String>,
}

impl ViewTreeLspExtension {
    /// Get the LSP server binary path
    fn get_language_server_binary(&mut self, language_server_id: &LanguageServerId, worktree: &Worktree) -> Result<String> {
        // Check for custom LSP settings first
        let binary_settings = LspSettings::for_worktree("view-tree-lsp", worktree)
            .ok()
            .and_then(|lsp_settings| lsp_settings.binary);

        // If user specified custom path, use that
        if let Some(path) = binary_settings.and_then(|binary_settings| binary_settings.path) {
            return Ok(path);
        }

        // Check if binary exists in PATH
        if let Some(path) = worktree.which("view-tree-lsp") {
            return Ok(path);
        }

        // Use cached binary if available
        if let Some(path) = &self.cached_binary_path {
            if fs::metadata(path).map_or(false, |stat| stat.is_file()) {
                return Ok(path.clone());
            }
        }

        // Download and install the LSP server binary
        self.install_language_server_binary(language_server_id)
    }

    /// Install the LSP server binary from GitHub releases
    fn install_language_server_binary(&mut self, language_server_id: &LanguageServerId) -> Result<String> {
        zed::set_language_server_installation_status(
            language_server_id,
            &zed::LanguageServerInstallationStatus::CheckingForUpdate,
        );

        let release = zed::latest_github_release(
            "dev-cmyser/zed-view.tree-mol-support",
            zed::GithubReleaseOptions {
                require_assets: true,
                pre_release: false,
            },
        )?;

        let (platform, arch) = zed::current_platform();
        let asset_name = format!(
            "view-tree-lsp-{}-{}-{}{}",
            release.version,
            match arch {
                zed::Architecture::Aarch64 => "aarch64",
                zed::Architecture::X86 => "x86",
                zed::Architecture::X8664 => "x86_64",
            },
            match platform {
                zed::Os::Mac => "apple-darwin",
                zed::Os::Linux => "unknown-linux-gnu",
                zed::Os::Windows => "pc-windows-msvc",
            },
            if platform == zed::Os::Windows { ".exe" } else { "" }
        );

        let asset = release
            .assets
            .iter()
            .find(|asset| asset.name == asset_name)
            .ok_or_else(|| format!("no asset found matching {:?}", asset_name))?;

        let version_dir = format!("view-tree-lsp-{}", release.version);
        let binary_path = format!("{}/view-tree-lsp{}", version_dir, if platform == zed::Os::Windows { ".exe" } else { "" });

        if !fs::metadata(&binary_path).map_or(false, |stat| stat.is_file()) {
            zed::set_language_server_installation_status(
                language_server_id,
                &zed::LanguageServerInstallationStatus::Downloading,
            );

            zed::download_file(&asset.download_url, &binary_path, zed::DownloadedFileType::Uncompressed)
                .map_err(|err| format!("failed to download LSP server: {err}"))?;

            zed::make_file_executable(&binary_path)?;

            // Clean up old versions
            let entries = fs::read_dir(".").map_err(|err| format!("failed to list directory: {err}"))?;
            for entry in entries {
                let entry = entry.map_err(|err| format!("failed to load directory entry: {err}"))?;
                if entry.file_name().to_str() != Some(&version_dir)
                    && entry.file_name().to_str().map_or(false, |name| name.starts_with("view-tree-lsp-"))
                {
                    fs::remove_dir_all(entry.path()).ok();
                }
            }
        }

        self.cached_binary_path = Some(binary_path.clone());
        Ok(binary_path)
    }
}

impl zed::Extension for ViewTreeLspExtension {
    fn new() -> Self {
        Self {
            cached_binary_path: None,
        }
    }

    fn language_server_command(
        &mut self,
        language_server_id: &LanguageServerId,
        worktree: &Worktree,
    ) -> Result<zed::Command> {
        let command = self.get_language_server_binary(language_server_id, worktree)?;
        
        Ok(zed::Command {
            command,
            args: vec!["--stdio".to_string()],
            env: Default::default(),
        })
    }

    fn language_server_workspace_configuration(
        &mut self,
        _language_server_id: &LanguageServerId,
        worktree: &Worktree,
    ) -> Result<Option<serde_json::Value>> {
        // Return workspace configuration for the LSP server
        let config = serde_json::json!({
            "viewTree": {
                "trace": {
                    "server": "verbose"
                },
                "scanOnStartup": true,
                "maxTsFiles": 100
            }
        });

        Ok(Some(config))
    }
}

zed::register_extension!(ViewTreeLspExtension);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extension_creation() {
        let extension = ViewTreeLspExtension::new();
        assert!(extension.cached_binary_path.is_none());
    }

    #[test]
    fn test_wrapper_script_creation() {
        let extension = ViewTreeLspExtension::new();
        let script = extension.create_lsp_wrapper_script().unwrap();
        assert!(script.contains("#!/bin/bash"));
        assert!(script.contains("ViewTree LSP Server"));
    }
}