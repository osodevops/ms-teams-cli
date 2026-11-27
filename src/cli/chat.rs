use clap::Subcommand;
use std::time::Instant;

use crate::api::chats::ChatOrder;
use crate::api::{self, GraphClient, PaginationOpts};
use crate::auth;
use crate::config::ConfigFile;
use crate::error::{Result, TeamsError};
use crate::models::chat::{Chat, ChatCreateRequest, ChatUpdateRequest};
use crate::models::member::{AddMemberRequest, ALL_HISTORY};
use crate::models::message::ChatMessageFrom;
use crate::output::{self, OutputFormat};

#[derive(Debug, Subcommand)]
pub enum ChatCommand {
    /// List your chats
    List {
        /// Order the chats; without it Graph's default order applies
        #[arg(long, value_enum)]
        order_by: Option<ChatOrderBy>,
    },
    /// Get a chat by ID
    Get {
        /// Chat ID
        id: String,
    },
    /// Create a new chat
    Create {
        /// Chat type: oneOnOne or group
        #[arg(long, alias = "type", default_value = "group")]
        chat_type: String,
        /// Topic for the chat (group chats only)
        #[arg(long)]
        topic: Option<String>,
        /// Members as comma-separated user IDs; append `:guest` for guest
        /// users (e.g. `<id>,<id>:guest`). Default role is owner.
        #[arg(long, value_delimiter = ',')]
        members: Vec<String>,
    },
    /// Update a chat topic
    Update {
        /// Chat ID
        id: String,
        /// New topic
        #[arg(long)]
        topic: String,
    },
    /// Hide a chat
    Hide {
        /// Chat ID
        chat_id: String,
        /// Your user ID
        #[arg(long)]
        user_id: String,
    },
    /// Unhide a chat
    Unhide {
        /// Chat ID
        chat_id: String,
        /// Your user ID
        #[arg(long)]
        user_id: String,
    },
    /// Chat member operations
    Members {
        #[command(subcommand)]
        command: ChatMemberCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum ChatMemberCommand {
    /// List chat members
    List {
        /// Chat ID
        chat_id: String,
    },
    /// Add a member to a chat
    Add {
        /// Chat ID
        chat_id: String,
        /// User ID to add
        #[arg(long)]
        user_id: String,
        /// Role (owner or member)
        #[arg(long, default_value = "member")]
        role: String,
        /// Chat history the new member can see: `all`, `none`, or an RFC 3339
        /// timestamp (e.g. `2026-11-01T00:00:00Z`) to share messages from then
        /// on. Without it the member sees no earlier history. Group chats only.
        #[arg(long, value_name = "all|none|TIMESTAMP", value_parser = parse_history)]
        history: Option<HistoryShare>,
    },
    /// Remove a member from a chat
    Remove {
        /// Chat ID
        chat_id: String,
        /// Membership ID to remove
        member_id: String,
    },
}

pub async fn run(
    cmd: ChatCommand,
    config: &ConfigFile,
    profile: &str,
    format: OutputFormat,
    pagination: &PaginationOpts,
) -> Result<()> {
    let token = auth::resolve_token(profile).await?;
    let client = GraphClient::new(token, &config.network)?;

    match cmd {
        ChatCommand::List { order_by } => {
            let start = Instant::now();
            let order = order_by.map_or(ChatOrder::Default, ChatOrder::from);
            let chats = api::chats::list_chats(&client, order, pagination).await?;
            if format == OutputFormat::Human {
                let headers = match order {
                    ChatOrder::Default => vec!["ID", "Topic", "Type", "Last Updated"],
                    ChatOrder::Activity => vec!["ID", "Topic", "Type", "Last Message", "From"],
                };
                let rows = chats
                    .items
                    .iter()
                    .map(|c| chat_list_row(c, order))
                    .collect();
                output::table::print_table(headers, rows);
                output::print_more_hint(&chats);
            } else {
                output::print_success_page(format, &chats, start);
            }
            Ok(())
        }

        ChatCommand::Get { id } => {
            let start = Instant::now();
            let chat = api::chats::get_chat(&client, &id).await?;
            output::print_success(format, &chat, start);
            Ok(())
        }

        ChatCommand::Create {
            chat_type,
            topic,
            members,
        } => {
            let start = Instant::now();
            let member_reqs: Vec<AddMemberRequest> = members
                .iter()
                .map(|spec| {
                    let (user_id, role) = parse_member_spec(spec)?;
                    Ok(AddMemberRequest::new(&user_id, vec![role]))
                })
                .collect::<Result<_>>()?;
            let req = ChatCreateRequest {
                chat_type,
                topic,
                members: member_reqs,
            };
            let chat = api::chats::create_chat(&client, &req).await?;
            output::print_success(format, &chat, start);
            Ok(())
        }

        ChatCommand::Update { id, topic } => {
            let start = Instant::now();
            let req = ChatUpdateRequest { topic: Some(topic) };
            let chat = api::chats::update_chat(&client, &id, &req).await?;
            output::print_success(format, &chat, start);
            Ok(())
        }

        ChatCommand::Hide { chat_id, user_id } => {
            let start = Instant::now();
            api::chats::hide_chat(&client, &chat_id, &user_id).await?;
            let result = serde_json::json!({"status": "hidden"});
            output::print_success(format, &result, start);
            Ok(())
        }

        ChatCommand::Unhide { chat_id, user_id } => {
            let start = Instant::now();
            api::chats::unhide_chat(&client, &chat_id, &user_id).await?;
            let result = serde_json::json!({"status": "unhidden"});
            output::print_success(format, &result, start);
            Ok(())
        }

        ChatCommand::Members { command } => run_members(command, &client, format, pagination).await,
    }
}

/// The orders `chat list --order-by` offers. Graph sorts `/me/chats` one way
/// only, by its newest message, so there is a single value; an enum leaves
/// room for another if Graph adds one, and lets clap list it in `--help` and
/// shell completions.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum ChatOrderBy {
    /// Newest message first, with each chat's `lastMessagePreview`
    Activity,
}

impl From<ChatOrderBy> for ChatOrder {
    fn from(order: ChatOrderBy) -> Self {
        match order {
            ChatOrderBy::Activity => ChatOrder::Activity,
        }
    }
}

fn chat_list_row(chat: &Chat, order: ChatOrder) -> Vec<String> {
    let preview = chat.last_message_preview.as_ref();
    let tail = match order {
        ChatOrder::Default => vec![chat.last_updated_date_time.clone()],
        ChatOrder::Activity => vec![
            preview.and_then(|p| p.created_date_time.clone()),
            preview
                .and_then(|p| p.from.as_ref())
                .and_then(ChatMessageFrom::display_name)
                .map(str::to_string),
        ],
    };
    [chat.id.clone(), chat.topic.clone(), chat.chat_type.clone()]
        .into_iter()
        .chain(tail)
        .map(Option::unwrap_or_default)
        .collect()
}

/// Splits a `--members` entry of the form `<user-id>[:role]`.
///
/// Graph requires every member in a chat-creation request to carry an explicit
/// role: `owner` for regular tenant users (all participants of a personal chat
/// are owners) and `guest` for Azure AD guest users. Entries without a suffix
/// default to `owner`.
fn parse_member_spec(spec: &str) -> Result<(String, String)> {
    let (user_id, role) = match spec.split_once(':') {
        Some((user_id, role)) => (user_id, role),
        None => (spec, "owner"),
    };
    if user_id.is_empty() {
        return Err(TeamsError::InvalidInput(format!(
            "empty user ID in --members entry '{spec}'"
        )));
    }
    match role {
        "owner" | "guest" => Ok((user_id.to_string(), role.to_string())),
        other => Err(TeamsError::InvalidInput(format!(
            "invalid role '{other}' in --members entry '{spec}' (expected 'owner' or 'guest')"
        ))),
    }
}

/// How much earlier chat history `chat members add --history` shares.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HistoryShare {
    All,
    None,
    /// An RFC 3339 timestamp, already validated; sent as given.
    Since(String),
}

