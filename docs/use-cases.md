# Use cases

This document describes the high-value workflows for `teams` and the auth model each one needs.

## Agency or consultancy client communications

Agencies often coordinate client work in Teams chats and channels. `teams` lets their agents and scripts:

- Read recent client messages before preparing a response.
- Post status updates into an existing client chat.
- Send release notes to a project channel.
- Upload lightweight report artifacts.
- Keep agency-side automation outside the Teams UI while preserving the user's identity.

Recommended auth:

- Delegated OSO public client app for most agencies.
- BYO customer app if the client tenant requires an approved app registration.

Example:

```bash
teams chat list --output json
teams message list --chat "$CLIENT_CHAT_ID" --page-size 10 --output json
teams message send --chat "$CLIENT_CHAT_ID" --body "The deployment is complete and smoke tests passed."
```

Operational caution: never dump raw client chat contents into CI logs or agent traces. Summarize or redact.

## Enterprise agent posting to Teams

Enterprises want Claude, Codex, or internal agents to post into Teams where employees already work. There are two separate modes:

1. Delegated user mode: the agent posts as a signed-in user. This is what the CLI supports today.
2. Bot/app mode: the agent posts as an installed Teams app/bot. This is the future mode for unattended enterprise posting.

Use delegated mode when a human owner is acceptable and audit trails should show that user's identity.

Use future bot mode when messages must come from a service identity, run unattended, or avoid relying on a user's refresh token.

## DevOps notifications

Teams can receive deployment notes, incident summaries, test results, or release approvals.

Good examples:

- "Release 2.4.1 deployed to production. Smoke tests passed."
- "Incident bridge notes have been summarized in this thread."
- "Preview environment is ready: <url>"

Poor examples:

- High-volume application logs.
- Every CI step as a separate Teams message.
- Repeated status spam no human will read.

Microsoft explicitly warns against using Teams as a log file for ordinary message send APIs.

## Support triage

Support teams can use the CLI to:

- Search for related messages.
- Identify a customer's team or channel.
- Pull recent thread context.
- Post a structured support update.

Example:

```bash
teams search messages --query "customer outage" --top 10 --output json
teams message send --team "$TEAM_ID" --channel "$CHANNEL_ID" --body "Support summary: investigation is ongoing."
```

## Knowledge and meeting workflows

Meeting commands can help agents:

- List meetings created by the signed-in user.
- Retrieve join URLs.
- Create simple online meetings.
- Fetch attendance reports where permissions allow.

Example:

```bash
teams meeting create \
  --subject "Client project review" \
  --start "2026-05-27T10:00:00Z" \
  --end "2026-05-27T10:30:00Z" \
  --output json
```

## Teams administration

The CLI includes team, channel, member, app, tab, tag, file, and subscription operations. These are powerful and should be gated by profiles and test tenants.

Safe read-only checks:

```bash
teams team list --output json
teams channel list "$TEAM_ID" --output json
teams team members list "$TEAM_ID" --output json
```

Potentially destructive commands:

```bash
teams team delete "$TEAM_ID"
teams team archive "$TEAM_ID"
teams app uninstall "$TEAM_ID" --app-id "$INSTALLATION_ID"
teams file delete --team "$TEAM_ID" --channel "$CHANNEL_ID" --file-id "$FILE_ID"
```

Do not expose destructive operations to autonomous agents without explicit policy, allowlists, and confirmation controls.

## Change notifications

Subscriptions and `teams listen` can support event-driven workflows:

- Listen for message changes.
- Trigger an agent on new messages.
- Maintain a local processing queue.

Important constraints:

- Microsoft Graph requires HTTPS notification URLs.
- `teams listen` is local HTTP and needs a tunnel or reverse proxy for live Graph callbacks.
- Subscriptions expire and must be renewed.
- Notifications should be deduplicated by ID.

## Data minimization

For commercial use, build examples and agent prompts around least data:

- Prefer metadata, IDs, timestamps, sender presence, and short body snippets.
- Avoid storing full chat transcripts unless the customer has explicitly agreed.
- Keep raw Teams data out of logs by default.
- Provide redaction examples in customer docs.

## Recommended first customer pilot

1. Create a dedicated OSO/customer test team and channel.
2. Grant consent to the OSO app or a BYO app.
3. Run `auth doctor`.
4. Run read-only commands.
5. Send one smoke-test message.
6. Reply once.
7. Upload/download a small file.
8. Document any Graph `403`, policy, or tenant restrictions.
9. Clean up the test artifacts.
10. Review audit logs with the customer's admin.
