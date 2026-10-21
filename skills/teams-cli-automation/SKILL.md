---
name: teams-cli-automation
description: "Automate Microsoft Teams with the teams CLI: read or set presence and status messages, create and manage online meetings and attendance reports, send activity notifications, subscribe to Graph change notifications, run the local webhook listener, or poll for new messages. Use when asked to set my Teams status, check if someone is available, schedule a Teams meeting, get a meeting join link, notify a user, watch a channel for new messages, or create a webhook subscription."
---

# Teams CLI Automation

Presence, meetings, notifications and event watching.

## Before you start

```bash
teams auth status --output json
```

Exit 1 or 3 means you are not signed in: use `teams-cli-setup`. Always pass `--output json` when parsing. Exit 4 means a missing scope or consent. Exit 6 means back off.

## Presence

```bash
teams presence get --output json
teams presence get --user <user-id> --output json
teams presence get-batch --user-ids <user-id>,<user-id> --output json
teams presence set --availability Busy --activity InACall --expiration PT1H
teams presence set-preferred --availability DoNotDisturb --expiration PT2H
teams presence status --message "Back at 14:00" --expiry 2026-01-01T14:00:00Z
teams presence clear
teams presence clear-preferred
```

- `set` needs a valid pair: `--availability` and `--activity` (for example `Available` + `Available`, `Busy` + `InACall`). Expiration is `PT5M` to `PT4H`.
- `set-preferred` overrides every session until it expires. Always set an expiration and tell the user to `clear-preferred` afterwards.
- Read presence for other people only when the user asked for it.

## Meetings

```bash
teams meeting list --output json
teams meeting get <meeting-id> --output json
teams meeting create --subject "Sprint review" --start 2026-01-01T10:00:00Z --end 2026-01-01T10:30:00Z
teams meeting update <meeting-id> --subject "Sprint review (moved)" --start 2026-01-01T11:00:00Z --end 2026-01-01T11:30:00Z
teams meeting join-url <meeting-id> --output json
teams meeting attendance <meeting-id> --output json
teams meeting attendance <meeting-id> --report-id <report-id> --output json
teams meeting delete <meeting-id>
```

- Times are ISO 8601 with an explicit offset or `Z`. Ask for the time zone if the user gave none.
- `meeting list` only shows meetings you created.
- `attendance` lists reports. Pass `--report-id` for the individual records.
- `meeting delete` removes the meeting and its join link for every participant: confirm the subject and id with the user first.

## Activity notifications

```bash
teams notify send --user <user-id> --topic "Deploy" --activity-type <activity-type> --preview "Deploy finished"
teams notify send-to-team <team-id> --topic "Deploy" --activity-type <activity-type> --preview "Deploy finished"
teams notify send-to-chat <chat-id> --topic "Deploy" --activity-type <activity-type> --preview "Deploy finished" --recipient-user <user-id>
```

`--activity-type` must match an activity declared in the manifest of a Teams app installed for the recipient. Without one the call fails. A notification is not a chat message: use `teams-cli-messaging` to post text.

## Change notifications

Graph pushes events to a public HTTPS URL. `teams listen` is plain HTTP on localhost, so put an HTTPS reverse proxy or tunnel in front of it.

```bash
teams listen --port 8080
teams subscribe create --resource <resource-path> --change-type created --webhook-url <https-url> --expiration 2026-01-01T12:00:00Z --client-state <random-secret>
teams subscribe list --output json
teams subscribe renew <subscription-id> --expiration 2026-01-02T12:00:00Z
teams subscribe delete <subscription-id>
```

- `listen` prints each notification as one JSON object per line (NDJSON). Read it line by line, and keep other output off that stream.
- Graph validates the webhook URL when you create the subscription, so start `listen` and the tunnel first.
- Use a random `--client-state` and compare it on every notification.
- Subscriptions expire quickly: Teams message, chat and presence resources last at most one hour, which is also the default when `--expiration` is omitted. Renew before `expirationDateTime`, or re-create.
- Message resources need extra permissions, and some need admin consent. See `teams subscribe --help` and the Graph docs.
- Delete subscriptions you created when done. `subscribe delete` stops delivery: confirm the id and its `resource` from `subscribe list` with the user first. There is no `subscribe get`.

## Polling instead

When no HTTPS endpoint is possible, poll and diff by message id.

```bash
teams message list --chat <chat-id> --output json
teams message list --team <team-id> --channel <channel-id> --page-size 20 --output json
```

1. Store the ids already seen (a file or state store you own).
2. Poll every 30 to 60 seconds or more. On exit 6, double the wait.
3. Process ids not seen. Follow `metadata.has_more` or add `--all-pages` for the first full pass only.
4. Do not poll many chats in parallel.

## Related skills

- `teams-cli-setup`: install, sign in, exit codes, output contract.
- `teams-cli-messaging`: read, send and reply in chats and channels.
- `teams-cli-admin`: teams, channels, tags, apps and tabs.
