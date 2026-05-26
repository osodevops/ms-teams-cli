# AI agent quickstart

This guide is for Codex, Claude, or another agent calling `teams` as a subprocess.

## 1. Require JSON

Always pass `--output json` from agent code:

```bash
teams auth status --output json
teams chat list --output json
```

The CLI auto-detects piped stdout and uses JSON, but explicit output is safer for agents.

## 2. Validate auth

```bash
teams auth doctor --output json
```

Agents should check:

- `.success == true`
- `.data.authenticated == true`
- `.data.token.auth_type == "delegated"` before sending normal Teams messages

## 3. Read before writing

```bash
teams chat list --page-size 20 --output json
teams message list --chat "$CHAT_ID" --page-size 20 --output json
```

Avoid printing full message bodies into logs. Keep raw Teams data in memory or redact it.

## 4. Send a message

```bash
teams message send --chat "$CHAT_ID" --body "$MESSAGE" --output json
```

For channel messages:

```bash
teams message send --team "$TEAM_ID" --channel "$CHANNEL_ID" --body "$MESSAGE" --output json
```

## 5. Branch on exit code

```bash
teams message send --chat "$CHAT_ID" --body "$MESSAGE" --output json
status=$?

case "$status" in
  0) echo "sent" ;;
  3) echo "reauth required" ;;
  4) echo "permission denied or inaccessible resource" ;;
  6) echo "rate limited" ;;
  *) echo "unexpected failure" ;;
esac
```

## 6. Agent safety rules

- Use allowlisted chat/team/channel IDs for writes.
- Ask for explicit human approval before destructive commands.
- Do not use Teams as a high-volume log sink.
- Deduplicate processed message IDs.
- Treat `403` for one chat as a skip unless every chat fails.
- Keep `auth token` output out of logs.
