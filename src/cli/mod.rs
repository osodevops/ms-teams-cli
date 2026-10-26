pub mod app;
pub mod auth;
pub mod channel;
pub mod chat;
pub mod config_cmd;
pub mod file;
pub mod listen;
pub mod meeting;
pub mod message;
pub mod message_attachments;
pub mod message_media;
pub mod notification;
pub mod presence;
pub mod search;
pub mod subscribe;
pub mod tab;
pub mod tag;
pub mod team;
pub mod user;

use crate::api::PaginationOpts;
use crate::config::ConfigFile;
use crate::error::{Result, TeamsError};
use crate::output::OutputFormat;
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "teams",
    version = crate::config::version_text(),
    about = "Microsoft Teams CLI — agent-first design"
)]
pub struct Cli {
    /// Output format: json, human, plain (auto-detected from TTY)
    #[arg(short, long, global = true)]
    pub output: Option<String>,

    /// Suppress non-essential output
    #[arg(short, long, global = true)]
    pub quiet: bool,

    /// Increase verbosity (-v, -vv, -vvv)
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// Disable ANSI color codes
    #[arg(long, global = true)]
    pub no_color: bool,

    /// Path to config file
    #[arg(long, global = true)]
    pub config: Option<String>,

    /// Named credential profile (defaults to "default")
    #[arg(long, global = true, env = "TEAMS_CLI_PROFILE")]
    pub profile: Option<String>,

    /// Request timeout in seconds
    #[arg(long, global = true)]
    pub timeout: Option<u64>,

    /// Max retry attempts for transient failures
    #[arg(long, global = true)]
    pub retry: Option<u32>,

    /// Items per page for paginated results
    #[arg(long, global = true)]
    pub page_size: Option<u64>,

    /// Automatically fetch all pages of paginated results
    #[arg(long, global = true)]
    pub all_pages: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Authentication commands
    Auth {
        #[command(subcommand)]
        command: auth::AuthCommand,
    },
    /// User lookup commands
    User {
        #[command(subcommand)]
        command: user::UserCommand,
    },
    /// Configuration management
    Config {
        #[command(subcommand)]
        command: config_cmd::ConfigCommand,
    },
    /// Generate shell completions
    Completions {
        /// Shell to generate completions for
        shell: clap_complete::Shell,
    },
    /// Team operations
    Team {
        #[command(subcommand)]
        command: team::TeamCommand,
    },
    /// Channel operations
    Channel {
        #[command(subcommand)]
        command: channel::ChannelCommand,
    },
    /// Message operations
    Message {
        #[command(subcommand)]
        command: message::MessageCommand,
    },
    /// Chat operations
    Chat {
        #[command(subcommand)]
        command: chat::ChatCommand,
    },
    /// Presence status operations
    Presence {
        #[command(subcommand)]
        command: presence::PresenceCommand,
    },
    /// Search messages, users, and teams
    Search {
        #[command(subcommand)]
        command: search::SearchCommand,
    },
    /// Tag operations for teams
    Tag {
        #[command(subcommand)]
        command: tag::TagCommand,
    },
    /// Online meeting operations
    Meeting {
        #[command(subcommand)]
        command: meeting::MeetingCommand,
    },
    /// Send activity notifications
    Notify {
        #[command(subcommand)]
        command: notification::NotifyCommand,
    },
    /// Installed app operations
    App {
        #[command(subcommand)]
        command: app::AppCommand,
    },
    /// Channel tab operations
    Tab {
        #[command(subcommand)]
        command: tab::TabCommand,
    },
    /// File operations for channels
    File {
        #[command(subcommand)]
        command: file::FileCommand,
    },
    /// Subscription operations for change notifications
    Subscribe {
        #[command(subcommand)]
        command: subscribe::SubscribeCommand,
    },
    /// Start a webhook listener for change notifications
    Listen {
        /// Port to listen on (0 picks a free port)
        #[arg(long, default_value = "8080")]
        port: u16,
        /// Address to bind. Use 0.0.0.0 to accept connections from other hosts
        #[arg(long, default_value = "127.0.0.1")]
        bind: std::net::IpAddr,
        /// Print only notifications carrying this clientState. Prefer the
        /// environment variable: a flag value shows in process listings
        #[arg(long, env = "TEAMS_CLI_CLIENT_STATE", hide_env_values = true)]
        client_state: Option<String>,
    },
}

