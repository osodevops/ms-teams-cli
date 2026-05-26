# Authentication guide

This guide explains how `teams` authenticates to Microsoft Graph, what customers need to approve, and which auth model is suitable for commercial use.

## Recommended commercial model

Use delegated Microsoft Graph auth for normal CLI usage.

Delegated auth means:

- A real Microsoft 365 user signs in.
- Microsoft Graph calls run on behalf of that signed-in user.
- Teams messages sent by the CLI appear as that user.
- The user's own Teams permissions still apply.
- Tenant admins can grant consent to the OSO app once for the organization.

This matches Microsoft Graph's delegated access model and avoids asking every customer to create an Entra app registration just to use the CLI.

## Why client credentials are not the default

Client credentials produce app-only tokens. App-only Graph access is valid for some admin and read scenarios, but it is not the right model for normal live Teams chat/channel sending.

Microsoft documents ordinary chat message sending as a delegated work/school operation with `ChatMessage.Send`; the application permission shown for send is `Teamwork.Migrate.All`, which is for migration/import scenarios, not normal live conversations. Channel replies are similar: delegated `ChannelMessage.Send` is the normal model and application permissions are migration-only.

The CLI now rejects app-only tokens before mutating normal Teams messages so users get a clear local error instead of a confusing Graph failure.

Use client credentials only for operations backed by Microsoft Graph application permissions that are appropriate for the target API.

## OSO public client app

Delegated login defaults to OSO's multi-tenant public client app:

```text
Client ID: fba1b5d0-fdd0-4fe2-9729-9ccdc38f9595
Authority: organizations
Redirect URI: http://localhost:8400/callback
```

The default lets most users start with:

```bash
teams auth login
```

or, for SSH/headless use:

```bash
teams auth login --device-code
```

The OSO app is baked into the CLI as the default delegated public client. It still requires publisher verification before broad commercial release. Publisher verification should be completed before asking external enterprise tenants to consent.

## Admin consent

Generate an admin consent URL:

```bash
teams auth consent-url --tenant-id <tenant-id-or-domain> --output json
```

For the default app, this prints a URL like:

```text
https://login.microsoftonline.com/<tenant>/adminconsent?client_id=fba1b5d0-fdd0-4fe2-9729-9ccdc38f9595
```

Use a concrete tenant ID or verified tenant domain for customer onboarding. `organizations` is useful for sign-in discovery, but a customer admin consent link should normally target the customer's tenant explicitly.

## Current delegated permissions

The OSO app registration currently asks for these delegated Microsoft Graph permissions:

```text
User.Read
offline_access
Team.ReadBasic.All
Channel.ReadBasic.All
ChannelMessage.Send
ChannelMessage.Read.All
Chat.ReadWrite
ChatMessage.Send
ChatMessage.Read
User.ReadBasic.All
Presence.Read.All
```

These permissions cover the current read/write message, team/channel discovery, chat, user lookup, and presence smoke tests. Future features may need additional consent.

## Login options

Browser PKCE login:

```bash
teams auth login
```

Device-code login:

```bash
teams auth login --device-code
```

Custom scopes:

```bash
teams auth login --device-code --scopes "User.Read ChatMessage.Send offline_access"
```

Customer-owned delegated app:

```bash
teams auth login --device-code \
  --client-id <customer-client-id> \
  --tenant-id <customer-tenant-id>
```

Client credentials for supported app-only Graph operations:

```bash
export TEAMS_CLI_CLIENT_ID=<client-id>
export TEAMS_CLI_CLIENT_SECRET=<client-secret>
export TEAMS_CLI_TENANT_ID=<tenant-id>

teams auth login --client-credentials
```

Pre-obtained token:

```bash
export TEAMS_CLI_ACCESS_TOKEN=<access-token>
teams user me --output json
```

## BYO customer app

Use BYO mode when a customer requires their own Entra app registration.

Config example:

```toml
[default]
profile = "customer"
output = "json"

[profiles.customer]
auth_app = "byo"
client_id = "11111111-1111-1111-1111-111111111111"
tenant_id = "22222222-2222-2222-2222-222222222222"
auth_flow = "device-code"
```

Then:

```bash
teams --profile customer auth login --device-code
teams --profile customer auth doctor --output json
```

BYO delegated app requirements:

- Supported account type: usually single-tenant for locked-down enterprises.
- Public client flows enabled.
- Redirect URI for browser flow: `http://localhost:8400/callback`.
- Delegated Graph permissions required for the features the customer wants.
- Admin consent granted where required.

## Future unattended posting

For enterprises that want agents to post without a human user session, the correct direction is a Teams app/bot mode:

- Customer installs a Teams app/bot into the target chat, team, or channel.
- OSO stores the bot installation/conversation reference securely.
- Agents send proactive messages through the bot identity.
- Audit logs clearly show an application/bot posted the message.

This is separate from the current Graph delegated CLI mode. Do not market client credentials as the solution for normal unattended Teams chat posting.

## Token storage

Tokens are stored in the operating system keyring:

- macOS: Keychain
- Windows: Credential Manager
- Linux: Secret Service or compatible keyring backend

The config file stores profile settings, not access tokens.

Current known gap: automatic refresh-token handling is not yet release-grade. If a token expires and refresh does not happen, commands return `AUTH_TOKEN_EXPIRED` and the user must run `teams auth login` again.

## Diagnostics

Check configured auth app, consent URL, and token claims:

```bash
teams auth doctor --output json
```

Check whether the profile is authenticated:

```bash
teams auth status --output json
```

List profiles:

```bash
teams auth list --output json
```

Log out:

```bash
teams auth logout
teams auth logout --all
```

## Environment variables

| Variable | Purpose |
| --- | --- |
| `TEAMS_CLI_ACCESS_TOKEN` | Use this bearer token instead of the keyring token. |
| `TEAMS_CLI_CLIENT_ID` | Entra app client ID. |
| `TEAMS_CLI_CLIENT_SECRET` | Client secret for client credentials flow. |
| `TEAMS_CLI_TENANT_ID` | Tenant ID or tenant domain. |
| `TEAMS_CLI_DISABLE_KEYRING` | Test-only escape hatch used by CLI tests to avoid real OS keyring access. |

## Microsoft references

- [Microsoft Graph authentication and authorization basics](https://learn.microsoft.com/en-us/graph/auth/auth-concepts)
- [Send message in a chat](https://learn.microsoft.com/en-us/graph/api/chat-post-messages?view=graph-rest-1.0)
- [Send replies to a message in a channel](https://learn.microsoft.com/en-us/graph/api/chatmessage-post-replies?view=graph-rest-1.0)
- [Publisher verification](https://learn.microsoft.com/en-us/entra/identity-platform/mark-app-as-publisher-verified)
