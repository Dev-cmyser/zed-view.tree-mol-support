use std::fs;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};

use zed_extension_api::{
    self as zed, settings::LspSettings, LanguageServerId, Result, Worktree,
};

pub mod lsp_server;

/// Structure to hold LSP binary information
struct ViewTreeLspBinary {
    path: String,
    args: Option<Vec<String>>,
}

/// Main Zed extension structure
struct ViewTreeLspExtension {
    cached_binary_path: Option<String>,
    lsp_process: Arc<Mutex<Option<Child>>>,
}

impl ViewTreeLspExtension {
    /// Get or create the LSP server binary
    fn language_server_binary(
        &mut self,
        language_server_id: &LanguageServerId,
        worktree: &Worktree,
    ) -> Result<ViewTreeLspBinary> {
        // Check for custom LSP settings
        let binary_settings = LspSettings::for_worktree("view-tree-lsp", worktree)
            .ok()
            .and_then(|lsp_settings| lsp_settings.binary);
        let binary_args = binary_settings
            .as_ref()
            .and_then(|binary_settings| binary_settings.arguments.clone());

        // If user specified custom path, use that
        if let Some(path) = binary_settings.and_then(|binary_settings| binary_settings.path) {
            return Ok(ViewTreeLspBinary {
                path,
                args: binary_args,
            });
        }

        // Check if binary exists in PATH
        if let Some(path) = worktree.which("view-tree-lsp") {
            return Ok(ViewTreeLspBinary {
                path,
                args: binary_args,
            });
        }

        // If we have cached binary, use it
        if let Some(path) = &self.cached_binary_path {
            if fs::metadata(path).map_or(false, |stat| stat.is_file()) {
                return Ok(ViewTreeLspBinary {
                    path: path.clone(),
                    args: binary_args,
                });
            }
        }

        // Create embedded LSP server binary
        self.create_embedded_lsp_binary(language_server_id)
            .map(|path| ViewTreeLspBinary {
                path,
                args: binary_args,
            })
    }

    /// Create embedded LSP server binary
    fn create_embedded_lsp_binary(&mut self, language_server_id: &LanguageServerId) -> Result<String> {
        zed::set_language_server_installation_status(
            language_server_id,
            &zed::LanguageServerInstallationStatus::CheckingForUpdate,
        );

        let binary_name = "view-tree-lsp";
        let version = "0.1.0";
        let version_dir = format!("{}-{}", binary_name, version);
        
        // Create version directory
        fs::create_dir_all(&version_dir)
            .map_err(|err| format!("failed to create directory '{}': {}", version_dir, err))?;

        let binary_path = format!("{}/{}", version_dir, binary_name);

        // Check if binary already exists
        if !fs::metadata(&binary_path).map_or(false, |stat| stat.is_file()) {
            zed::set_language_server_installation_status(
                language_server_id,
                &zed::LanguageServerInstallationStatus::Downloading,
            );

            // Create the embedded LSP server
            self.build_embedded_server(&binary_path)?;

            zed::make_file_executable(&binary_path)?;

            // Clean up old versions
            let entries = fs::read_dir(".")
                .map_err(|err| format!("failed to list working directory: {}", err))?;
            
            for entry in entries {
                let entry = entry.map_err(|err| format!("failed to load directory entry: {}", err))?;
                if entry.file_name().to_str() != Some(&version_dir)
                    && entry.file_name().to_str().map_or(false, |name| {
                        name.starts_with(&format!("{}-", binary_name))
                    })
                {
                    fs::remove_dir_all(entry.path()).ok();
                }
            }
        }

        self.cached_binary_path = Some(binary_path.clone());
        Ok(binary_path)
    }

    /// Build the embedded LSP server
    fn build_embedded_server(&self, binary_path: &str) -> Result<()> {
        // Since we're embedding the LSP server in the same binary,
        // we need to create a wrapper script that starts our embedded server
        let wrapper_script = self.create_lsp_wrapper_script()?;
        
        fs::write(binary_path, wrapper_script)
            .map_err(|err| format!("failed to write LSP wrapper: {}", err))?;

        Ok(())
    }

    /// Create a wrapper script that launches our embedded LSP server
    fn create_lsp_wrapper_script(&self) -> Result<String> {
        let script = r#"#!/bin/bash
# ViewTree LSP Server Wrapper
# This script runs the embedded Rust LSP server

# Get the directory of this script
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# For now, we'll use a simple approach - call back into this extension
# In a full implementation, you'd either:
# 1. Compile a separate LSP binary
# 2. Use the embedded server directly
# 3. Start the LSP server as a subprocess

# Start the embedded LSP server
exec "$SCRIPT_DIR/../view-tree-lsp-embedded" "$@"
"#;
        Ok(script.to_string())
    }

    /// Start the embedded LSP server process
    fn start_embedded_lsp_server(&self) -> Result<Child> {
        // In a real implementation, you would:
        // 1. Spawn the LSP server as a separate process
        // 2. Set up stdio communication
        // 3. Handle the LSP protocol

        // For this example, we'll create a placeholder process
        // In practice, you'd start the actual LSP server here
        let child = Command::new("cat") // Placeholder command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|err| format!("failed to start LSP server: {}", err))?;

        Ok(child)
    }
}

impl zed::Extension for ViewTreeLspExtension {
    fn new() -> Self {
        Self {
            cached_binary_path: None,
            lsp_process: Arc::new(Mutex::new(None)),
        }
    }

    fn language_server_command(
        &mut self,
        language_server_id: &LanguageServerId,
        worktree: &Worktree,
    ) -> Result<zed::Command> {
        // Get the LSP binary
        let binary = self.language_server_binary(language_server_id, worktree)?;

        // For embedded LSP, we need to start our server differently
        // This is a simplified approach - in practice you'd handle the full LSP lifecycle
        
        Ok(zed::Command {
            command: binary.path,
            args: binary.args.unwrap_or_else(|| vec!["--stdio".to_string()]),
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

// Register the extension with Zed
zed::register_extension!(ViewTreeLspExtension);

// Alternative approach: Direct LSP integration
// This would be used if you want to run the LSP server directly in the extension process
// rather than as a separate binary

#[cfg(feature = "embedded-lsp")]
mod embedded {
    use super::*;
    use tokio::runtime::Runtime;

    pub struct EmbeddedLspExtension {
        runtime: Option<Runtime>,
    }

    impl EmbeddedLspExtension {
        fn start_lsp_runtime(&mut self) -> Result<()> {
            if self.runtime.is_none() {
                let rt = Runtime::new()
                    .map_err(|err| format!("failed to create tokio runtime: {}", err))?;
                
                // Start the LSP server in the background
                rt.spawn(async {
                    lsp_server::start_server().await;
                });

                self.runtime = Some(rt);
            }
            Ok(())
        }
    }

    impl zed::Extension for EmbeddedLspExtension {
        fn new() -> Self {
            Self { runtime: None }
        }

        fn language_server_command(
            &mut self,
            language_server_id: &LanguageServerId,
            worktree: &Worktree,
        ) -> Result<zed::Command> {
            // Start the embedded LSP server
            self.start_lsp_runtime()?;

            // Return a command that connects to our embedded server
            // This is a simplified example - real implementation would be more complex
            Ok(zed::Command {
                command: "view-tree-lsp-embedded".to_string(),
                args: vec!["--stdio".to_string()],
                env: Default::default(),
            })
        }
    }
}

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