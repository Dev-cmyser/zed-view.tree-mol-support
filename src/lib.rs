use std::process::Command;
use zed_extension_api as zed;
use zed_extension_api::{LanguageServerId, Result};

struct ViewTreeExtension {
    did_update: bool,
}

impl ViewTreeExtension {
    fn update_lsp_server(&mut self) {
        if self.did_update {
            return;
        }

        eprintln!("view.tree LSP: Updating LSP server...");

        match Command::new("npm")
            .args(&["install", "-g", "--force", "view-tree-lsp@latest"])
            .output()
        {
            Ok(output) => {
                if output.status.success() {
                    eprintln!("view.tree LSP: Successfully updated LSP server");
                    if !output.stdout.is_empty() {
                        eprintln!("view.tree LSP: {}", String::from_utf8_lossy(&output.stdout));
                    }
                } else {
                    eprintln!(
                        "view.tree LSP: Update failed with status: {}",
                        output.status
                    );
                    if !output.stderr.is_empty() {
                        eprintln!("view.tree LSP: {}", String::from_utf8_lossy(&output.stderr));
                    }
                }
            }
            Err(e) => {
                eprintln!("view.tree LSP: Failed to execute npm update: {}", e);
            }
        }

        self.did_update = true;
    }
}

impl zed::Extension for ViewTreeExtension {
    fn new() -> Self {
        Self { did_update: false }
    }

    fn language_server_command(
        &mut self,
        _language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<zed::Command> {
        // Update LSP server on first call
        self.update_lsp_server();

        match worktree.which("view-tree-lsp") {
            Some(path) => {
                eprintln!("view.tree LSP: Found view-tree-lsp at {}", path);
                Ok(zed::Command {
                    command: path,
                    args: vec!["--stdio".to_string()],
                    env: Default::default(),
                })
            }
            None => {
                eprintln!("view.tree LSP: view-tree-lsp not found in PATH");
                eprintln!("view.tree LSP: Please install it with: npm install -g view-tree-lsp");
                eprintln!("view.tree LSP: Or link locally: cd path/to/view.tree && npm link");
                Err("Unable to find view-tree-lsp. Please install it globally with npm.".into())
            }
        }
    }
}

zed::register_extension!(ViewTreeExtension);
