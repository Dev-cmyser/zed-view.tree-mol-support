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

        match zed::npm_install_package("view-tree-lsp", "latest") {
            Ok(()) => {
                eprintln!("view.tree LSP: Successfully updated LSP server");
                self.did_update = true;
            }
            Err(e) => {
                // TODO: Note that the eprintln will not be visible to users that install the extension, you'd need to write the entire message as an Err instead
                eprintln!("view.tree LSP: Failed to install npm package: {}", e);
            }
        }
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
