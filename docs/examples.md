# Examples

These examples use placeholder IDs. Discover real IDs with `team list`, `channel list`, `chat list`, and `user` commands before writing to Teams.

## Login and diagnostics

```bash
teams auth login --device-code
teams auth doctor --output json | jq .
teams auth status --output json
```

PowerShell:

```powershell
teams auth login --device-code
teams auth doctor --output json | ConvertFrom-Json
```

## Find a team and channel

```bash
TEAM_ID=$(teams team list --output json | jq -r '.data[] | select(.displayName=="Engineering") | .id')
CHANNEL_ID=$(teams channel list "$TEAM_ID" --output json | jq -r '.data[] | select(.displayName=="General") | .id')
```

PowerShell:

```powershell
$teams = teams team list --output json | ConvertFrom-Json
$teamId = ($teams.data | Where-Object displayName -eq "Engineering").id
$channels = teams channel list $teamId --output json | ConvertFrom-Json
$channelId = ($channels.data | Where-Object displayName -eq "General").id
```

## Send a channel message

```bash
teams message send \
  --team "$TEAM_ID" \
  --channel "$CHANNEL_ID" \
  --body "Release 2.4.1 is deployed."
```

HTML body:

```bash
teams message send \
  --team "$TEAM_ID" \
  --channel "$CHANNEL_ID" \
  --content-type html \
  --body "<b>Release complete</b><br/>Version: 2.4.1"
```

## Send from stdin

```bash
git log --oneline -5 | teams message send \
  --team "$TEAM_ID" \
  --channel "$CHANNEL_ID" \
  --stdin
```

```bash
cat report.html | teams message send \
  --team "$TEAM_ID" \
  --channel "$CHANNEL_ID" \
  --stdin \
  --content-type html
```

## Read recent chat messages safely

```bash
CHAT_ID=$(teams chat list --output json | jq -r '.data[0].id')
teams message list --chat "$CHAT_ID" --page-size 10 --output json |
  jq '.data[] | {id, createdDateTime, messageType, bodyType: .body.contentType, bodyChars: (.body.content | length)}'
```

This pattern avoids printing real message bodies in logs.

## Reply to a channel message

```bash
MESSAGE_ID=$(teams message list --team "$TEAM_ID" --channel "$CHANNEL_ID" --output json | jq -r '.data[0].id')

teams message reply \
  --team "$TEAM_ID" \
  --channel "$CHANNEL_ID" \
  --message-id "$MESSAGE_ID" \
  --body "Acknowledged."
```

## Controlled smoke test

Use a dedicated test channel.

```bash
BODY="teams-cli smoke test $(date -u +%Y-%m-%dT%H:%M:%SZ)"

SENT=$(teams message send \
  --team "$TEAM_ID" \
  --channel "$CHANNEL_ID" \
  --body "$BODY" \
  --output json)

MESSAGE_ID=$(echo "$SENT" | jq -r '.data.id')

teams message reply \
  --team "$TEAM_ID" \
  --channel "$CHANNEL_ID" \
  --message-id "$MESSAGE_ID" \
  --body "reply smoke test" \
  --output json
```

Cleanup only if the signed-in user and tenant policy allow deleting messages:

```bash
teams message delete \
  --team "$TEAM_ID" \
  --channel "$CHANNEL_ID" \
  --message "$MESSAGE_ID" \
  --output json
```

## Upload and download a file

```bash
printf 'smoke test\n' | teams file upload \
  --team "$TEAM_ID" \
  --channel "$CHANNEL_ID" \
  --stdin \
  --name teams-cli-smoke.txt \
  --output json
```

```bash
teams file list --team "$TEAM_ID" --channel "$CHANNEL_ID" --output json

teams file download \
  --team "$TEAM_ID" \
  --channel "$CHANNEL_ID" \
  --file-id "$FILE_ID" \
  --path ./teams-cli-smoke.txt
```

## Read a screenshot pasted into a message

Users paste screenshots directly into Teams messages; those are stored as
Graph "hosted contents", not file attachments. Enumerate and download
everything readable in a message:

```bash
teams message attachments list \
  --team "$TEAM_ID" \
  --channel "$CHANNEL_ID" \
  "$MESSAGE_ID" --output json

teams message attachments download \
  --team "$TEAM_ID" \
  --channel "$CHANNEL_ID" \
  "$MESSAGE_ID" --dir ./attachments --output json
```

The download result lists one entry per item with its local path, size, and
MIME type — ready for a multimodal agent to read. Use `--chat "$CHAT_ID"`
instead of `--team`/`--channel` for chat messages, `--reply "$REPLY_ID"` for
a reply inside a channel thread, and `--index N --path -` to stream a single
item to stdout. File attachments (SharePoint/OneDrive references) need the
`Files.Read.All` delegated scope; inline images work with plain message-read
scopes.

## Agency client update

```bash
CLIENT_CHAT_ID="<chat-id>"

cat <<'TEXT' | teams message send --chat "$CLIENT_CHAT_ID" --stdin
Daily update:
- Deployment completed
- Smoke tests passed
- Follow-up items are in the project board
TEXT
```

## Agent loop skeleton

```bash
teams message list --chat "$CHAT_ID" --page-size 20 --output json |
  jq -r '.data[] | [.id, .createdDateTime, (.body.content // "")] | @tsv' |
  while IFS=$'\t' read -r id created body; do
    response=$(printf '%s\n' "$body" | ./agent-decide-response)
    if [ -n "$response" ]; then
      teams message send --chat "$CHAT_ID" --body "$response" --output json
    fi
  done
```

Add deduplication before using an agent loop in production. Store processed message IDs.

## Error handling in shell

```bash
if ! output=$(teams message send --chat "$CHAT_ID" --body "Hello" --output json); then
  code=$?
  case "$code" in
    3) teams auth login --device-code ;;
    4) echo "Permission denied; check consent and chat membership" >&2 ;;
    6) sleep 30 ;;
    *) echo "$output" >&2 ;;
  esac
fi
```

## Windows environment variables

PowerShell:

```powershell
$env:TEAMS_CLI_TENANT_ID = "<tenant-id>"
$env:TEAMS_CLI_CLIENT_ID = "<client-id>"
teams auth login --device-code
```

Command Prompt:

```cmd
set TEAMS_CLI_TENANT_ID=<tenant-id>
set TEAMS_CLI_CLIENT_ID=<client-id>
teams auth login --device-code
```
