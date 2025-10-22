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
        // Zero-config: require Node in PATH, run server from ../out/server/index.js relative to this crate
        let node = worktree.which("node").unwrap_or_else(|| "node".to_string());

        let server_js = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("out")
            .join("server")
            .join("index.js");
        if !server_js.exists() {
            return Err(format!(
                "Bundled LSP server not found at {}",
                server_js.display()
            ));
        }
        let server_js_str = server_js.to_string_lossy().to_string();

        eprintln!(
            "view.tree LSP (Zed): {} {:?}",
            node,
            vec![server_js_str.clone(), "--stdio".to_string()]
        );

        Ok(zed::Command {
            command: node,
            args: vec![server_js_str, "--stdio".to_string()],
            env: Default::default(),
        })
    }
}

zed::register_extension!(ViewTreeExtension);
