---
name: teams-cli-messaging
description: "Read, send and reply in Microsoft Teams chats and channels with the teams CLI. Use when asked to post a Teams message, read a chat, list channel messages, reply in a thread, edit, delete or restore a message, react or pin, @mention someone, find a channel id, chat id or user id, search Teams messages, send or download attachments and inline images, upload or share channel files, or create and manage a chat."
---

# Teams CLI Messaging

Read and write Teams messages as the signed-in user.

## Before you start

```bash
teams auth status --output json
```

Exit 1 or 3 means you are not signed in: use `teams-cli-setup`. Always pass `--output json` when parsing. Read before you write, and echo the target (chat or channel name) back to the user before sending.

## Find ids

```bash
teams team list --output json
teams channel list <team-id> --output json
teams chat list --order-by activity --output json
teams user me --output json
teams user resolve "<name-or-email>" --output json
teams search teams "<name>" --output json
teams search messages "<text>" --top 10 --output json
```

- `chat list --order-by activity` puts the newest chat first and adds `lastMessagePreview`.
- `user resolve` turns a name or email into candidate user ids. Ask the user to pick when there are several.
- Chat ids look like `19:...@thread.v2`. Quote them in the shell.

## Read

```bash
teams message list --chat <chat-id> --output json
teams message list --team <team-id> --channel <channel-id> --output json
teams message list --team <team-id> --channel <channel-id> --message-id <message-id> --output json
teams message get --team <team-id> --channel <channel-id> <message-id> --output json
teams message get --team <team-id> --channel <channel-id> --reply <reply-id> <message-id> --output json
```

- `message list` returns one page. Check `metadata.has_more`, or add `--all-pages` to get everything.
- `--message-id` lists the replies in a channel thread instead of the thread roots.
- There are no date or text filters on `message list`. Filter the JSON yourself, or use `search messages`.
- `message get` is for channel messages; `--reply` reads one reply, with the parent post as `<message-id>`. Read a single chat message from `message list --chat`.
- Some meeting chats list but reject reads (403) once you left the roster. Skip them.

## Write

```bash
teams message send --chat <chat-id> --body "Build is green"
teams message send --team <team-id> --channel <channel-id> --subject "Release" --body "v1.2 is out"
teams message reply --team <team-id> --channel <channel-id> --message-id <message-id> --body "Thanks"
teams message send --chat <chat-id> --quote <message-id> --body "Yes, that works"
teams message send --chat <chat-id> --mention <user-email> --body "Can you review?"
teams message send --chat <chat-id> --mention <user-email> --body "Hey {@0}, can you review?"
teams message send --chat <chat-id> --content-type html --body "<b>Done</b>"
teams message send --chat <chat-id> --stdin
teams message update --chat <chat-id> <message-id> --body "Corrected text"
teams message update --team <team-id> --channel <channel-id> --reply <reply-id> <message-id> --body "Corrected text"
teams message update --chat <chat-id> <message-id> --mention <user-email> --body "Hey {@0}, corrected text"
teams message react --chat <chat-id> --message-id <message-id> like
teams message react --team <team-id> --channel <channel-id> --reply <reply-id> --message-id <message-id> like
teams message unreact --chat <chat-id> --message-id <message-id> like
teams message pin --team <team-id> --channel <channel-id> <message-id>
teams message unpin --team <team-id> --channel <channel-id> <pinned-message-id>
```

- Content is `text` by default. `--content-type html` sends HTML; there is no markdown option. Use `--stdin` for long or multi-line bodies.
- `--mention` takes an Entra object id or UPN and builds a real @mention. Do not type raw `<at>` markup.
- Mentions are prepended unless the body places them: `{@0}` is the first `--mention`, `{@1}` the second, `{@}` the only one. Place every `--mention` once you use tokens. `{{@0}}` is the literal text.
- `update` replaces the whole body. Pass `--mention` again for every mention the edited message should keep.
- `--subject` works on channel root messages only. `--quote` works in chats only.
- Replies exist for channel threads. In a chat, send a normal message or use `--quote`.
- A send that fails with exit 7 or 8 may still have posted. List recent messages before re-sending.
- `--reply <reply-id>` targets a reply in a channel thread for `update`, `react` and `unreact`; `<message-id>` is then the parent post. Channel only.
- `pin` and `unpin` are channel only. `unpin` takes the pinned-message id, not the message id.

## Delete and restore

Delete is a soft delete and only works on your own messages. Confirm with the user and echo the target first.

```bash
teams message delete --chat <chat-id> <message-id> --yes
teams message undelete --chat <chat-id> <message-id>
teams message delete --team <team-id> --channel <channel-id> --reply <reply-id> <message-id> --yes
```

`update` uses the same reply targeting: `--reply <reply-id>` with the parent as `<message-id>`.

Without `--yes` the command exits 2 and sends nothing. Channel deletes and edits need `ChannelMessage.ReadWrite`, which needs admin consent and is not in the default scopes: expect exit 4 until an admin grants it (`teams auth consent-url` prints the URL for them).

## Attachments

```bash
teams message get --team <team-id> --channel <channel-id> <message-id> --with-attachments --output json
teams message attachments list --chat <chat-id> <message-id> --output json
teams message attachments download --chat <chat-id> <message-id> --dir ./downloads
teams message attachments download --chat <chat-id> <message-id> --index 0 --path ./file.bin
teams message send --chat <chat-id> --body "Report" --attach ./report.pdf
teams message send --chat <chat-id> --body "Screenshot" --image ./shot.png
```

Pattern: `message get --with-attachments` (channel) or `attachments list` (chat or channel), read `attachment_items`, then `attachments download --dir`. `--index` picks one item. File attachments need `Files.Read.All`. `--attach` needs `Files.ReadWrite` (chats) or `Files.ReadWrite.All` (channels); `--image` needs no extra scope and is capped at 3 MB. Never print downloaded bytes to a terminal.

`message attachments delete ... --yes` removes only files you attached from your own OneDrive. Confirm first.

## Channel files

```bash
teams file list --team <team-id> --channel <channel-id> --output json
teams file get --team <team-id> --channel <channel-id> --file-id <file-id> --output json
teams file upload --team <team-id> --channel <channel-id> --file ./notes.md
teams file download --team <team-id> --channel <channel-id> --file-id <file-id> --path ./notes.md
teams file share --team <team-id> --channel <channel-id> --file-id <file-id> --link-type view --scope organization
teams file delete --team <team-id> --channel <channel-id> --file-id <file-id>
```

`file download` writes to stdout without `--path`. Always set it. Use `--scope organization` unless the user asks for `anonymous`. `file delete` is permanent: confirm first.

## Chats

```bash
teams chat get <chat-id> --output json
teams chat create --chat-type group --topic "Release war room" --members <your-user-id>,<user-id>
teams chat update <chat-id> --topic "New topic"
teams chat hide <chat-id> --user-id <user-id>
teams chat unhide <chat-id> --user-id <user-id>
teams chat members list <chat-id> --output json
```

Include your own id (`teams user me`) in `--members`; the CLI does not add it. Mark guest users as `<user-id>:guest`. Members default to owner.

## Limits

- Client credentials (app-only tokens) cannot post as a user. Use a delegated login.
- Exit 4 means a missing scope or consent. Report it instead of retrying.

## Related skills

- `teams-cli-setup`: install, sign in, exit codes, output contract.
- `teams-cli-admin`: teams, channels, tags, apps and tabs.
- `teams-cli-automation`: presence, meetings, notifications, webhooks.
