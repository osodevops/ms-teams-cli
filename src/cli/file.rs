use clap::Subcommand;
use std::io::{self, Read as IoRead, Write as IoWrite};
use std::time::Instant;

use crate::api::{self, GraphClient, PaginationOpts};
use crate::auth;
use crate::config::ConfigFile;
use crate::error::Result;
use crate::output::{self, OutputFormat};

#[derive(Debug, Subcommand)]
pub enum FileCommand {
    /// List files in a channel
    List {
        /// Team ID
        #[arg(long)]
        team: String,
        /// Channel ID
        #[arg(long)]
        channel: String,
    },
    /// Get file metadata
    Get {
        /// Team ID
        #[arg(long)]
        team: String,
        /// Channel ID
        #[arg(long)]
        channel: String,
        /// File ID
        #[arg(long)]
        file_id: String,
    },
    /// Upload a file to a channel
    Upload {
        /// Team ID
        #[arg(long)]
        team: String,
        /// Channel ID
        #[arg(long)]
        channel: String,
        /// Local file path to upload
        #[arg(long, conflicts_with = "stdin")]
        file: Option<String>,
        /// Read from stdin
        #[arg(long, conflicts_with = "file")]
        stdin: bool,
        /// Filename (required with --stdin)
        #[arg(long)]
        name: Option<String>,
    },
    /// Download a file from a channel
    Download {
        /// Team ID
        #[arg(long)]
        team: String,
        /// Channel ID
        #[arg(long)]
        channel: String,
        /// File ID
        #[arg(long)]
        file_id: String,
        /// Output file path (defaults to stdout)
        #[arg(long, alias = "output-file")]
        path: Option<String>,
    },
    /// Delete a file from a channel
    Delete {
        /// Team ID
        #[arg(long)]
        team: String,
        /// Channel ID
        #[arg(long)]
        channel: String,
        /// File ID
        #[arg(long)]
        file_id: String,
    },
    /// Create a sharing link for a file
    Share {
        /// Team ID
        #[arg(long)]
        team: String,
        /// Channel ID
        #[arg(long)]
        channel: String,
        /// File ID
        #[arg(long)]
        file_id: String,
        /// Link type (view, edit, embed)
        #[arg(long, default_value = "view")]
        link_type: String,
        /// Link scope (anonymous, organization)
        #[arg(long, default_value = "organization")]
        scope: String,
    },
}

pub async fn run(
    cmd: FileCommand,
    config: &ConfigFile,
    profile: &str,
    format: OutputFormat,
    pagination: &PaginationOpts,
) -> Result<()> {
    let token = auth::resolve_token(profile).await?;
    let client = GraphClient::new(token, &config.network)?;

    match cmd {
        FileCommand::List { team, channel } => {
            let start = Instant::now();
            let files = api::files::list_files(&client, &team, &channel, pagination).await?;
            if format == OutputFormat::Human {
                let headers = vec!["ID", "Name", "Size", "Type", "Modified"];
                let rows: Vec<Vec<String>> = files
                    .iter()
                    .map(|f| {
                        let file_type = if f.folder.is_some() { "folder" } else { "file" };
                        vec![
                            f.id.clone().unwrap_or_default(),
                            f.name.clone().unwrap_or_default(),
                            f.size.map(|s| s.to_string()).unwrap_or_default(),
                            file_type.to_string(),
                            f.last_modified_date_time.clone().unwrap_or_default(),
                        ]
                    })
                    .collect();
                output::table::print_table(headers, rows);
            } else {
                output::print_success_list(format, &files, start);
            }
            Ok(())
        }

        FileCommand::Get {
            team,
            channel,
            file_id,
        } => {
            let start = Instant::now();
            let file = api::files::get_file(&client, &team, &channel, &file_id).await?;
            output::print_success(format, &file, start);
            Ok(())
        }

        FileCommand::Upload {
            team,
            channel,
            file,
            stdin,
            name,
        } => {
            let start = Instant::now();

            let (filename, bytes, content_type) = if stdin {
                let filename = name.ok_or_else(|| {
                    crate::error::TeamsError::InvalidInput(
                        "--name is required when using --stdin".to_string(),
                    )
                })?;
                let mut buf = Vec::new();
                io::stdin().read_to_end(&mut buf).map_err(|e| {
                    crate::error::TeamsError::InvalidInput(format!("Failed to read stdin: {e}"))
                })?;
                let ct = mime_guess::from_path(&filename)
                    .first_or_octet_stream()
                    .to_string();
                (filename, buf, ct)
            } else if let Some(path) = file {
                let filename = name.unwrap_or_else(|| {
                    std::path::Path::new(&path)
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "upload".to_string())
                });
                let bytes = std::fs::read(&path).map_err(|e| {
                    crate::error::TeamsError::InvalidInput(format!(
                        "Failed to read file '{}': {e}",
                        path
                    ))
                })?;
                let ct = mime_guess::from_path(&filename)
                    .first_or_octet_stream()
                    .to_string();
                (filename, bytes, ct)
            } else {
                return Err(crate::error::TeamsError::InvalidInput(
                    "Either --file or --stdin must be provided".to_string(),
                ));
            };

            let item =
                api::files::upload_file(&client, &team, &channel, &filename, bytes, &content_type)
                    .await?;
            output::print_success(format, &item, start);
            Ok(())
        }

        FileCommand::Download {
            team,
            channel,
            file_id,
            path,
        } => {
            let bytes = api::files::download_file(&client, &team, &channel, &file_id).await?;

            if let Some(path) = path {
                std::fs::write(&path, &bytes).map_err(|e| {
                    crate::error::TeamsError::InvalidInput(format!(
                        "Failed to write file '{}': {e}",
                        path
                    ))
                })?;
                if format != OutputFormat::Plain {
                    let start = Instant::now();
                    let result = serde_json::json!({
                        "status": "downloaded",
                        "path": path,
                        "size": bytes.len(),
                    });
                    output::print_success(format, &result, start);
                }
            } else {
                io::stdout().write_all(&bytes).map_err(|e| {
                    crate::error::TeamsError::InvalidInput(format!(
                        "Failed to write to stdout: {e}"
                    ))
                })?;
            }
            Ok(())
        }

        FileCommand::Delete {
            team,
            channel,
            file_id,
        } => {
            let start = Instant::now();
            api::files::delete_file(&client, &team, &channel, &file_id).await?;
            let result = serde_json::json!({"status": "deleted"});
            output::print_success(format, &result, start);
            Ok(())
        }

        FileCommand::Share {
            team,
            channel,
            file_id,
            link_type,
            scope,
        } => {
            let start = Instant::now();
            let link = api::files::create_share_link(
                &client, &team, &channel, &file_id, &link_type, &scope,
            )
            .await?;
            output::print_success(format, &link, start);
            Ok(())
        }
    }
}
