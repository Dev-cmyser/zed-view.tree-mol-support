use zed_extension_api::settings::LspSettings;
use zed_extension_api::{self as zed, LanguageServerId, Result};

// Constants for the View.Tree LSP
const VIEW_TREE_LSP_GITHUB_REPO: &str = "Dev-cmyser/zed-view.tree-mol-support";
const EXECUTABLE_NAME: &str = "lsp";

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
        let binary_settings = LspSettings::for_worktree("zed-view.tree-mol-support", worktree)
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

        // If the Go binary is available in PATH, use that
        if let Some(path) = worktree.which(EXECUTABLE_NAME) {
            return Ok(ViewTreeBinary {
                path,
                args: binary_args,
            });
        }

        // Always check if server exists before trying to use cached path

        // Download directly from GitHub tag
        eprintln!("LSP Debug: Starting LSP server download process");

        zed::set_language_server_installation_status(
            language_server_id,
            &zed::LanguageServerInstallationStatus::CheckingForUpdate,
        );

        zed::set_language_server_installation_status(
            language_server_id,
            &zed::LanguageServerInstallationStatus::Downloading,
        );

        // Download Go binary release from GitHub
        let source_tarball_url = format!(
            "https://github.com/Dev-cmyser/lsp-view.tree/releases/download/v1.0.1/lsp-go-binary.tar.gz",
        );

        eprintln!("LSP Debug: Downloading from URL: {}", source_tarball_url);

        zed::download_file(
            &source_tarball_url,
            "lsp-go-binary.tar.gz",
            zed::DownloadedFileType::GzipTar,
        )
        .map_err(|err| format!("failed to download Go LSP binary: {err}"))?;

        eprintln!("LSP Debug: Download completed successfully");

        // Check current working directory and files
        let current_dir =
            std::env::current_dir().map_or_else(|_| "unknown".to_string(), |p| format!("{:?}", p));

        let mut files_info = Vec::new();
        if let Ok(entries) = std::fs::read_dir(".") {
            for entry in entries {
                if let Ok(entry) = entry {
                    let file_name = entry.file_name();
                    let file_type = if entry.file_type().map_or(false, |t| t.is_dir()) {
                        "DIR"
                    } else {
                        "FILE"
                    };
                    files_info.push(format!("{} {:?}", file_type, file_name));
                }
            }
        }

        // After extraction by Zed, the directory structure is: lsp-go-binary.tar.gz/
        let source_dir = "lsp-go-binary.tar.gz";

        // Check if the extracted directory exists
        let source_dir_info = match std::fs::metadata(&source_dir) {
            Ok(metadata) => {
                if metadata.is_dir() {
                    let mut contents = Vec::new();
                    if let Ok(entries) = std::fs::read_dir(&source_dir) {
                        for entry in entries {
                            if let Ok(entry) = entry {
                                let file_name = entry.file_name();
                                let file_type = if entry.file_type().map_or(false, |t| t.is_dir()) {
                                    "DIR"
                                } else {
                                    "FILE"
                                };
                                contents.push(format!("{} {:?}", file_type, file_name));
                            }
                        }
                    }

                    // Check for Go binary
                    let binary_path = format!("{}/{}", source_dir, EXECUTABLE_NAME);
                    let binary_info = match std::fs::metadata(&binary_path) {
                        Ok(metadata) => {
                            format!("Go binary exists, size: {} bytes", metadata.len())
                        }
                        Err(e) => format!("Go binary not found: {}", e),
                    };

                    format!(
                        "{} exists with contents: [{}]. {}",
                        source_dir,
                        contents.join(", "),
                        binary_info
                    )
                } else {
                    format!("{} exists but is not a directory", source_dir)
                }
            }
            Err(e) => format!("{} not found: {}", source_dir, e),
        };

        // Create absolute path to the Go binary
        let current_dir =
            std::env::current_dir().map_err(|e| format!("Cannot get current directory: {}", e))?;
        let final_binary_path = current_dir.join(&source_dir).join(EXECUTABLE_NAME);
        let final_binary_path_str = final_binary_path.to_string_lossy().to_string();

        // Check if the Go binary exists and make it executable
        match std::fs::metadata(&final_binary_path) {
            Ok(metadata) => {
                eprintln!("LSP Debug: Go binary found, size: {} bytes", metadata.len());
                
                // Make binary executable on Unix systems
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let mut perms = std::fs::metadata(&final_binary_path)?.permissions();
                    perms.set_mode(0o755);
                    std::fs::set_permissions(&final_binary_path, perms)?;
                }
                
                self.cached_binary_path = Some(source_dir.to_string());

                Ok(ViewTreeBinary {
                    path: final_binary_path_str,
                    args: binary_args,
                })
            },
            Err(err) => {
                Err(format!("Go LSP binary not found after download: {}. Working dir: {}. Files in current dir: [{}]. Source dir status: {}. Final path: {}",
                    err, current_dir.display(), files_info.join(", "), source_dir_info, final_binary_path_str))
            }
        }
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