impl HistoryShare {
    /// The `visibleHistoryStartDateTime` value to send; `None` leaves the
    /// field out, which Graph treats as no history.
    fn visible_history_start(&self) -> Option<&str> {
        match self {
            HistoryShare::All => Some(ALL_HISTORY),
            HistoryShare::None => None,
            HistoryShare::Since(ts) => Some(ts),
        }
    }
}

/// Parses `--history`. Runs inside clap, so a bad value is a usage error
/// (exit code 2) before any token is read or request made.
fn parse_history(value: &str) -> std::result::Result<HistoryShare, String> {
    match value {
        "all" => Ok(HistoryShare::All),
        "none" => Ok(HistoryShare::None),
        // chrono also takes a Unicode minus sign (U+2212) in the offset, which
        // RFC 3339 does not; Graph would get a value it may reject.
        ts if !ts.is_ascii() => Err(format!(
            "expected 'all', 'none' or an RFC 3339 timestamp such as \
             2026-11-01T00:00:00Z; '{ts}' has a non-ASCII character"
        )),
        ts => chrono::DateTime::parse_from_rfc3339(ts)
            .map(|_| HistoryShare::Since(ts.to_string()))
            .map_err(|e| {
                format!(
                    "expected 'all', 'none' or an RFC 3339 timestamp such as \
                     2026-11-01T00:00:00Z ({e})"
                )
            }),
    }
}

