# Command reference

This reference is a practical map of the current CLI surface. Run `teams <command> --help` for exact Clap-generated help on the installed binary.

## Global options

```text
teams [OPTIONS] <COMMAND>
```

| Option | Purpose |
| --- | --- |
| `-o, --output json|human|plain` | Force output format. Defaults to human on TTY and JSON when piped. |
| `-q, --quiet` | Suppress non-essential output. |
| `-v, --verbose` | Increase stderr logging. Repeat for debug/trace. |
| `--no-color` | Disable ANSI colors. |
| `--config <path>` | Use an explicit config path. |
| `--profile <name>` | Use a named credential profile. |
| `--timeout <seconds>` | Override request timeout. |
| `--retry <count>` | Override retry attempts. |
| `--page-size <count>` | Page size for paginated list operations. |
| `--all-pages` | Follow paginated Graph results until complete. |

## Auth

```bash
teams auth login [--device-code] [--client-id ID] [--tenant-id ID] [--scopes SCOPES]
teams auth login --client-credentials --client-id ID --client-secret SECRET --tenant-id ID
teams auth status
teams auth consent-url [--client-id ID] [--tenant-id ID]
teams auth doctor [--client-id ID] [--tenant-id ID]
teams auth list
teams auth switch NAME
teams auth logout [--profile NAME] [--all]
teams auth token [--format bearer|json]
```

Delegated login defaults to the OSO public client app. Client credentials always require explicit customer credentials.

## Users

```bash
teams user me
teams user get USER_ID_OR_UPN
teams user list [--filter ODATA_FILTER]
```

## Config

```bash
teams config init
teams config show
teams config get KEY
teams config set KEY VALUE
teams config path
teams config profiles
```

