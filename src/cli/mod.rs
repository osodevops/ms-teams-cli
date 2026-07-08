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
pub mod notification;
pub mod presence;
pub mod search;
pub mod subscribe;
pub mod tab;
pub mod tag;
pub mod team;
pub mod user;

use clap::{Parser, Subcommand};

use crate::api::PaginationOpts;
use crate::config::ConfigFile;
use crate::error::Result;
use crate::output::OutputFormat;

#[derive(Debug, Parser)]
#[command(
    name = "teams",
    version,
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

    /// Named credential profile
    #[arg(long, global = true, default_value = "default")]
    pub profile: String,

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
        /// Port to listen on
        #[arg(long, default_value = "8080")]
        port: u16,
    },
}

pub async fn run(cli: Cli, config: &ConfigFile) -> Result<()> {
    let format = OutputFormat::detect(crate::config::resolve_output_format(
        cli.output.as_deref(),
        config,
    ));
    let profile = crate::config::resolve_profile(&cli.profile, config).to_string();
    let mut runtime_config = config.clone();
    runtime_config.network =
        crate::config::effective_network_config(config, cli.timeout, cli.retry);
    let pagination = PaginationOpts {
        page_size: crate::config::effective_page_size(config, cli.page_size),
        all_pages: cli.all_pages,
    };

    match cli.command {
        Commands::Auth { command } => auth::run(command, config, &profile, format).await,
        Commands::User { command } => {
            user::run(command, &runtime_config, &profile, format, &pagination).await
        }
        Commands::Config { command } => {
            config_cmd::run(command, config, cli.config.as_deref(), format).await
        }
        Commands::Completions { shell } => {
            use clap::CommandFactory;
            clap_complete::generate(shell, &mut Cli::command(), "teams", &mut std::io::stdout());
            Ok(())
        }
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
        Commands::Listen { port } => listen::run(port).await,
    }
}