async fn run_members(
    cmd: ChatMemberCommand,
    client: &GraphClient,
    format: OutputFormat,
    pagination: &PaginationOpts,
) -> Result<()> {
    match cmd {
        ChatMemberCommand::List { chat_id } => {
            let start = Instant::now();
            let members = api::chats::list_members(client, &chat_id, pagination).await?;
            if format == OutputFormat::Human {
                let headers = vec!["ID", "Display Name", "Roles", "Email"];
                let rows: Vec<Vec<String>> = members
                    .items
                    .iter()
                    .map(|m| {
                        vec![
                            m.id.clone().unwrap_or_default(),
                            m.display_name.clone().unwrap_or_default(),
                            m.roles.as_ref().map(|r| r.join(", ")).unwrap_or_default(),
                            m.email.clone().unwrap_or_default(),
                        ]
                    })
                    .collect();
                output::table::print_table(headers, rows);
                output::print_more_hint(&members);
            } else {
                output::print_success_page(format, &members, start);
            }
            Ok(())
        }

        ChatMemberCommand::Add {
            chat_id,
            user_id,
            role,
            history,
        } => {
            let start = Instant::now();
            let roles = if role == "member" { vec![] } else { vec![role] };
            let mut req = AddMemberRequest::new(&user_id, roles);
            if let Some(ts) = history
                .as_ref()
                .and_then(HistoryShare::visible_history_start)
            {
                req = req.with_visible_history(ts);
            }
            let member = api::chats::add_member(client, &chat_id, &req).await?;
            output::print_success(format, &member, start);
            Ok(())
        }

        ChatMemberCommand::Remove { chat_id, member_id } => {
            let start = Instant::now();
            api::chats::remove_member(client, &chat_id, &member_id).await?;
            let result = serde_json::json!({"status": "removed"});
            output::print_success(format, &result, start);
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_all_maps_to_graphs_all_history_value() {
        let share = parse_history("all").unwrap();
        assert_eq!(share.visible_history_start(), Some("0001-01-01T00:00:00Z"));
    }

    #[test]
    fn history_none_leaves_the_field_out() {
        assert_eq!(parse_history("none").unwrap().visible_history_start(), None);
    }

    #[test]
    fn history_accepts_rfc3339_timestamps_as_given() {
        for ts in ["2026-11-01T00:00:00Z", "2026-11-01T09:30:00.5+01:00"] {
            assert_eq!(parse_history(ts).unwrap().visible_history_start(), Some(ts));
        }
    }

    #[test]
    fn history_rejects_values_that_are_not_rfc3339() {
        for bad in [
            "",
            "All",
            "2026-11-01",
            "2026-11-01 00:00:00",
            "yesterday",
            "2026-11-01T00:00:00\u{2212}05:00",
        ] {
            let err = parse_history(bad).unwrap_err();
            assert!(err.contains("RFC 3339"), "{bad}: {err}");
        }
    }

    #[test]
    fn activity_row_shows_the_newest_message_not_the_update_time() {
        let chat: Chat = serde_json::from_value(serde_json::json!({
            "id": "19:abc@thread.v2",
            "chatType": "group",
            "lastUpdatedDateTime": "2026-06-10T12:42:02Z",
            "lastMessagePreview": {
                "createdDateTime": "2026-09-09T08:22:39Z",
                "from": {"user": {"displayName": "Catalin Lupuleti"}}
            }
        }))
        .unwrap();
        let row = chat_list_row(&chat, ChatOrder::Activity);
        assert_eq!(&row[3..], ["2026-09-09T08:22:39Z", "Catalin Lupuleti"]);
        let row = chat_list_row(&chat, ChatOrder::Default);
        assert_eq!(&row[3..], ["2026-06-10T12:42:02Z"]);
    }

    #[test]
    fn member_spec_defaults_to_owner() {
        let (user_id, role) = parse_member_spec("user-1").unwrap();
        assert_eq!(user_id, "user-1");
        assert_eq!(role, "owner");
    }

    #[test]
    fn member_spec_accepts_guest_suffix() {
        let (user_id, role) = parse_member_spec("user-2:guest").unwrap();
        assert_eq!(user_id, "user-2");
        assert_eq!(role, "guest");
    }

    #[test]
    fn member_spec_rejects_unknown_role() {
        let err = parse_member_spec("user-3:admin").unwrap_err();
        assert!(matches!(err, TeamsError::InvalidInput(_)));
    }

    #[test]
    fn member_spec_rejects_empty_user_id() {
        let err = parse_member_spec(":guest").unwrap_err();
        assert!(matches!(err, TeamsError::InvalidInput(_)));
    }
}
