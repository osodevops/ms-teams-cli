# teams — Microsoft Teams CLI for AI Agents and Developers (Rust)

A fast, single-binary CLI that gives AI agents and automation full access to [Microsoft Teams](https://teams.microsoft.com) via the Microsoft Graph API.

Every command returns structured JSON with deterministic exit codes — designed from the ground up for autonomous agents (Claude, GPT, custom LLM agents), CI/CD pipelines, and developer scripts. Not a chatbot framework. A tool that agents wield.

[![CI](https://github.com/osodevops/ms-teams-cli/actions/workflows/ci.yml/badge.svg)](https://github.com/osodevops/ms-teams-cli/actions/workflows/ci.yml)
[![Release](https://github.com/osodevops/ms-teams-cli/actions/workflows/release.yml/badge.svg)](https://github.com/osodevops/ms-teams-cli/releases)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

## Why This Exists

AI agents need to operate in Microsoft Teams — reading messages, posting updates, managing channels, reacting to events — but there is no comprehensive CLI for Teams. Existing MCP servers cover only a fraction of the Graph API. Bot Framework SDKs assume a conversational UI inside Teams, not an external agent calling in.

**teams-cli** fills this gap: a complete, headless, machine-readable interface to Teams that any agent can call as a subprocess.

## Agent Integration Contract

Every command, when piped or called programmatically, returns a **JSON envelope**:

```json
{
  "success": true,
  "data": { "id": "...", "displayName": "Engineering", "..." : "..." },
  "metadata": {
    "request_id": "550e8400-e29b-41d4-a716-446655440000",
    "timestamp": "2026-03-15T00:00:00Z",
    "duration_ms": 123
  }
}
```

On failure:

```json
{
  "success": false,
  "error": {
    "code": "AUTH_TOKEN_EXPIRED",
    "message": "Access token has expired. Run `teams auth login` to re-authenticate."
  }
}
```

**Exit codes** map directly to error categories — agents can branch on `$?` without parsing:

| Code | Meaning | Agent Action |
|------|---------|--------------|
| 0 | Success | Parse `.data` from stdout |
| 1 | General error | Log and investigate |
| 2 | Invalid arguments | Fix command syntax |
| 3 | Authentication error | Re-authenticate |
| 4 | Permission denied (403) | Escalate or skip |
| 5 | Resource not found (404) | Handle missing resource |
| 6 | Rate limited (429) | Back off and retry |
| 7 | Network error / timeout | Retry with backoff |
| 8 | Server error (5xx) | Retry with backoff |
| 10 | Configuration error | Check config/env vars |

**Output auto-detection**: When stdout is a TTY, output defaults to human-readable tables. When piped (which is how agents call it), output defaults to JSON. Override with `--output json|human|plain`.

## Install

```bash
# Homebrew (macOS/Linux)
brew install osodevops/tap/teams

# Pre-built binaries — download from GitHub Releases
# https://github.com/osodevops/ms-teams-cli/releases

# From source
cargo install --git https://github.com/osodevops/ms-teams-cli
```

## Agent Authentication

For AI agents and automation, use **client credentials** (fully headless, no browser):

```bash
# Option 1: Environment variables (recommended for agents)
export TEAMS_CLI_CLIENT_ID=your-client-id
export TEAMS_CLI_CLIENT_SECRET=your-secret
export TEAMS_CLI_TENANT_ID=your-tenant-id
teams auth login --client-credentials

# Option 2: Pass a pre-obtained token directly
export TEAMS_CLI_ACCESS_TOKEN=eyJ0eXAi...
teams team list  # no login step needed

# Option 3: Explicit flags
teams auth login --client-credentials \
  --client-id <client-id> --client-secret <secret> --tenant-id <tenant-id>
```

Tokens are cached in the OS keyring — subsequent commands reuse the session without re-authentication.

**Other auth flows** (for interactive/developer use):

```bash
# Browser-based login (Authorization Code + PKCE)
teams auth login --client-id <client-id> --tenant-id <tenant-id>

# Device code flow (headless/SSH, still requires a human to approve once)
teams auth login --device-code --client-id <client-id> --tenant-id <tenant-id>
```

**Credential resolution order**: CLI flags > environment variables > config file profiles.

## Agent Workflow Patterns

### Pattern 1: Read-Act-Respond

```bash
# Agent reads recent messages, decides how to respond
MESSAGES=$(teams message list --team $TEAM --channel $CHANNEL --output json)
# Parse with jq, pass to LLM, then act
echo "$MESSAGES" | jq -r '.data[].body.content' | my-agent-process
teams message send --team $TEAM --channel $CHANNEL --body "$RESPONSE"
```

### Pattern 2: Fan-Out Notifications

```bash
# Send a message to every channel in a team
teams channel list $TEAM_ID --output json | \
  jq -r '.data[].id' | \
  xargs -I{} teams message send --team $TEAM_ID --channel {} --body "Deploy v2.3.1 complete"
```

### Pattern 3: Real-Time Event Loop

```bash
# Listen for new messages via webhook and pipe to agent processing
teams listen --port 8080 | \
  jq --unbuffered 'select(.changeType == "created")' | \
  while IFS= read -r event; do
    my-agent-handler "$event"
  done
```

### Pattern 4: Stdin Composition

```bash
# Pipe any command output as a Teams message
kubectl get pods --namespace prod | \
  teams message send --team $TEAM --channel $CHANNEL --stdin

# Pipe a file as message body
cat report.md | teams message send --team $TEAM --channel $CHANNEL --stdin --content-type html
```

### Pattern 5: Conditional Error Handling

```bash
teams message send --team $TEAM --channel $CHANNEL --body "Hello"
case $? in
  0) echo "Sent" ;;
  3) teams auth login --client-credentials && retry ;;
  6) sleep 30 && retry ;;  # Rate limited
  5) echo "Channel not found" ;;
  *) echo "Unexpected error" ;;
esac
```

## Capabilities

### Authentication

```bash
teams auth login             # Interactive login (browser)
teams auth login --device-code  # Device code flow
teams auth login --client-credentials  # Client credentials (agents)
teams auth status            # Check if session is valid (exit code 0/1)
teams auth list              # List authenticated profiles
teams auth switch <profile>  # Switch active profile
teams auth logout            # Clear stored credentials
teams auth token             # Export access token to stdout
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

### Apps & Tabs

```bash
teams app list <team-id>
teams app install <team-id> --app-id <catalog-app-id>
teams app uninstall <team-id> <installation-id>
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

### Subscriptions & Webhooks

```bash
# Create a subscription for real-time change notifications
teams subscribe create \
  --resource "/teams/{team-id}/channels/{channel-id}/messages" \
  --change-type created,updated \
  --webhook-url https://your-endpoint/webhook
teams subscribe list
teams subscribe renew <subscription-id> [--expiration <datetime>]
teams subscribe delete <subscription-id>

# Start a webhook listener (outputs NDJSON to stdout)
teams listen --port 8080
teams listen --port 8080 | jq '.changeType, .resource'
```

**Note:** Microsoft Graph requires HTTPS for webhook URLs. Use [ngrok](https://ngrok.com) or similar:

```bash
ngrok http 8080
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

## Resilience & Rate Limiting

The Graph API client handles transient failures automatically:

- **Retry with exponential backoff** on 429 (rate limited) and 5xx errors
- **Respects `Retry-After` header** from Microsoft Graph
- **Configurable**: `--retry <max>` flag, or `network.max_retries` in config
- **Pagination**: `--all-pages` fetches all results; `--page-size` controls batch size
- **Idempotent operations**: safe to retry on failure

Agents should treat exit code 6 (rate limited) as "retry later" — the CLI has already retried internally.

## Multi-Profile Support

Manage multiple tenants or service principals:

```bash
teams --profile prod auth login --client-credentials --client-id ... --tenant-id ...
teams --profile staging auth login --client-credentials --client-id ... --tenant-id ...

teams --profile prod team list
teams --profile staging message send --team <id> --channel <id> --body "Deployed"
```

Config file (`~/.config/teams-cli/config.toml` on Linux, `~/Library/Application Support/teams-cli/config.toml` on macOS):

```toml
[default]
profile = "prod"

[output]
format = "auto"    # auto, json, human, plain
color = true
page_size = 50

[network]
timeout = 30
max_retries = 3
retry_backoff_base = 2

[profiles.prod]
client_id = "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx"
tenant_id = "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx"
auth_flow = "client-credentials"

[profiles.staging]
client_id = "yyyyyyyy-yyyy-yyyy-yyyy-yyyyyyyyyyyy"
tenant_id = "yyyyyyyy-yyyy-yyyy-yyyy-yyyyyyyyyyyy"
auth_flow = "client-credentials"
```

## Environment Variables

| Variable | Description |
|----------|-------------|
| `TEAMS_CLI_CLIENT_ID` | Azure AD application (client) ID |
| `TEAMS_CLI_CLIENT_SECRET` | Azure AD client secret |
| `TEAMS_CLI_TENANT_ID` | Azure AD tenant ID |
| `TEAMS_CLI_ACCESS_TOKEN` | Pre-obtained access token (skips login entirely) |
| `RUST_LOG` | Tracing filter (e.g., `debug`, `teams=trace`) |

## Verbosity

```bash
teams team list              # Warnings only (stderr)
teams -v team list           # Info level
teams -vv team list          # Debug level
teams -vvv team list         # Trace level
```

Trace output goes to stderr, never polluting stdout JSON.

## Shell Completions

```bash
teams completions bash >> ~/.bashrc
teams completions zsh >> ~/.zshrc
teams completions fish > ~/.config/fish/completions/teams.fish
teams completions powershell > teams.ps1
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
