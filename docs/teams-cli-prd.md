# Product Requirements Document (PRD)
# `teams-cli` — A Rust CLI for Microsoft Teams (Agent-First Design)

**Version:** 1.0.0-draft  
**Author:** [Your Name]  
**Date:** March 2026  
**Status:** Draft  
**Language:** Rust  

---

## 1. Executive Summary

`teams-cli` is a Rust-based command-line interface that provides full-featured access to Microsoft Teams via the Microsoft Graph API. Unlike existing tools that are either abandoned (fossteams/teams-cli, archived Oct 2025), send-only (send2teams), or focused on SDK scaffolding (Microsoft's own Teams CLI), `teams-cli` is designed from the ground up for **AI agents operating from the command line** while remaining fully usable by humans.

The tool enables everything a human can do in the Teams desktop/web client — send and read messages, manage teams and channels, handle presence, join meetings, upload files, react to messages, search conversations, and more — all through a composable, pipeable CLI interface with structured JSON output.

### Why This Exists

- **No comprehensive Teams CLI exists.** The only open-source attempt (fossteams/teams-cli in Go) was archived in Oct 2025 with only login and channel listing working.
- **Agent-first gap.** AI agents (Claude, GPT, custom LLM agents) need programmatic Teams access. Existing MCP servers for Teams (e.g., viaSocket, Softeria ms-365-mcp-server) are limited — missing reactions, file uploads, presence, meetings, and search.
- **Developer frustration with Teams APIs.** Microsoft's own ecosystem suffers from constant SDK churn (TeamsFx → Teams AI v2 → M365 Agents SDK), scattered documentation across 5+ domains, and complex permission models. A well-designed CLI abstracts this pain away.
- **Rust advantages.** Single binary distribution, no runtime dependencies, excellent performance, memory safety, and cross-platform support (macOS, Linux, Windows).

---

## 2. Goals & Non-Goals

### Goals

1. **Complete Teams coverage** — Every action a human can perform in Teams should be achievable via CLI commands.
2. **Agent-first design** — Structured JSON output, stdin/stdout piping, non-interactive auth, idempotent operations, deterministic error codes.
3. **MCP server mode** — Built-in Model Context Protocol server so AI agents (Claude Desktop, Cursor, etc.) can use Teams as a tool natively.
4. **Robust auth handling** — Support all Microsoft Identity Platform flows: client credentials, device code, auth code with PKCE, and certificate-based auth. Secure token caching with auto-refresh.
5. **Resilient by default** — Automatic retry with exponential backoff for throttling (429s), graceful degradation, and respect for Microsoft Graph rate limits.
6. **Cross-platform single binary** — Compile for macOS (ARM64/x86_64), Linux (x86_64/ARM64), and Windows (x86_64) with no runtime dependencies.
7. **Real-time event streaming** — Built-in webhook listener and change notification subscription management for real-time message/presence/channel events.
8. **Multi-account support** — Manage multiple tenant/account configurations simultaneously.

### Non-Goals

- Building a TUI (text user interface) with ncurses/ratatui — this is a CLI tool, not a terminal Teams client.
- Replacing the Teams desktop app for daily human use.
- Implementing calling/VoIP functionality (PSTN/VoIP requires media streams beyond CLI scope).
- Building a bot framework — this is a client tool, not a server-side bot.
- Supporting GCC High / DoD / 21Vianet environments in v1.0 (future consideration).

---

## 3. Target Users

| User Type | Use Case |
|-----------|----------|
| **AI Agents** | LLM-powered agents that need to read, send, and manage Teams messages/channels as part of autonomous workflows |
| **DevOps/Platform Engineers** | Automating Teams notifications from CI/CD pipelines, incident response, deployment alerts |
| **Developers** | Scripting Teams interactions, building integrations, testing Graph API workflows |
| **IT Administrators** | Bulk team/channel management, user presence monitoring, policy automation |
| **Power Users** | Quick message sending, channel monitoring, and search without leaving the terminal |

---

## 4. Competitive Landscape & Differentiation

| Feature | fossteams/teams-cli | send2teams | MS Teams CLI | SlackCLI (reference) | **teams-cli (ours)** |
|---------|-------------------|------------|--------------|---------------------|---------------------|
| Language | Go | Go | Node.js | TypeScript/Bun | **Rust** |
| Status | Archived (Oct 2025) | Active (send only) | Active (SDK scaffold only) | Active | **New** |
| Send messages | ❌ | ✅ (webhook only) | ❌ | ✅ | **✅** |
| Read messages | ❌ | ❌ | ❌ | ✅ | **✅** |
| Manage teams/channels | ❌ | ❌ | ❌ | ✅ | **✅** |
| Presence | ❌ | ❌ | ❌ | ✅ | **✅** |
| Meetings | ❌ | ❌ | ❌ | ❌ | **✅** |
| File operations | ❌ | ❌ | ❌ | ✅ | **✅** |
| Reactions | ❌ | ❌ | ❌ | ✅ | **✅** |
| Search | ❌ | ❌ | ❌ | ✅ | **✅** |
| JSON output | ❌ | ❌ | ❌ | ✅ | **✅** |
| MCP server mode | ❌ | ❌ | ❌ | ❌ | **✅** |
| Multi-account | ❌ | ❌ | ❌ | ✅ | **✅** |
| Real-time events | ❌ | ❌ | ❌ | ❌ | **✅** |
| Single binary | ✅ | ✅ | ❌ (Node) | ❌ (Bun) | **✅** |

---

## 5. Technical Architecture

### 5.1 High-Level Architecture

```
┌─────────────────────────────────────────────────┐
│                  teams-cli binary                │
├─────────────┬───────────────┬───────────────────┤
│  CLI Layer  │  MCP Server   │  Webhook Listener │
│  (clap v4)  │  (stdio/sse)  │  (axum)           │
├─────────────┴───────────────┴───────────────────┤
│              Command Router / Orchestrator        │
├─────────────────────────────────────────────────┤
│              Core Service Layer                   │
│  ┌─────────┬──────────┬────────┬──────────────┐ │
│  │ Teams   │ Messages │ Files  │ Meetings     │ │
│  │ Channels│ Chats    │ Search │ Presence     │ │
│  │ Members │ Reactions│ Tags   │ Notifications│ │
│  └─────────┴──────────┴────────┴──────────────┘ │
├─────────────────────────────────────────────────┤
│              Microsoft Graph Client               │
│  ┌──────────────────┬──────────────────────────┐ │
│  │  HTTP Client     │  Rate Limiter / Retry    │ │
│  │  (reqwest)       │  (tower / backoff)       │ │
│  └──────────────────┴──────────────────────────┘ │
├─────────────────────────────────────────────────┤
│              Auth Layer                           │
│  ┌──────────┬──────────┬───────────┬───────────┐ │
│  │ Client   │ Device   │ Auth Code │ Cert-     │ │
│  │ Creds    │ Code     │ + PKCE    │ Based     │ │
│  └──────────┴──────────┴───────────┴───────────┘ │
│  Token Cache (keyring / encrypted file)           │
├─────────────────────────────────────────────────┤
│              Config Manager                       │
│  Multi-account profiles (~/.config/teams-cli/)    │
└─────────────────────────────────────────────────┘
```

### 5.2 Key Crate Dependencies

| Crate | Purpose | Version |
|-------|---------|---------|
| `clap` | CLI argument parsing with derive macros | ^4 |
| `reqwest` | HTTP client for Graph API calls | ^0.12 |
| `tokio` | Async runtime | ^1.25 |
| `serde` / `serde_json` | JSON serialization/deserialization | ^1 |
| `axum` | Webhook listener HTTP server | ^0.7 |
| `tower` | Middleware (rate limiting, retry, timeout) | ^0.4 |
| `keyring` | OS-native credential storage (macOS Keychain, Windows Credential Manager, Linux Secret Service) | ^3 |
| `jsonwebtoken` | JWT handling for token validation | ^9 |
| `oauth2` | OAuth 2.0 client implementation | ^4 |
| `tracing` / `tracing-subscriber` | Structured logging | ^0.1 |
| `tabled` | Human-readable table output | ^0.16 |
| `indicatif` | Progress bars for long operations | ^0.17 |
| `chrono` | Date/time handling | ^0.4 |
| `uuid` | UUID generation for request correlation | ^1 |
| `dirs` | Cross-platform config directory resolution | ^5 |
| `backon` | Retry with backoff strategies | ^1 |

### 5.3 Graph API Versions

- **Primary:** Microsoft Graph v1.0 (`https://graph.microsoft.com/v1.0/`)
- **Beta features:** Microsoft Graph Beta (`https://graph.microsoft.com/beta/`) for reactions, search, advanced presence, policy APIs
- **Configuration flag:** `--api-version <v1.0|beta>` with per-command defaults

---

## 6. Authentication & Authorization

**Commercial auth direction update:** see
[`docs/auth-implementation-plan.md`](auth-implementation-plan.md). The product should
default to a publisher-verified OSO multi-tenant public client app for delegated
Graph auth. Client credentials are not a general solution for normal Teams
message posting because Microsoft Graph only supports app-only channel/chat
message POSTs for migration/import scenarios. Unattended posting should be
implemented with a Teams bot app and Bot Framework proactive messaging.

### 6.1 Supported Auth Flows

| Flow | Use Case | Interactive | Permissions Type |
|------|----------|-------------|-----------------|
| **Client Credentials** | Supported app-only Graph admin/read operations; not normal live Teams message posting | No | Application |
| **Device Code** | CLI sessions where browser is available but not on same machine | Semi (one-time browser step) | Delegated |
| **Auth Code + PKCE** | Interactive human use, initial setup | Yes (opens browser) | Delegated |
| **Certificate-Based** | Enterprise/production deployments | No | Application |

### 6.2 Token Management

- **Encrypted local cache:** Tokens stored via OS keyring (macOS Keychain, Windows Credential Manager, Linux Secret Service/libsecret) with fallback to encrypted file (`~/.config/teams-cli/tokens.enc`).
- **Automatic refresh:** Background token refresh before expiry. The `graph-oauth` crate provides this natively.
- **Multi-account:** Named profiles in `~/.config/teams-cli/config.toml` with independent token stores.
- **Environment variable override:** `TEAMS_CLI_CLIENT_ID`, `TEAMS_CLI_CLIENT_SECRET`, `TEAMS_CLI_TENANT_ID`, `TEAMS_CLI_ACCESS_TOKEN` for CI/CD.

### 6.3 Required Permissions (Scopes)

The tool should request the minimum scopes needed per command. Document required permissions for each command.

**Application Permissions (Client Credentials / Cert):**

Application permissions vary by Graph endpoint and are not interchangeable with
delegated permissions. In particular, normal Teams channel/chat message sending
is delegated-only; app-only message POST is limited to migration/import flows.
The exact app-only permission set must be validated per command before release.

- `Team.ReadBasic.All`, `TeamSettings.ReadWrite.All`
- `Channel.ReadBasic.All`, `ChannelMessage.Read.All`
- `Chat.Read.All`, `Chat.ReadWrite.All`, `ChatMessage.Read.All`
- `ChatMember.Read.All`, `ChatMember.ReadWrite.All`
- `User.Read.All`, `Presence.Read.All`
- `OnlineMeetings.Read.All`, `OnlineMeetings.ReadWrite.All`
- `Files.Read.All`, `Files.ReadWrite.All`
- `TeamsActivity.Send`
- `TeamworkTag.Read.All`, `TeamworkTag.ReadWrite.All`

**Delegated Permissions (Device Code / Auth Code):**
- `Team.ReadBasic.All`, `Team.Create`
- `Channel.ReadBasic.All`, `Channel.Create`, `ChannelMessage.Send`, `ChannelMessage.Read.All`
- `Chat.ReadWrite`, `ChatMessage.Send`, `ChatMessage.Read`
- `User.Read`, `User.ReadBasic.All`
- `Presence.Read.All`, `Presence.ReadWrite`
- `OnlineMeetings.ReadWrite`
- `Files.ReadWrite.All`
- `offline_access` (for refresh tokens)

### 6.4 Auth Commands

```bash
# Interactive login with browser
teams auth login

# Device code flow (for SSH/headless with browser elsewhere)
teams auth login --device-code

# Client credentials (non-interactive)
teams auth login --client-credentials \
  --client-id <id> --client-secret <secret> --tenant-id <tenant>

# Certificate-based
teams auth login --certificate \
  --client-id <id> --cert-path ./cert.pem --tenant-id <tenant>

# Check current auth status
teams auth status

# List all profiles
teams auth list

# Switch active profile
teams auth switch <profile-name>

# Logout / revoke tokens
teams auth logout [--profile <name>] [--all]

# Export token for use in other tools
teams auth token [--format <bearer|jwt|json>]
```

---

## 7. Command Reference

### 7.1 Global Flags

```
--profile <name>        Use specific account profile
--output <format>       Output format: json (default for pipes), table, csv, yaml
--quiet                 Suppress non-essential output
--verbose               Enable debug logging
--dry-run               Show what would be done without executing
--api-version <ver>     Force Graph API version (v1.0 or beta)
--no-color              Disable colored output
--timeout <seconds>     Request timeout (default: 30)
--retry <count>         Max retry attempts for transient failures (default: 3)
--page-size <n>         Items per page for paginated results (default: 50, max: 999)
--all-pages             Automatically fetch all pages of paginated results
```

### 7.2 Teams Management

```bash
# List teams the authenticated user is a member of
teams team list [--filter <odata-filter>] [--select <fields>]

# Get team details
teams team get <team-id>

# Create a new team
teams team create --name "Engineering" --description "Eng team" \
  [--visibility <public|private>] [--template <standard|educationClass|...>]

# Update team settings
teams team update <team-id> --name "New Name" [--description "..."]

# Delete a team
teams team delete <team-id> [--confirm]

# Clone a team
teams team clone <team-id> --name "Cloned Team" \
  [--parts <apps,tabs,settings,channels,members>]

# Archive / unarchive a team
teams team archive <team-id>
teams team unarchive <team-id>

# List team members
teams team members <team-id> [--role <owner|member|guest>]

# Add member to team
teams team add-member <team-id> --user <user-id-or-email> [--role <owner|member>]

# Remove member from team
teams team remove-member <team-id> --user <user-id-or-email> [--confirm]
```

### 7.3 Channel Management

```bash
# List channels in a team
teams channel list <team-id> [--filter <standard|private|shared>]

# Get channel details
teams channel get <team-id> <channel-id>

# Create a channel
teams channel create <team-id> --name "releases" \
  [--description "Release notes"] [--type <standard|private|shared>]

# Update a channel
teams channel update <team-id> <channel-id> --name "New Name"

# Delete a channel
teams channel delete <team-id> <channel-id> [--confirm]

# List channel members (for private/shared channels)
teams channel members <team-id> <channel-id>

# Add/remove channel members
teams channel add-member <team-id> <channel-id> --user <user-id>
teams channel remove-member <team-id> <channel-id> --user <user-id>
```

### 7.4 Messaging

```bash
# Send a message to a channel
teams message send --team <team-id> --channel <channel-id> --body "Hello world"

# Send a message to a chat (1:1 or group)
teams message send --chat <chat-id> --body "Hello"

# Send with rich content
teams message send --team <team-id> --channel <channel-id> \
  --body "<h1>Title</h1><p>Content</p>" --content-type html

# Send with @mention
teams message send --team <team-id> --channel <channel-id> \
  --body "Please review" --mention <user-id>

# Send with importance
teams message send --chat <chat-id> --body "Urgent!" --importance urgent

# Send an Adaptive Card
teams message send --team <team-id> --channel <channel-id> \
  --adaptive-card ./card.json

# Send from stdin (useful for piping)
echo "Build passed ✅" | teams message send --chat <chat-id> --stdin

# Read messages from a channel
teams message list --team <team-id> --channel <channel-id> \
  [--top <n>] [--since <datetime>] [--before <datetime>]

# Read messages from a chat
teams message list --chat <chat-id> [--top <n>]

# Get a specific message
teams message get --team <team-id> --channel <channel-id> --message <msg-id>

# Get replies to a message
teams message replies --team <team-id> --channel <channel-id> --message <msg-id>

# Reply to a message
teams message reply --team <team-id> --channel <channel-id> --message <msg-id> \
  --body "Thanks!"

# Reply from stdin
cat response.md | teams message reply --chat <chat-id> --message <msg-id> --stdin

# Delete a message
teams message delete --team <team-id> --channel <channel-id> --message <msg-id>

# Update/edit a message
teams message update --team <team-id> --channel <channel-id> --message <msg-id> \
  --body "Updated content"

# React to a message
teams message react --team <team-id> --channel <channel-id> --message <msg-id> \
  --reaction "👍"

# Remove a reaction
teams message unreact --team <team-id> --channel <channel-id> --message <msg-id> \
  --reaction "👍"

# Pin a message in a channel
teams message pin --team <team-id> --channel <channel-id> --message <msg-id>
teams message unpin --team <team-id> --channel <channel-id> --message <msg-id>
```

### 7.5 Chat Management

```bash
# List chats for the authenticated user
teams chat list [--filter <oneOnOne|group|meeting>] [--top <n>]

# Get chat details
teams chat get <chat-id>

# Create a new chat
teams chat create --type oneOnOne --members <user-id-1>,<user-id-2>
teams chat create --type group --members <user-id-1>,<user-id-2>,<user-id-3> \
  --topic "Project Discussion"

# Update chat topic
teams chat update <chat-id> --topic "New Topic"

# List chat members
teams chat members <chat-id>

# Add/remove chat members (group chats)
teams chat add-member <chat-id> --user <user-id>
teams chat remove-member <chat-id> --user <user-id>

# Hide/unhide a chat
teams chat hide <chat-id>
teams chat unhide <chat-id>

# Mark chat as read/unread
teams chat mark-read <chat-id>
teams chat mark-unread <chat-id>
```

### 7.6 Presence

```bash
# Get your own presence
teams presence get

# Get a specific user's presence
teams presence get --user <user-id>

# Get presence for multiple users (batch)
teams presence get --users <id1>,<id2>,<id3>

# Set your presence
teams presence set --availability <Available|Busy|DoNotDisturb|Away|BeRightBack|Offline> \
  --activity <Available|InACall|InAMeeting|Presenting|...> \
  [--expiration <duration, e.g. 1h, 30m>]

# Set status message
teams presence status --message "In deep focus until 3pm" \
  [--expiry <datetime>]

# Clear presence (reset to automatic)
teams presence clear

# Watch presence changes for users (real-time via subscription)
teams presence watch --users <id1>,<id2> [--interval <seconds>]
```

### 7.7 Meetings

```bash
# List upcoming meetings
teams meeting list [--start <datetime>] [--end <datetime>]

# Get meeting details
teams meeting get <meeting-id>

# Create an online meeting
teams meeting create --subject "Standup" \
  --start "2026-03-05T10:00:00Z" --end "2026-03-05T10:30:00Z" \
  [--participants <user-id-1>,<user-id-2>] \
  [--lobby-bypass <always|organization|organizationAndFederated|organizer>]

# Update a meeting
teams meeting update <meeting-id> --subject "Updated Standup"

# Delete/cancel a meeting
teams meeting delete <meeting-id> [--confirm]

# Get meeting attendance report
teams meeting attendance <meeting-id>

# Get meeting transcript (if available)
teams meeting transcript <meeting-id> [--format <text|vtt>]

# Get meeting recording info
teams meeting recording <meeting-id>

# Join info / get join URL
teams meeting join-url <meeting-id>
```

### 7.8 Files

```bash
# List files in a channel's SharePoint folder
teams file list --team <team-id> --channel <channel-id> [--path <subfolder>]

# Upload a file to a channel
teams file upload --team <team-id> --channel <channel-id> \
  --file ./report.pdf [--path <subfolder>]

# Upload from stdin
cat data.csv | teams file upload --team <team-id> --channel <channel-id> \
  --name "data.csv" --stdin

# Download a file
teams file download --team <team-id> --channel <channel-id> \
  --file-id <id> [--output ./local-file.pdf]

# Get file metadata
teams file get --team <team-id> --channel <channel-id> --file-id <id>

# Delete a file
teams file delete --team <team-id> --channel <channel-id> --file-id <id> [--confirm]

# Share a file in a message (send as card attachment)
teams file share --team <team-id> --channel <channel-id> \
  --file-id <id> --body "Here's the report"
```

### 7.9 Search

```bash
# Search messages across all Teams chats and channels
teams search messages --query "deployment failed" \
  [--from <user-id>] [--since <datetime>] [--top <n>]

# Search within a specific team
teams search messages --query "quarterly review" --team <team-id>

# Search within a specific channel
teams search messages --query "bug fix" --team <team-id> --channel <channel-id>

# Search for users
teams search users --query "John" [--top <n>]

# Search for teams
teams search teams --query "engineering" [--top <n>]
```

### 7.10 Tags

```bash
# List tags in a team
teams tag list <team-id>

# Get tag details
teams tag get <team-id> <tag-id>

# Create a tag
teams tag create <team-id> --name "Frontend" \
  --members <user-id-1>,<user-id-2>

# Update a tag
teams tag update <team-id> <tag-id> --name "Frontend Engineers"

# Delete a tag
teams tag delete <team-id> <tag-id> [--confirm]

# Add/remove tag members
teams tag add-member <team-id> <tag-id> --user <user-id>
teams tag remove-member <team-id> <tag-id> --user <user-id>
```

### 7.11 Activity Feed Notifications

```bash
# Send an activity feed notification to a user
teams notify send --user <user-id> --topic "New Assignment" \
  --activity-type "taskCreated" \
  --preview "You have a new task assigned"

# Send batch notifications
teams notify send --users <id1>,<id2>,<id3> \
  --topic "Deployment Complete" \
  --preview "v2.3.1 deployed to production"
```

### 7.12 Apps & Tabs

```bash
# List installed apps in a team
teams app list <team-id>

# Install an app to a team
teams app install <team-id> --app-id <catalog-app-id>

# Uninstall an app
teams app uninstall <team-id> --app-id <installation-id>

# List tabs in a channel
teams tab list <team-id> <channel-id>

# Create a tab
teams tab create <team-id> <channel-id> --app-id <app-id> \
  --name "Wiki" --content-url <url>

# Delete a tab
teams tab delete <team-id> <channel-id> --tab-id <id> [--confirm]
```

### 7.13 User Lookup

```bash
# Get current user info
teams user me

# Get user by ID or UPN
teams user get <user-id-or-upn>

# List users in the organization
teams user list [--filter <odata-filter>] [--top <n>]
```

### 7.14 Subscriptions & Real-Time Events

```bash
# Subscribe to channel message events
teams subscribe --resource "/teams/{team-id}/channels/{channel-id}/messages" \
  --change-type "created,updated" \
  --webhook-url <your-endpoint> \
  [--expiration <datetime>]

# Subscribe to chat messages
teams subscribe --resource "/chats/{chat-id}/messages" \
  --change-type "created"

# Subscribe to presence changes
teams subscribe --resource "/communications/presences/{user-id}" \
  --change-type "updated"

# List active subscriptions
teams subscribe list

# Renew a subscription
teams subscribe renew <subscription-id> [--expiration <datetime>]

# Delete a subscription
teams subscribe delete <subscription-id>

# Start built-in webhook listener (receives change notifications)
teams listen --port 8080 [--tls-cert <path>] [--tls-key <path>]

# Stream events to stdout (for piping to agents)
teams listen --port 8080 --output json --stdout
```

### 7.15 MCP Server Mode

```bash
# Start as an MCP server (stdio transport — for Claude Desktop, Cursor, etc.)
teams mcp serve --transport stdio

# Start as MCP server (SSE transport — for remote agents)
teams mcp serve --transport sse --port 3001 [--auth-token <token>]

# List available MCP tools
teams mcp tools
```

**MCP Tool Definitions (exposed to agents):**

| Tool Name | Description |
|-----------|-------------|
| `teams_send_message` | Send a message to a channel or chat |
| `teams_read_messages` | Read recent messages from a channel or chat |
| `teams_reply_message` | Reply to a specific message |
| `teams_react_message` | React to a message with an emoji |
| `teams_search_messages` | Search for messages by keyword |
| `teams_list_teams` | List teams the user belongs to |
| `teams_list_channels` | List channels in a team |
| `teams_list_chats` | List the user's chats |
| `teams_get_presence` | Get user presence/status |
| `teams_set_presence` | Set user presence/status |
| `teams_create_meeting` | Create an online meeting |
| `teams_upload_file` | Upload a file to a channel |
| `teams_download_file` | Download a file from a channel |
| `teams_list_members` | List members of a team or channel |
| `teams_send_notification` | Send an activity feed notification |
| `teams_create_channel` | Create a new channel in a team |
| `teams_get_user` | Get user profile information |

### 7.16 Configuration

```bash
# Initialize configuration (interactive setup wizard)
teams config init

# Show current configuration
teams config show

# Set a configuration value
teams config set <key> <value>

# Get a configuration value
teams config get <key>

# List all profiles
teams config profiles

# Create a new profile
teams config profile create <name> \
  --client-id <id> --tenant-id <tenant>

# Delete a profile
teams config profile delete <name>

# Set default profile
teams config profile default <name>

# Export config (for sharing/backup — excludes secrets)
teams config export > teams-cli-config.toml

# Import config
teams config import < teams-cli-config.toml
```

---

## 8. Output Formats & Agent Integration

### 8.1 Output Format Strategy

The CLI auto-detects whether output is going to a terminal (human) or a pipe (agent):

| Destination | Default Format | Behavior |
|-------------|---------------|----------|
| Terminal (TTY) | `table` | Human-readable tables with colors, truncation, headers |
| Pipe / redirect | `json` | Machine-parseable JSON, one object per result |
| Explicit `--output json` | `json` | Always JSON regardless of destination |
| Explicit `--output csv` | `csv` | CSV with headers |
| Explicit `--output yaml` | `yaml` | YAML format |
| Explicit `--output jsonl` | `jsonl` | JSON Lines (one JSON object per line) |

### 8.2 JSON Output Contract

All JSON output follows a consistent envelope:

```json
{
  "success": true,
  "data": { ... },
  "metadata": {
    "request_id": "uuid",
    "timestamp": "2026-03-04T17:20:00Z",
    "api_version": "v1.0",
    "next_link": "https://graph.microsoft.com/v1.0/...",
    "total_count": 42
  }
}
```

Error responses:

```json
{
  "success": false,
  "error": {
    "code": "AUTH_TOKEN_EXPIRED",
    "message": "Access token has expired. Run `teams auth login` to refresh.",
    "graph_error_code": "InvalidAuthenticationToken",
    "status": 401,
    "retry_after": null
  }
}
```

### 8.3 Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error |
| 2 | Invalid arguments / usage error |
| 3 | Authentication error |
| 4 | Permission denied (403) |
| 5 | Resource not found (404) |
| 6 | Rate limited (429) — after all retries exhausted |
| 7 | Network error / timeout |
| 8 | Server error (5xx) |
| 10 | Configuration error |

### 8.4 Piping & Composition Examples

```bash
# Get all channels, filter with jq, send a message to each
teams channel list <team-id> --output json | \
  jq -r '.data[].id' | \
  xargs -I{} teams message send --team <team-id> --channel {} --body "Reminder: standup in 5"

# Monitor a channel and pipe messages to an LLM for summarization
teams listen --port 8080 --stdout | \
  jq 'select(.data.body.content != null) | .data.body.content' | \
  while read msg; do echo "$msg" | llm "Summarize this Teams message"; done

# Export chat history to CSV
teams message list --chat <chat-id> --all-pages --output csv > chat_history.csv

# CI/CD: Send build result with Adaptive Card
teams message send --team <team-id> --channel <channel-id> \
  --adaptive-card <(envsubst < ./build-card-template.json)
```

---

## 9. Rate Limiting & Resilience

### 9.1 Microsoft Graph Throttling

Microsoft Graph imposes service-specific throttling limits. The CLI must handle these transparently:

- **Per-app limits:** Vary by service (e.g., Teams messages, subscriptions, presence)
- **Per-tenant limits:** Shared across all apps in a tenant
- **HTTP 429 responses:** Include `Retry-After` header

### 9.2 Retry Strategy

```
Retry Policy:
  - Max retries: 3 (configurable via --retry)
  - Backoff: Exponential with jitter
  - Base delay: 1 second
  - Max delay: 60 seconds
  - Retry on: 429 (Too Many Requests), 503 (Service Unavailable),
              504 (Gateway Timeout), network errors
  - Do NOT retry: 400, 401, 403, 404, 409
  - Honor Retry-After header when present
```

### 9.3 Batch Operations

For commands that operate on multiple resources, use Microsoft Graph's `$batch` endpoint:

```
POST https://graph.microsoft.com/v1.0/$batch
Content-Type: application/json

{
  "requests": [
    { "id": "1", "method": "GET", "url": "/teams/{id}/channels" },
    { "id": "2", "method": "GET", "url": "/teams/{id}/members" }
  ]
}
```

Implement batch support for:
- Multi-user presence queries
- Bulk message operations
- Multi-channel listing

---

## 10. Configuration File Format

Location: `~/.config/teams-cli/config.toml`

```toml
[default]
profile = "work"
output = "json"
api_version = "v1.0"
page_size = 50
timeout = 30
retry = 3
color = true

[profiles.work]
client_id = "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx"
tenant_id = "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx"
auth_flow = "device-code"
# client_secret stored in OS keyring, not in config file

[profiles.ci]
client_id = "yyyyyyyy-yyyy-yyyy-yyyy-yyyyyyyyyyyy"
tenant_id = "yyyyyyyy-yyyy-yyyy-yyyy-yyyyyyyyyyyy"
auth_flow = "client-credentials"
# Secrets from environment variables: TEAMS_CLI_CLIENT_SECRET

[profiles.personal]
client_id = "zzzzzzzz-zzzz-zzzz-zzzz-zzzzzzzzzzzz"
tenant_id = "common"
auth_flow = "auth-code-pkce"

[mcp]
transport = "stdio"
default_tools = ["teams_send_message", "teams_read_messages", "teams_search_messages"]
max_message_length = 4000

[webhook]
port = 8080
tls = false
```

---

## 11. Security Considerations

1. **No secrets in config files.** Client secrets and tokens stored in OS keyring only. Config file contains only non-sensitive identifiers.
2. **Token encryption at rest.** If keyring is unavailable, fallback to AES-256-GCM encrypted file with user-derived key.
3. **Minimal scope requests.** Each command requests only the permissions it needs. The auth layer tracks granted scopes.
4. **Audit logging.** All API calls logged with timestamps and request IDs for troubleshooting (opt-in via `--verbose`).
5. **No credential leaking in output.** Access tokens, secrets, and auth headers are never included in JSON output or error messages.
6. **MCP auth.** SSE transport MCP server requires bearer token authentication. Stdio transport inherits parent process auth.

---

## 12. Error Handling Philosophy

### For Humans (TTY output)
- Clear, actionable error messages with suggested fixes
- Example: `Error: Permission denied. The scope 'ChannelMessage.Send' is required. Run 'teams auth login' to grant additional permissions.`

### For Agents (piped output)
- Structured JSON errors with machine-parseable error codes
- Deterministic exit codes for branching logic
- No interactive prompts — fail fast with clear error

### Common Error Scenarios & Handling

| Scenario | Handling |
|----------|----------|
| Token expired | Auto-refresh using refresh token. If refresh fails, exit code 3 with re-auth instruction |
| Rate limited (429) | Retry with exponential backoff, respecting Retry-After header |
| Insufficient permissions | Exit code 4 with specific scope needed |
| Resource not found | Exit code 5 with resource identifier in error |
| Network timeout | Retry up to max, then exit code 7 |
| Invalid input | Exit code 2 with usage hint |
| Graph API deprecation warning | Log warning to stderr, continue execution |

---

## 13. Testing Strategy

### 13.1 Unit Tests
- Auth flow token handling and refresh logic
- JSON output envelope formatting
- Rate limiter and retry logic
- Configuration parsing and validation
- Command argument parsing

### 13.2 Integration Tests
- Against Microsoft Graph API using a dedicated test tenant (Microsoft 365 Developer Program)
- Auth flow end-to-end (client credentials flow for CI)
- CRUD operations for teams, channels, messages
- File upload/download
- Search operations
- Subscription lifecycle (create, renew, delete)

### 13.3 End-to-End Tests
- Full CLI workflows via subprocess execution
- Pipe chain verification (`teams channel list | jq | xargs teams message send`)
- MCP server mode interaction tests
- Multi-profile switching

### 13.4 Mocking
- Use `wiremock` crate for HTTP-level Graph API mocking in unit tests
- Record/replay mode for integration test development

---

## 14. Distribution & Installation

### 14.1 Installation Methods

```bash
# Homebrew (macOS/Linux)
brew install teams-cli

# Cargo (from source)
cargo install teams-cli

# Binary download (GitHub Releases)
curl -sSL https://github.com/<org>/teams-cli/releases/latest/download/teams-cli-$(uname -s)-$(uname -m) -o /usr/local/bin/teams
chmod +x /usr/local/bin/teams

# Docker (for CI/CD)
docker run --rm -v ~/.config/teams-cli:/root/.config/teams-cli teams-cli:latest message send ...

# Nix
nix-env -iA nixpkgs.teams-cli
```

### 14.2 CI/CD Build Matrix

| OS | Architecture | Target Triple |
|----|-------------|---------------|
| macOS | ARM64 | aarch64-apple-darwin |
| macOS | x86_64 | x86_64-apple-darwin |
| Linux | x86_64 | x86_64-unknown-linux-musl |
| Linux | ARM64 | aarch64-unknown-linux-musl |
| Windows | x86_64 | x86_64-pc-windows-msvc |

Use `cross` for cross-compilation and musl for fully static Linux binaries.

### 14.3 Shell Completions

Generate shell completions during build via `clap_complete`:

```bash
# Install completions
teams completions bash > /etc/bash_completion.d/teams
teams completions zsh > ~/.zfunc/_teams
teams completions fish > ~/.config/fish/completions/teams.fish
teams completions powershell > teams.ps1
```

---

## 15. Milestones & Implementation Phases

### Phase 1: Foundation (Weeks 1-3)
- [ ] Project scaffolding (Cargo workspace, CI/CD with GitHub Actions)
- [ ] Auth layer: client credentials + device code flows
- [ ] Token caching with keyring + encrypted file fallback
- [ ] Config management (multi-profile TOML)
- [ ] HTTP client with rate limiting and retry (reqwest + tower)
- [ ] Output formatter (JSON, table, CSV)
- [ ] Global flags and CLI framework (clap v4)
- [ ] `teams auth *` commands
- [ ] `teams user me` / `teams user get`

### Phase 2: Core Messaging (Weeks 4-6)
- [ ] `teams team *` commands (list, get, create, update, delete, members)
- [ ] `teams channel *` commands (CRUD, members)
- [ ] `teams message send` (plain text, HTML, adaptive cards, stdin)
- [ ] `teams message list` / `teams message get` with pagination
- [ ] `teams message reply`
- [ ] `teams message react` / `teams message unreact`
- [ ] `teams chat *` commands (list, get, create, members)
- [ ] Stdin piping support for message body

### Phase 3: Extended Features (Weeks 7-9)
- [ ] `teams presence *` commands
- [ ] `teams meeting *` commands
- [ ] `teams file *` commands (upload, download, list, share)
- [ ] `teams search *` commands
- [ ] `teams tag *` commands
- [ ] `teams notify *` commands
- [ ] `teams app *` / `teams tab *` commands
- [ ] Batch operation support

### Phase 4: Real-Time & MCP (Weeks 10-12)
- [ ] Webhook listener (`teams listen`)
- [ ] Change notification subscriptions (`teams subscribe *`)
- [ ] MCP server mode — stdio transport
- [ ] MCP server mode — SSE transport
- [ ] MCP tool definitions for all core operations
- [ ] Event streaming to stdout

### Phase 5: Polish & Distribution (Weeks 13-14)
- [ ] Shell completions (bash, zsh, fish, PowerShell)
- [ ] Comprehensive error messages with fix suggestions
- [ ] Man pages / `teams help` improvements
- [ ] Homebrew formula
- [ ] Docker image
- [ ] GitHub Actions release automation
- [ ] README, CONTRIBUTING, CHANGELOG
- [ ] Integration test suite against test tenant

---

## 16. Open Questions & Future Considerations

| # | Question | Status |
|---|----------|--------|
| 1 | Should we use `graph-rs-sdk` as the Graph client or build a thin wrapper over `reqwest`? The SDK is 44K SLoC and may add unnecessary complexity. Recommend: thin wrapper for v1, evaluate SDK for v2. | Open |
| 2 | How to handle delegated vs application permission differences in the same CLI? Some operations (e.g., set own presence) require delegated; others (e.g., read all messages) require application. | Recommend: auto-detect based on auth flow and warn if scope insufficient |
| 3 | Should MCP server mode be a separate binary or a subcommand? | Recommend: subcommand (`teams mcp serve`) to keep single binary |
| 4 | Support for Teams webhooks (incoming/outgoing) as a simpler alternative to Graph API for message sending? Note: O365 connectors deprecated, Workflows replacing them. | Recommend: Graph API only for consistency; document migration from webhooks |
| 5 | Interactive message composer (e.g., `teams message compose` with $EDITOR)? | Future: nice-to-have for human users |
| 6 | Should we support GCC/GCC High/DoD environments? Different Graph endpoints and auth. | Future: v2.0+ |
| 7 | Offline cache for frequently accessed data (team list, channel list, user directory)? | Future: useful for agent performance |

---

## 17. Success Metrics

| Metric | Target |
|--------|--------|
| Command coverage | 100% of Graph Teams API v1.0 endpoints exposed as CLI commands |
| Auth flow success rate | >99% for client credentials, >95% for device code |
| Response time | <2s for single-resource operations (excluding auth) |
| Binary size | <20MB (stripped, release build) |
| Test coverage | >80% unit test coverage, >60% integration test coverage |
| Cross-platform | Builds and runs on macOS, Linux, Windows |
| GitHub stars (6 months) | 500+ |
| MCP integration | Works with Claude Desktop, Cursor, and custom agents |

---

## 18. References & Resources

- **Microsoft Graph Teams API Overview:** https://learn.microsoft.com/en-us/graph/api/resources/teams-api-overview
- **Microsoft Graph Auth Overview:** https://learn.microsoft.com/en-us/graph/auth/
- **Graph API Throttling Limits:** https://learn.microsoft.com/en-us/graph/throttling-limits
- **graph-rs-sdk (Rust Graph SDK):** https://crates.io/crates/graph-rs-sdk
- **graph-oauth (Rust OAuth):** https://crates.io/crates/graph-oauth
- **Microsoft Graph Change Notifications:** https://learn.microsoft.com/en-us/graph/api/resources/change-notifications-api-overview
- **Microsoft Search API for Teams Messages:** https://learn.microsoft.com/en-us/graph/search-concept-chat-messages
- **Presence API:** https://learn.microsoft.com/en-us/graph/cloud-communications-manage-presence-state
- **Activity Feed Notifications:** https://learn.microsoft.com/en-us/graph/teams-send-activityfeednotifications
- **MCP Specification:** https://modelcontextprotocol.io/specification
- **Teams MCP Server Registration:** https://learn.microsoft.com/en-us/microsoftteams/platform/m365-apps/agent-connectors
- **fossteams/teams-cli (reference, archived):** https://github.com/fossteams/teams-cli
- **SlackCLI (design reference):** https://shaharia.com/blog/slackcli-command-line-tool-slack-automation/
- **Teams Developer Survival Guide:** https://www.voitanos.io/blog/microsoft-teams-navigate-developer-docs-survival-guide/

---

*This PRD is intended to be given to a coding agent (e.g., Claude Code, Cursor, Aider) to implement. Each command section includes the exact CLI syntax, expected behavior, and API endpoints to call. The agent should implement commands incrementally following the milestone phases.*
