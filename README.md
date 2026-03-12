# teams — Microsoft Teams CLI for Developers, Bots, and AI Agents (Rust)

A fast, single-binary CLI for [Microsoft Teams](https://teams.microsoft.com), built in Rust.

Manage teams, channels, messages, meetings, presence, files, and more from your terminal — with structured JSON output, real-time webhook support, and full agentic workflow support.

[![CI](https://github.com/osodevops/ms-teams-cli/actions/workflows/ci.yml/badge.svg)](https://github.com/osodevops/ms-teams-cli/actions/workflows/ci.yml)
[![Release](https://github.com/osodevops/ms-teams-cli/actions/workflows/release.yml/badge.svg)](https://github.com/osodevops/ms-teams-cli/releases)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

## Install

```bash
# Homebrew (macOS/Linux)
brew install osodevops/tap/teams

# Pre-built binaries — download from GitHub Releases
# https://github.com/osodevops/ms-teams-cli/releases

# From source
cargo install --git https://github.com/osodevops/ms-teams-cli
```

## Setup

Authenticate with your Azure AD application:

```bash
# Interactive login (opens browser, Authorization Code + PKCE)
teams auth login --client-id <client-id> --tenant-id <tenant-id>

# Device code flow (headless/SSH)
teams auth login --device-code --client-id <client-id> --tenant-id <tenant-id>

# Client credentials (non-interactive, for CI/CD and agents)
teams auth login --client-credentials \
  --client-id <client-id> --client-secret <secret> --tenant-id <tenant-id>
```

Or use environment variables:

```bash
export TEAMS_CLI_CLIENT_ID=your-client-id
export TEAMS_CLI_TENANT_ID=your-tenant-id
export TEAMS_CLI_CLIENT_SECRET=your-secret  # client credentials only
teams auth login --client-credentials
```

Tokens are stored in the OS keyring (macOS Keychain, Windows Credential Manager, Linux Secret Service). Sessions persist across CLI invocations.

## Quick Start

```bash
# List your teams
teams team list

# List channels in a team
teams channel list <team-id>

# Send a message to a channel
teams message send --team <team-id> --channel <channel-id> --body "Hello from CLI"

# Search messages across Teams
teams search messages --query "deployment failed"

# Check your presence
teams presence get

# List upcoming meetings
teams meeting list
```

## Commands

### Authentication

```bash
teams auth login             # Interactive login (browser)
teams auth login --device-code  # Device code flow
teams auth login --client-credentials  # Client credentials
teams auth status            # Check if session is valid (exit code 0/1)
teams auth list              # List authenticated profiles
teams auth switch <profile>  # Switch active profile
teams auth logout            # Clear stored credentials
teams auth token             # Export access token
```

### Teams

```bash
teams team list              # List joined teams
teams team get <team-id>     # Get team details
teams team create --name "Engineering" [--description "..."]
teams team update <team-id> --name "New Name"
teams team delete <team-id>
teams team clone <team-id> --name "Cloned Team" [--parts apps,tabs,settings,channels,members]
teams team archive <team-id>
teams team unarchive <team-id>
teams team members list <team-id>
teams team members add <team-id> --user-id <user-id> [--role owner]
teams team members remove <team-id> <member-id>
```

### Channels

```bash
teams channel list <team-id>
teams channel get <team-id> <channel-id>
teams channel create <team-id> --name "releases" [--description "..."] [--type private]
teams channel update <team-id> <channel-id> [--name "..."] [--description "..."]
teams channel delete <team-id> <channel-id>
teams channel members list <team-id> <channel-id>
teams channel members add <team-id> <channel-id> --user-id <user-id>
teams channel members remove <team-id> <channel-id> <member-id>
```

### Messages

```bash
teams message send --team <team-id> --channel <channel-id> --body "Hello"
teams message send --chat <chat-id> --body "Hello"
teams message send --team <team-id> --channel <channel-id> --body "<h1>Rich</h1>" --content-type html
echo "Build passed" | teams message send --team <team-id> --channel <channel-id> --stdin
teams message list --team <team-id> --channel <channel-id>
teams message list --chat <chat-id>
teams message get --team <team-id> --channel <channel-id> --message <msg-id>
teams message reply --team <team-id> --channel <channel-id> --message <msg-id> --body "Thanks!"
teams message delete --team <team-id> --channel <channel-id> --message <msg-id>
teams message react --team <team-id> --channel <channel-id> --message <msg-id> --reaction like
teams message unreact --team <team-id> --channel <channel-id> --message <msg-id> --reaction like
teams message pin --team <team-id> --channel <channel-id> --message <msg-id>
teams message unpin --team <team-id> --channel <channel-id> --pinned-message-id <id>
```

### Chats

```bash
teams chat list
teams chat get <chat-id>
teams chat create --type oneOnOne --members <user-id-1>,<user-id-2>
teams chat hide <chat-id>
teams chat unhide <chat-id>
teams chat members list <chat-id>
teams chat members add <chat-id> --user-id <user-id>
teams chat members remove <chat-id> <member-id>
```

### Presence

```bash
teams presence get                      # Your own presence
teams presence get --user-id <user-id>  # Another user's presence
teams presence get-batch --user-ids <id1>,<id2>
teams presence set --availability Available --activity Available
teams presence clear
teams presence status --message "In deep focus" [--expiry <datetime>]
```

### Search

```bash
teams search messages --query "quarterly review"
teams search users --query "John"
teams search teams --query "engineering"
```

### Tags

```bash
teams tag list <team-id>
teams tag get <team-id> <tag-id>
teams tag create <team-id> --name "Frontend" --members <user-id-1>,<user-id-2>
teams tag delete <team-id> <tag-id>
teams tag add-member <team-id> <tag-id> --user-id <user-id>
teams tag remove-member <team-id> <tag-id> <member-id>
```

### Meetings

```bash
teams meeting list
teams meeting get <meeting-id>
teams meeting create --subject "Standup" --start "2026-03-15T10:00:00Z" --end "2026-03-15T10:30:00Z"
teams meeting delete <meeting-id>
teams meeting join-url <meeting-id>
teams meeting attendance <meeting-id>
```

### Notifications

```bash
teams notify send --user-id <user-id> --topic "New Assignment" \
  --activity-type taskCreated --preview "You have a new task"
teams notify send-to-team --team-id <team-id> --topic "Deploy" \
  --activity-type deploymentComplete --preview "v2.3.1 deployed"
teams notify send-to-chat --chat-id <chat-id> --topic "Update" \
  --activity-type statusUpdate --preview "Status changed"
```

### Apps

```bash
teams app list <team-id>
teams app install <team-id> --app-id <catalog-app-id>
teams app uninstall <team-id> <installation-id>
```

### Tabs

```bash
teams tab list <team-id> <channel-id>
teams tab create <team-id> <channel-id> --app-id <app-id> --name "Wiki" --content-url <url>
teams tab delete <team-id> <channel-id> <tab-id>
```

### Files

```bash
teams file list --team <team-id> --channel <channel-id>
teams file get --team <team-id> --channel <channel-id> --file-id <id>
teams file upload --team <team-id> --channel <channel-id> --file ./report.pdf
teams file download --team <team-id> --channel <channel-id> --file-id <id> --output ./local.pdf
teams file delete --team <team-id> --channel <channel-id> --file-id <id>
teams file share --team <team-id> --channel <channel-id> --file-id <id> --scope organization
```

### Subscriptions

```bash
teams subscribe create \
  --resource "/teams/{team-id}/channels/{channel-id}/messages" \
  --change-type created,updated \
  --webhook-url https://your-endpoint/webhook
teams subscribe list
teams subscribe renew <subscription-id> [--expiration <datetime>]
teams subscribe delete <subscription-id>
```

### Webhook Listener

```bash
# Start a webhook listener to receive change notifications
teams listen --port 8080
```

The listener outputs one JSON object per line (NDJSON) to stdout, suitable for piping:

```bash
teams listen --port 8080 | jq '.changeType, .resource'
```

**Note:** Microsoft Graph requires HTTPS for webhook notification URLs. Use a reverse proxy such as [ngrok](https://ngrok.com) in front of the listener:

```bash
ngrok http 8080  # exposes https://xxxx.ngrok.io -> localhost:8080
teams subscribe create --resource "/teams/all/messages" \
  --change-type created --webhook-url https://xxxx.ngrok.io/webhook
teams listen --port 8080
```

### User Lookup

```bash
teams user me
teams user get <user-id-or-upn>
teams user list
```

### Configuration

```bash
teams config path            # Show config file path
teams config show            # Show current config
teams config set <key> <value>
teams config get <key>
teams config init            # Create default config file
```

### Shell Completions

```bash
teams completions bash >> ~/.bashrc
teams completions zsh >> ~/.zshrc
teams completions fish > ~/.config/fish/completions/teams.fish
teams completions powershell > teams.ps1
```

## Output Formats

teams auto-detects the output format:

- **TTY** (interactive terminal): Human-readable tables
- **Pipe** (non-interactive): Machine-readable JSON envelope

Override with `--output`:

```bash
teams team list --output json      # Force JSON
teams team list --output human     # Force human-readable tables
teams team list --output plain     # Plain text
```

### JSON Envelope

All JSON output follows a standard envelope:

```json
{
  "success": true,
  "data": { ... },
  "metadata": {
    "request_id": "uuid",
    "timestamp": "2026-03-15T00:00:00Z",
    "duration_ms": 123
  }
}
```

Error responses:

```json
{
  "success": false,
  "error": {
    "code": "AUTH_TOKEN_EXPIRED",
    "message": "Token expired"
  }
}
```

## Configuration

Config file location:
- Linux: `~/.config/teams-cli/config.toml`
- macOS: `~/Library/Application Support/teams-cli/config.toml`

```toml
[default]
profile = "work"

[output]
format = "auto"    # auto, json, human, plain
color = true
page_size = 50

[network]
timeout = 30
max_retries = 3
retry_backoff_base = 2

[profiles.work]
client_id = "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx"
tenant_id = "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx"
auth_flow = "device-code"

[profiles.ci]
client_id = "yyyyyyyy-yyyy-yyyy-yyyy-yyyyyyyyyyyy"
tenant_id = "yyyyyyyy-yyyy-yyyy-yyyy-yyyyyyyyyyyy"
auth_flow = "client-credentials"
```

### Profiles

Use named profiles for multiple accounts:

```bash
teams --profile work auth login --client-id ... --tenant-id ...
teams --profile ci auth login --client-credentials --client-id ... --tenant-id ...

teams --profile work team list
teams --profile ci message send --team <id> --channel <id> --body "Deploy complete"
```

## Environment Variables

| Variable | Description |
|----------|-------------|
| `TEAMS_CLI_CLIENT_ID` | Azure AD application (client) ID |
| `TEAMS_CLI_CLIENT_SECRET` | Azure AD client secret |
| `TEAMS_CLI_TENANT_ID` | Azure AD tenant ID |
| `TEAMS_CLI_ACCESS_TOKEN` | Pre-obtained access token (skips login) |
| `RUST_LOG` | Tracing filter (e.g., `debug`, `teams=trace`) |

## Verbosity

```bash
teams team list              # Warnings only
teams -v team list           # Info level
teams -vv team list          # Debug level
teams -vvv team list         # Trace level
```

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error |
| 2 | Invalid arguments / usage error |
| 3 | Authentication error |
| 4 | Permission denied (403) |
| 5 | Resource not found (404) |
| 6 | Rate limited (429) |
| 7 | Network error / timeout |
| 8 | Server error (5xx) |
| 10 | Configuration error |

## Agentic Usage

teams is designed for use by AI agents and automation. Key features:

- **Structured JSON output** when piped — parse with `jq` or any JSON library
- **Deterministic exit codes** for error handling
- **No interactive prompts** in client-credentials flow
- **Environment variable auth** — no interactive login needed
- **Real-time events** via webhook listener with NDJSON output
- **Idempotent operations** — safe to retry

Example in a script:

```bash
#!/bin/bash
export TEAMS_CLI_CLIENT_ID=your-client-id
export TEAMS_CLI_CLIENT_SECRET=your-secret
export TEAMS_CLI_TENANT_ID=your-tenant

# Login
teams auth login --client-credentials

# Get all channels, send a message to each
teams channel list <team-id> --output json | \
  jq -r '.data[].id' | \
  xargs -I{} teams message send --team <team-id> --channel {} --body "Reminder: standup in 5"

# Monitor a channel via webhook and pipe to processing
teams listen --port 8080 | \
  jq 'select(.changeType == "created") | .resource'
```

## Development

```bash
cargo build                          # Debug build
cargo build --release                # Release build
cargo test --all-targets             # All tests (unit + integration)
cargo test --lib --bins              # Unit tests only
cargo fmt -- --check                 # Check formatting
cargo clippy --all-targets -- -D warnings  # Lint
```

## Contributing

We welcome issues and PRs. Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## Security

See [SECURITY.md](SECURITY.md) for our security policy and how to report vulnerabilities.

## License

MIT — see [LICENSE](LICENSE).
