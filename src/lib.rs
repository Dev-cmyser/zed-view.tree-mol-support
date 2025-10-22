use zed_extension_api as zed;
use zed_extension_api::{LanguageServerId, Result};

struct ViewTreeExtension;

impl zed::Extension for ViewTreeExtension {
    fn new() -> Self {
        Self
    }

    fn language_server_command(
        &mut self,
        _language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<zed::Command> {
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
                eprintln!("view.tree LSP: Please install it with: npm install -g view-tree");
                eprintln!("view.tree LSP: Or link locally: cd path/to/view.tree && npm link");
                Err("Unable to find view-tree-lsp. Please install it globally with npm.".into())
            }
        }
    }
}

zed::register_extension!(ViewTreeExtension);