pub async fn run(cli: Cli, config: &ConfigFile) -> Result<()> {
    let format = OutputFormat::detect(crate::config::resolve_output_format(
        cli.output.as_deref(),
        config,
    ));
    let profile = crate::config::resolve_profile(cli.profile.as_deref(), config).to_string();
    let mut runtime_config = config.clone();
    runtime_config.network =
        crate::config::effective_network_config(config, cli.timeout, cli.retry);
    let pagination = PaginationOpts {
        page_size: crate::config::effective_page_size(config, cli.page_size),
        all_pages: cli.all_pages,
    };

    match cli.command {
        Commands::Auth { command } => {
            auth::run(command, config, cli.config.as_deref(), &profile, format).await
        }
        Commands::User { command } => {
            user::run(command, &runtime_config, &profile, format, &pagination).await
        }
        Commands::Config { command } => {
            config_cmd::run(command, config, cli.config.as_deref(), format).await
        }
        Commands::Completions { shell } => generate_completions(shell),
        Commands::Team { command } => {
            team::run(command, &runtime_config, &profile, format, &pagination).await
        }
        Commands::Channel { command } => {
            channel::run(command, &runtime_config, &profile, format, &pagination).await
        }
        Commands::Message { command } => {
            message::run(command, &runtime_config, &profile, format, &pagination).await
        }
        Commands::Chat { command } => {
            chat::run(command, &runtime_config, &profile, format, &pagination).await
        }
        Commands::Presence { command } => {
            presence::run(command, &runtime_config, &profile, format).await
        }
        Commands::Search { command } => {
            search::run(command, &runtime_config, &profile, format).await
        }
        Commands::Tag { command } => {
            tag::run(command, &runtime_config, &profile, format, &pagination).await
        }
        Commands::Meeting { command } => {
            meeting::run(command, &runtime_config, &profile, format, &pagination).await
        }
        Commands::Notify { command } => {
            notification::run(command, &runtime_config, &profile, format).await
        }
        Commands::App { command } => {
            app::run(command, &runtime_config, &profile, format, &pagination).await
        }
        Commands::Tab { command } => {
            tab::run(command, &runtime_config, &profile, format, &pagination).await
        }
        Commands::File { command } => {
            file::run(command, &runtime_config, &profile, format, &pagination).await
        }
        Commands::Subscribe { command } => {
            subscribe::run(command, &runtime_config, &profile, format, &pagination).await
        }
        Commands::Listen {
            port,
            bind,
            client_state,
        } => listen::run(bind, port, client_state).await,
    }
}

fn generate_completions(shell: clap_complete::Shell) -> Result<()> {
    let output = std::thread::Builder::new()
        .name("teams-completions".into())
        .stack_size(16 * 1024 * 1024)
        .spawn(move || {
            use clap::CommandFactory;

            let mut command = Cli::command();
            let mut output = Vec::new();
            clap_complete::generate(shell, &mut command, "teams", &mut output);
            output
        })
        .map_err(|e| {
            TeamsError::Other(anyhow::anyhow!("Failed to start completion generator: {e}"))
        })?
        .join()
        .map_err(|_| TeamsError::Other(anyhow::anyhow!("Completion generator panicked")))?;

    crate::output::write_stdout(&output);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::Cli;
    use clap::Parser;

    /// Split a shell command line on whitespace, honouring double quotes.
    fn shell_words(line: &str) -> Vec<String> {
        let mut words = Vec::new();
        let mut word = String::new();
        let mut quoted = false;
        let mut in_word = false;
        for c in line.chars() {
            match c {
                '"' => {
                    quoted = !quoted;
                    in_word = true;
                }
                c if c.is_whitespace() && !quoted => {
                    if in_word {
                        words.push(std::mem::take(&mut word));
                        in_word = false;
                    }
                }
                c => {
                    word.push(c);
                    in_word = true;
                }
            }
        }
        if in_word {
            words.push(word);
        }
        words
    }

    /// Replace each `<placeholder>` with a value of the right shape.
    fn fill_placeholders(line: &str) -> String {
        let mut out = String::new();
        let mut rest = line;
        while let Some(start) = rest.find('<') {
            let Some(len) = rest[start..].find('>') else {
                break;
            };
            let name = &rest[start + 1..start + len];
            out.push_str(&rest[..start]);
            out.push_str(if name.contains("url") {
                "https://example.com/x"
            } else {
                "x1"
            });
            rest = &rest[start + len + 1..];
        }
        out.push_str(rest);
        out
    }

    /// Every `teams` line in a fenced block of an agent skill
    /// (`skills/*/SKILL.md`) has to parse, so renaming a flag or subcommand
    /// cannot leave the skills teaching agents a command that fails.
    #[test]
    fn agent_skill_commands_parse() {
        let skills = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("skills");
        let mut checked = 0;
        for entry in std::fs::read_dir(&skills).unwrap() {
            let path = entry.unwrap().path().join("SKILL.md");
            let text = std::fs::read_to_string(&path).unwrap();
            let mut in_block = false;
            for (n, line) in text.lines().enumerate() {
                if line.starts_with("```") {
                    in_block = !in_block;
                    continue;
                }
                if !in_block || !line.starts_with("teams ") {
                    continue;
                }
                if let Err(e) = Cli::try_parse_from(shell_words(&fill_placeholders(line))) {
                    panic!("{}:{}: {line}\n{e}", path.display(), n + 1);
                }
                checked += 1;
            }
        }
        assert!(checked >= 100, "found only {checked} skill commands");
    }

    #[test]
    fn shell_words_honours_double_quotes() {
        assert_eq!(
            shell_words(r#"teams message send --body "Build is green" --chat c"#),
            [
                "teams",
                "message",
                "send",
                "--body",
                "Build is green",
                "--chat",
                "c"
            ]
        );
        assert_eq!(shell_words(r#"a "" b"#), ["a", "", "b"]);
    }
}