See [configuration](#configuration-file) for keys.

## Teams

```bash
teams team list
teams team get TEAM_ID
teams team create --name NAME [--description TEXT]
teams team update TEAM_ID [--name NAME] [--description TEXT]
teams team delete TEAM_ID
teams team clone TEAM_ID --name NAME [--parts apps,tabs,settings,channels,members] [--visibility private|public] [--description TEXT]
teams team archive TEAM_ID
teams team unarchive TEAM_ID
teams team members list TEAM_ID
teams team members add TEAM_ID --user-id USER_ID [--role member|owner]
teams team members remove TEAM_ID MEMBER_ID
```

Note: Microsoft Graph `/me/joinedTeams` does not support OData query customization, so `team list` intentionally avoids `$top` for that endpoint.

## Channels

```bash
teams channel list TEAM_ID
teams channel get TEAM_ID CHANNEL_ID
teams channel create TEAM_ID --name NAME [--description TEXT] [--membership-type standard|private|shared]
teams channel create TEAM_ID --name NAME --type private
teams channel update TEAM_ID CHANNEL_ID [--name NAME] [--description TEXT]
teams channel delete TEAM_ID CHANNEL_ID
teams channel members list TEAM_ID CHANNEL_ID
teams channel members add TEAM_ID CHANNEL_ID --user-id USER_ID [--role member|owner]
teams channel members remove TEAM_ID CHANNEL_ID MEMBER_ID
```

## Messages

```bash
teams message send (--team TEAM_ID --channel CHANNEL_ID | --chat CHAT_ID) (--body TEXT | --stdin) [--content-type text|html] [--adaptive-card PATH]
teams message list (--team TEAM_ID --channel CHANNEL_ID | --chat CHAT_ID)
teams message get --team TEAM_ID --channel CHANNEL_ID (MESSAGE_ID | --message MESSAGE_ID)
teams message reply --team TEAM_ID --channel CHANNEL_ID --message-id MESSAGE_ID (--body TEXT | --stdin) [--content-type text|html]
teams message update --team TEAM_ID --channel CHANNEL_ID (MESSAGE_ID | --message MESSAGE_ID) --body TEXT [--content-type text|html]
teams message delete --team TEAM_ID --channel CHANNEL_ID (MESSAGE_ID | --message MESSAGE_ID)
teams message react --team TEAM_ID --channel CHANNEL_ID --message-id MESSAGE_ID (REACTION | --reaction REACTION)
teams message unreact --team TEAM_ID --channel CHANNEL_ID --message-id MESSAGE_ID (REACTION | --reaction REACTION)
teams message pin --team TEAM_ID --channel CHANNEL_ID (MESSAGE_ID | --message MESSAGE_ID)
teams message unpin --team TEAM_ID --channel CHANNEL_ID (PINNED_MESSAGE_ID | --pinned-message-id PINNED_MESSAGE_ID)
```

Normal message mutation requires delegated auth. App-only/client-credentials tokens are rejected for these commands.

## Chats

```bash
teams chat list
teams chat get CHAT_ID
teams chat create [--chat-type group|oneOnOne] [--type group|oneOnOne] [--topic TOPIC] [--members USER_ID[:owner|guest],USER_ID[:owner|guest]]
teams chat update CHAT_ID --topic TOPIC
teams chat hide CHAT_ID --user-id USER_ID
teams chat unhide CHAT_ID --user-id USER_ID
teams chat members list CHAT_ID
teams chat members add CHAT_ID --user-id USER_ID [--role member|owner]
teams chat members remove CHAT_ID MEMBER_ID
```

When creating a chat, members default to the `owner` role. Azure AD guest users must be marked with a `:guest` suffix (e.g. `--members <your-id>,<guest-id>:guest`) or Microsoft Graph rejects the request.

Some meeting chats can appear in `chat list` but reject message reads if the signed-in user is no longer in the meeting roster. Treat that as a per-chat skip, not as proof that auth is broken.

## Presence

```bash
teams presence get
teams presence get --user USER_ID
teams presence get --users USER_ID,USER_ID
teams presence get-batch --user-ids USER_ID,USER_ID
teams presence set --availability AVAILABILITY --activity ACTIVITY [--expiration ISO8601_DURATION]
teams presence status --message TEXT [--expiry ISO8601_DATETIME]
teams presence clear
```

## Search

```bash
teams search messages QUERY [--top COUNT]
teams search messages --query QUERY [--top COUNT]
teams search users QUERY [--top COUNT]
teams search users --query QUERY [--top COUNT]
teams search teams QUERY
teams search teams --query QUERY
```

## Tags

```bash
teams tag list TEAM_ID
teams tag get TEAM_ID TAG_ID
teams tag create TEAM_ID --name NAME [--members USER_ID,USER_ID]
teams tag update TEAM_ID TAG_ID --name NAME
teams tag delete TEAM_ID TAG_ID
teams tag add-member TEAM_ID TAG_ID --user USER_ID
teams tag remove-member TEAM_ID TAG_ID --user USER_ID
```

## Meetings

```bash
teams meeting list
teams meeting get MEETING_ID
teams meeting create --subject SUBJECT [--start ISO8601] [--end ISO8601] [--allowed-presenters VALUE]
teams meeting update MEETING_ID [--subject SUBJECT] [--start ISO8601] [--end ISO8601] [--allowed-presenters VALUE]
teams meeting delete MEETING_ID
teams meeting join-url MEETING_ID
teams meeting attendance MEETING_ID [--report-id REPORT_ID]
```

## Notifications

```bash
teams notify send (--user USER_ID | --users USER_ID,USER_ID) --topic TEXT --activity-type TYPE --preview TEXT
teams notify send-to-team TEAM_ID --topic TEXT --activity-type TYPE --preview TEXT [--recipient-user USER_ID]
teams notify send-to-chat CHAT_ID --topic TEXT --activity-type TYPE --preview TEXT [--recipient-user USER_ID]
```

Activity type must match the Teams app manifest activity definitions.

## Apps and tabs

```bash
teams app list TEAM_ID
teams app install TEAM_ID --app-id CATALOG_APP_ID
teams app uninstall TEAM_ID --app-id INSTALLATION_ID
teams tab list TEAM_ID CHANNEL_ID
teams tab create TEAM_ID CHANNEL_ID --app-id APP_ID --name NAME --content-url URL
teams tab delete TEAM_ID CHANNEL_ID --tab-id TAB_ID
```

## Files

```bash
teams file list --team TEAM_ID --channel CHANNEL_ID
teams file get --team TEAM_ID --channel CHANNEL_ID --file-id FILE_ID
teams file upload --team TEAM_ID --channel CHANNEL_ID --file PATH [--name NAME]
teams file upload --team TEAM_ID --channel CHANNEL_ID --stdin --name NAME
teams file download --team TEAM_ID --channel CHANNEL_ID --file-id FILE_ID [--path PATH]
teams file delete --team TEAM_ID --channel CHANNEL_ID --file-id FILE_ID
teams file share --team TEAM_ID --channel CHANNEL_ID --file-id FILE_ID [--link-type view|edit|embed] [--scope anonymous|organization]
```

If `file download` runs without `--path`, raw bytes go to stdout. Use `--path` in scripts unless stdout is intentionally the file sink.

## Subscriptions and listener

```bash
teams subscribe create --resource RESOURCE --change-type created,updated,deleted --webhook-url URL [--expiration ISO8601] [--client-state STATE]
teams subscribe list
teams subscribe renew SUBSCRIPTION_ID [--expiration ISO8601]
teams subscribe delete SUBSCRIPTION_ID
teams listen [--port PORT]
```

Microsoft Graph requires an HTTPS notification URL. `teams listen` is a local HTTP listener for testing behind a tunnel or reverse proxy.

## Completions

```bash
teams completions bash
teams completions zsh
teams completions fish
teams completions powershell
```

## Configuration file

Default paths:

```text
Linux:   ~/.config/teams-cli/config.toml
macOS:   ~/Library/Application Support/teams-cli/config.toml
Windows: %APPDATA%\teams-cli\config.toml
```

Example:

```toml
[default]
profile = "prod"
output = "json"
page_size = 50
timeout = 30
retry = 3
color = true

[output]
format = "auto"
color = true
page_size = 50

[network]
timeout = 30
max_retries = 3
retry_backoff_base = 2

[profiles.prod]
tenant_id = "00000000-0000-0000-0000-000000000000"
auth_flow = "device-code"

[profiles.locked-down]
auth_app = "byo"
client_id = "11111111-1111-1111-1111-111111111111"
tenant_id = "22222222-2222-2222-2222-222222222222"
auth_flow = "device-code"
```

## Exit codes

| Code | Meaning | Script action |
| --- | --- | --- |
| 0 | Success | Parse stdout. |
| 1 | General error, or unauthenticated `auth status` | Inspect JSON error or status payload. |
| 2 | Invalid arguments or input | Fix command syntax or data. |
| 3 | Authentication error or token expired | Re-run `teams auth login`. |
| 4 | Permission denied | Check Graph permissions, membership, admin consent. |
| 5 | Resource not found | Re-discover IDs. |
| 6 | Rate limited | Back off and retry later. |
| 7 | Network error or timeout | Retry with backoff. |
| 8 | Server error | Retry with backoff. |
| 10 | Config or keyring error | Check config path, TOML, OS credential store. |
