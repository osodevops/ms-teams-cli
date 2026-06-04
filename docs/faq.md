# FAQ

## What does this repo do?

It builds a single Rust binary named `teams`. The CLI wraps Microsoft Graph APIs for Teams automation and emits structured output that AI agents and scripts can consume.

## Is this a Teams bot?

No. The current CLI is a Microsoft Graph command line tool. It can be used by agents as a subprocess, but it is not a Bot Framework app running inside Teams.

## Can agents send Teams messages with it?

Yes, using delegated auth. A user signs in and the CLI posts as that user, subject to that user's Teams permissions and the tenant's Graph consent policy.

## Can it post as an application with client credentials?

Not for normal live Teams chat/channel messages. Microsoft Graph's ordinary send APIs are delegated-user APIs for normal use; the application permission surfaced for sending is a migration/import path. For unattended service-identity posting, the product should add a Teams app/bot proactive messaging mode.

## Do customers need to create their own client ID?

Not in the default commercial model. The CLI defaults to OSO's multi-tenant public client app. Customers can grant admin consent to that app.

Customers can still use BYO mode when their enterprise policy requires their own app registration.

## What is the OSO client ID?

```text
fba1b5d0-fdd0-4fe2-9729-9ccdc38f9595
```

Do not treat the client ID as a secret. Client secrets, refresh tokens, and access tokens are secrets.

## Is the OSO app ready for external commercial tenants?

Not fully. The app is built into the CLI as the default delegated public client, but it must be publisher verified before broad public release so Microsoft consent prompts show a verified publisher badge.

## What permissions does the app request?

Current delegated permissions:

```text
User.Read
offline_access
Team.ReadBasic.All
Channel.ReadBasic.All
ChannelMessage.Send
Chat.ReadWrite
ChatMessage.Send
ChatMessage.Read
User.ReadBasic.All
Presence.Read.All
```

The default avoids `ChannelMessage.Read.All` because Microsoft marks that delegated Graph scope as admin-consent required. Customers that need channel message reads should grant it explicitly with `--scopes` or through a customer-owned app. Future features may require additional permissions.

## Why did `team list` return no teams?

`team list` uses `/me/joinedTeams`, which returns teams where the signed-in user is a direct member. If the user is not a direct member of any team, it can return an empty list even if they have chats or meetings.

## Why can `chat list` show a meeting chat that `message list --chat` cannot read?

Teams/Graph can list chats that later fail message reads because the user is no longer in the meeting roster or lacks access to that thread. Treat this as a per-chat `403` and skip it.

## Why did I get `AUTH_TOKEN_EXPIRED`?

The stored access token expired. The CLI requests `offline_access`, but automatic refresh-token handling is still a release-readiness gap. Run:

```bash
teams auth login --device-code
```

Then retry the command.

## Where are tokens stored?

In the operating system keyring:

- macOS Keychain
- Windows Credential Manager
- Linux Secret Service-compatible keyring

Tokens are not stored in `config.toml`.

## Where is the config file?

```text
Linux:   ~/.config/teams-cli/config.toml
macOS:   ~/Library/Application Support/teams-cli/config.toml
Windows: %APPDATA%\teams-cli\config.toml
```

Check with:

```bash
teams config path
```

## Does this work on Windows?

The code is designed to work on Windows and CI runs on `windows-latest`. Windows release readiness still requires full CI matrix validation and a live smoke test on Windows Credential Manager.

## What output should agents parse?

Use `--output json`. The CLI also auto-detects non-TTY stdout and defaults to JSON when piped.

```bash
teams chat list --output json
```

Agents should branch on exit code first, then parse the JSON envelope.

## Can this be used for high-volume logging?

No. Teams should not be used as a log sink. Send concise human-readable messages that people are expected to read.

## Can it read all Teams data in a tenant?

No, not with delegated auth. Delegated mode is scoped by the signed-in user's access and the app's granted permissions. Tenant-wide read scenarios need carefully reviewed application permissions or resource-specific consent depending on the Graph API.

## What is the safest pilot?

Use a dedicated test team/channel, a test profile, and non-sensitive messages. Validate:

```bash
teams auth doctor --output json
teams user me --output json
teams team list --output json
teams chat list --output json
teams message send --team "$TEAM_ID" --channel "$CHANNEL_ID" --body "teams-cli smoke test" --output json
```
