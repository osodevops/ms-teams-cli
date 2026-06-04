# teams — Microsoft Teams CLI for AI Agents and Developers (Rust)

A fast, single-binary CLI that gives AI agents and automation full access to [Microsoft Teams](https://teams.microsoft.com) via the Microsoft Graph API.

Every command returns structured JSON with deterministic exit codes — designed from the ground up for autonomous agents (Claude, GPT, custom LLM agents), CI/CD pipelines, and developer scripts. Not a chatbot framework. A tool that agents wield.

[![CI](https://github.com/osodevops/ms-teams-cli/actions/workflows/ci.yml/badge.svg)](https://github.com/osodevops/ms-teams-cli/actions/workflows/ci.yml)
[![Release](https://github.com/osodevops/ms-teams-cli/actions/workflows/release.yml/badge.svg)](https://github.com/osodevops/ms-teams-cli/releases)
[![Latest Release](https://img.shields.io/github/v/release/osodevops/ms-teams-cli)](https://github.com/osodevops/ms-teams-cli/releases)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

Website: [msteamscli.com](http://msteamscli.com/)

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
brew install osodevops/tap/teams-cli

# Pre-built binaries — download from GitHub Releases
# https://github.com/osodevops/ms-teams-cli/releases

# From source
cargo install --git https://github.com/osodevops/ms-teams-cli
```

## Documentation

- [Project website](http://msteamscli.com/)
- [Docs index](docs/README.md)
- [Quickstarts](docs/quickstarts/README.md)
- [Authentication guide](docs/auth.md)
- [Command reference](docs/command-reference.md)
- [Examples](docs/examples.md)
- [Use cases](docs/use-cases.md)
- [FAQ](docs/faq.md)
- [Troubleshooting](docs/troubleshooting.md)
- [Release readiness](docs/release-readiness.md)
- Man pages: `docs/man/teams.1`, `docs/man/teams-config.5`, `docs/man/teams-auth.7`, `docs/man/teams-agent-contract.7`, `docs/man/teams-examples.7`

## Quickstart

```bash
# 1. Sign in with the built-in OSO public client app
teams auth login --device-code

# 2. Check auth, tenant, token type, and consent URL
teams auth doctor --output json

# 3. Read Teams context
teams user me --output json
teams chat list --page-size 10 --output json
teams team list --output json

# 4. Send only to a known safe test chat or channel
teams message send --chat <chat-id> --body "teams-cli smoke test" --output json
```

Use a dedicated test target for first writes. Do not test against client or production chats.

## Commercial Microsoft Setup

The CLI has OSO's multi-tenant delegated public client app baked in:

```text
Client ID: fba1b5d0-fdd0-4fe2-9729-9ccdc38f9595
Default authority: organizations
Loopback redirect URI: http://localhost:8400/callback
```

Customer admin consent:

```bash
teams auth consent-url --tenant-id <customer-tenant-id-or-domain> --output json
```

For enterprises that require their own app registration, configure BYO mode:

```toml
[profiles.customer]
auth_app = "byo"
client_id = "11111111-1111-1111-1111-111111111111"
tenant_id = "22222222-2222-2222-2222-222222222222"
auth_flow = "device-code"
```

Then sign in:

```bash
teams --profile customer auth login --device-code
teams --profile customer auth doctor --output json
```

Official Microsoft trust checklist before broad external rollout:

- Complete Microsoft Entra publisher verification for the OSO app.
- Set clear Entra branding: display name, logo, publisher domain, homepage, privacy policy, and terms URLs.
- Document every delegated Graph scope requested.
- Use tenant-specific admin consent URLs for customer onboarding.
- Teams Store submission is not required for this CLI-only Graph app. It becomes relevant only if OSO ships a Teams app/bot package.

## Agent Authentication

For Teams message posting and most user-facing actions, use **delegated auth**.
Messages sent through Microsoft Graph appear as the signed-in user, and Microsoft
Graph does not generally support app-only tokens for normal live Teams
chat/channel message posting.

Delegated login defaults to the OSO multi-tenant public client app. Customers
can still bring their own Entra app by passing `--client-id` and `--tenant-id`
or configuring a profile; see
[`docs/auth-implementation-plan.md`](docs/auth-implementation-plan.md).

This CLI calls Microsoft Graph. Every token used by Graph commands must be a
Microsoft Graph access token. Tokens captured from the Microsoft Teams web or
desktop client, including `fossteams/teams-token` files such as
`~/.config/fossteams/token-teams.jwt`, are issued for Teams-specific audiences
and will fail against Graph with `InvalidAuthenticationToken: Invalid audience`.

```bash
# Browser-based login with OSO's public client app
teams auth login

# Device code flow with OSO's public client app
teams auth login --device-code

# Browser-based login with a customer-owned app
teams auth login --client-id <client-id> --tenant-id <tenant-id>

# Device code flow with a customer-owned app
teams auth login --device-code --client-id <client-id> --tenant-id <tenant-id>
```

Use **client credentials** only for commands backed by Graph application
permissions, such as supported read/admin automation. Do not use this as the
primary model for sending normal Teams messages:

```bash
# Environment variables for app-only Graph operations
export TEAMS_CLI_CLIENT_ID=<client-id>
export TEAMS_CLI_CLIENT_SECRET=<client-secret>
export TEAMS_CLI_TENANT_ID=<tenant-id>
teams auth login --client-credentials

# Pass a pre-obtained Microsoft Graph token directly
export TEAMS_CLI_ACCESS_TOKEN=<access-token>
teams team list  # no login step needed

# Explicit flags
teams auth login --client-credentials \
  --client-id <client-id> --client-secret <client-secret> --tenant-id <tenant-id>
```

Tokens are cached in the OS keyring — subsequent commands reuse the session without re-authentication.

**Credential resolution order**: CLI flags > environment variables > config file profiles.

### Why not import Teams client tokens?

Tools such as `fossteams/teams-token` are attractive because they avoid Entra
app registration and consent setup, especially in tenants where users cannot
approve third-party apps themselves. That is a real usability signal: `teams
auth login` should be easy to diagnose, should support browser and device-code
flows clearly, and should not make users reverse-engineer token audiences.

The supported fix is better Graph-native auth, not reusing Teams client tokens.
A Teams, Skype, ChatSvcAgg, or ID token cannot be converted into a Graph token
by this CLI. Use delegated login, device-code login, client credentials for
supported app-only Graph operations, or `TEAMS_CLI_ACCESS_TOKEN` only when the
token was explicitly acquired for Microsoft Graph.

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
  3) teams auth login --device-code && retry ;;
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
teams auth login --client-credentials  # App-only Graph operations where supported
teams auth status            # Check if session is valid (exit code 0/1)
teams auth consent-url       # Print admin consent URL for the active auth app
teams auth doctor            # Diagnose config and token state
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
teams tag add-member <team-id> <tag-id> --user <user-id>
teams tag remove-member <team-id> <tag-id> --user <user-id>
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
teams notify send --user <user-id> --topic "New Assignment" \
  --activity-type taskCreated --preview "You have a new task"
teams notify send-to-team <team-id> --topic "Deploy" \
  --activity-type deploymentComplete --preview "v2.3.1 deployed"
teams notify send-to-chat <chat-id> --topic "Update" \
  --activity-type statusUpdate --preview "Status changed"
```

### Apps & Tabs

```bash
teams app list <team-id>
teams app install <team-id> --app-id <catalog-app-id>
teams app uninstall <team-id> --app-id <installation-id>
teams tab list <team-id> <channel-id>
teams tab create <team-id> <channel-id> --app-id <app-id> --name "Wiki" --content-url <url>
teams tab delete <team-id> <channel-id> --tab-id <tab-id>
```

### Files

```bash
teams file list --team <team-id> --channel <channel-id>
teams file get --team <team-id> --channel <channel-id> --file-id <id>
teams file upload --team <team-id> --channel <channel-id> --file ./report.pdf
teams file download --team <team-id> --channel <channel-id> --file-id <id> --path ./local.pdf
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

Manage multiple tenants, delegated users, or service principals:

```bash
teams --profile prod auth login --device-code
teams --profile staging auth login --device-code --client-id ... --tenant-id ...

teams --profile prod team list
teams --profile staging message send --team <id> --channel <id> --body "Deployed"
```

Config file (`~/.config/teams-cli/config.toml` on Linux, `~/Library/Application Support/teams-cli/config.toml` on macOS, `%APPDATA%\teams-cli\config.toml` on Windows):

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
tenant_id = "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx"
auth_flow = "device-code"

[profiles.staging]
auth_app = "byo"
client_id = "yyyyyyyy-yyyy-yyyy-yyyy-yyyyyyyyyyyy"
tenant_id = "yyyyyyyy-yyyy-yyyy-yyyy-yyyyyyyyyyyy"
auth_flow = "device-code"
```

## Environment Variables

| Variable | Description |
|----------|-------------|
| `TEAMS_CLI_CLIENT_ID` | Azure AD application (client) ID |
| `TEAMS_CLI_CLIENT_SECRET` | Azure AD client secret |
| `TEAMS_CLI_TENANT_ID` | Azure AD tenant ID |
| `TEAMS_CLI_ACCESS_TOKEN` | Pre-obtained Microsoft Graph access token (skips login entirely) |
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

## Man Pages

Man pages are maintained under `docs/man/` and included in release archives:

```bash
man ./docs/man/teams.1
man ./docs/man/teams-config.5
man ./docs/man/teams-auth.7
man ./docs/man/teams-agent-contract.7
man ./docs/man/teams-examples.7
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
