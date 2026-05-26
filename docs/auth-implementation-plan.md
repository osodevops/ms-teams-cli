# Authentication and Teams App Implementation Plan

**Status:** Draft implementation plan
**Last reviewed:** 2026-05-26
**Scope:** Commercial authentication, delegated Microsoft Graph access, bring-your-own Entra apps, and unattended Teams posting.

## Executive Decision

The commercial product should use two complementary Microsoft integration models:

1. **CLI user actions use delegated Microsoft Graph auth.** The default path is an OSO-owned multi-tenant Microsoft Entra public client app. Users authenticate with device code or authorization code plus PKCE. Publisher verification is required before broad external commercial rollout. This supports agencies, consultancies, and enterprise users who want the CLI or an AI coding agent to act as the signed-in user.
2. **Unattended posting uses a Teams bot app.** Microsoft Graph app-only tokens cannot send normal live Teams chat/channel messages. For userless/proactive posting, OSO needs a Teams bot app installed into the customer tenant/team/chat and a bot service that sends Bot Framework proactive messages.

Client credentials remain useful for admin/read/service operations that Microsoft Graph supports with application permissions, but they must not be marketed as the general way for agents to post normal Teams messages.

## Microsoft Platform Constraints

- Device code and auth code with PKCE are suitable public-client flows for CLI sign-in.
- A commercial multi-tenant CLI app should be publisher verified and should expect tenant admin consent for non-trivial Graph scopes.
- Teams channel/chat message send through Microsoft Graph is delegated for normal live messages. App-only support is limited to migration/import scenarios and is not a production posting model.
- Teams bot proactive messaging requires the bot to be installed in the target personal scope, team, group chat, or channel context and requires storing conversation references received from Teams.
- Graph app-only permissions can help install the Teams app/bot or manage Teams app installation, but the actual unattended message should be sent through Bot Framework, not app-only Graph chat/channel message endpoints.

Primary Microsoft references:

- Microsoft identity public/client auth flows: <https://learn.microsoft.com/en-us/entra/identity-platform/msal-authentication-flows>
- Microsoft Graph permissions model: <https://learn.microsoft.com/en-us/graph/permissions-overview>
- Admin consent: <https://learn.microsoft.com/en-us/entra/identity/enterprise-apps/grant-admin-consent>
- Channel message send API: <https://learn.microsoft.com/en-us/graph/api/channel-post-messages>
- Teams proactive bot messages: <https://learn.microsoft.com/en-us/microsoftteams/platform/graph-api/proactive-bots-and-messages/graph-proactive-bots-and-messages>

## Target Customer Paths

### Agencies and Consultancies

Use case: a consultant uses Teams with multiple client tenants and wants Claude, Codex, shell scripts, or a human CLI session to read and post messages as that consultant.

Recommended auth:

- `teams auth login --device-code --tenant-id <client-tenant-or-organizations>`
- Default OSO public client ID, no customer app registration required.
- Named profiles per client tenant.
- Delegated permissions only.

Expected identity:

- Messages sent through Graph appear as the signed-in user.

Admin involvement:

- Low for permissive tenants.
- Required where user consent is blocked or requested scopes require admin consent.

### Enterprise User-Delegated Automation

Use case: an enterprise wants AI agents to post as a controlled user account, for example `teams-agent@customer.com`.

Recommended auth:

- Dedicated licensed service user or controlled user account.
- Delegated device-code or PKCE login once, with refresh-token storage in OS keyring.
- Strict documentation that this posts as that user and inherits that user's access.

Expected identity:

- Messages appear as the service user or signed-in user.

Admin involvement:

- Admin creates/approves the account and grants delegated consent to the OSO app or customer BYO app.

### Enterprise Unattended Bot Posting

Use case: enterprise wants Claude/Codex/platform automation to post without being a human or service user.

Recommended auth:

- Install OSO Teams bot app in the tenant/team/chat.
- Bot service stores conversation references and sends proactive messages via Bot Framework.
- Graph app-only permissions may be used for Teams app installation, not for live Graph message posting.

Expected identity:

- Messages appear from the OSO/agent bot.

Admin involvement:

- Admin consents to the bot app and any Graph application permissions needed for app installation.
- Teams admin may publish/approve the app in the Teams admin center or install it from the Teams Store once available.

### Locked-Down Enterprise BYO App

Use case: customer policy blocks third-party multi-tenant apps or requires customer-owned app registrations.

Recommended auth:

- Customer registers their own Entra public client app for delegated CLI auth.
- Customer optionally registers their own confidential app/bot for customer-hosted bot mode.
- CLI keeps `--client-id`, `--tenant-id`, and profile configuration support.

Expected identity:

- Delegated mode posts as the signed-in customer user.
- Bot mode posts as the customer-owned bot.

Admin involvement:

- High, but fully under customer control.

## Required Microsoft Assets

### 1. OSO CLI Public Client App

Purpose: default delegated sign-in for the CLI.

Created OSO app registration:

- Application/client ID: `fba1b5d0-fdd0-4fe2-9729-9ccdc38f9595`
- Redirect URI: `http://localhost:8400/callback`

Required settings:

- Supported account types: accounts in any organizational directory.
- App type: public client, no client secret.
- Public client flows enabled.
- Redirect URI for browser PKCE loopback login.
- Device code flow support.
- Verified publisher configured before broad external commercial release.
- Minimal delegated Microsoft Graph API permissions.

Recommended initial delegated scopes:

- `User.Read`
- `offline_access`
- `Team.ReadBasic.All`
- `Channel.ReadBasic.All`
- `ChannelMessage.Send`
- `ChannelMessage.Read.All`
- `Chat.ReadWrite`
- `ChatMessage.Send`
- `ChatMessage.Read`
- `User.ReadBasic.All`
- `Presence.Read.All`

Implementation note: split these into documented scope presets before release. The current default scope string is broad and convenient for testing, but commercial onboarding should explain exactly why each scope exists and allow lower-scope profiles where possible.

### 2. Customer BYO Public Client App

Purpose: delegated sign-in for strict enterprises.

Required settings:

- Single-tenant or multi-tenant, depending on customer policy.
- Public client flows enabled.
- Same redirect URI pattern as the OSO CLI app.
- Same delegated Graph permissions as the OSO profile they want to use.
- Admin consent granted by customer admin.

CLI requirements:

- Continue supporting `--client-id` and `--tenant-id`.
- Support config profiles with customer app IDs.
- Provide a generated admin consent URL and validation command.

### 3. OSO Teams Bot App and Bot Service

Purpose: unattended/proactive posting.

Required Microsoft assets:

- Azure Bot resource or equivalent Bot Framework registration.
- Confidential Entra app for the bot/service.
- Teams app manifest with bot definition, Teams scopes, icons, and `webApplicationInfo`.
- Publisher verification for customer-facing multi-tenant apps.
- Teams app package for tenant upload, org catalog, and eventually Teams Store distribution.

Required backend capabilities:

- HTTPS Bot Framework messaging endpoint.
- Tenant installation and consent tracking.
- Conversation reference storage keyed by tenant, team/chat/channel, and installation scope.
- Proactive message send API.
- Audit logs for message sends.
- Secret/certificate management.

Recommended Graph application permissions for installation should be finalized against the exact install flows, but likely include the least-privileged `TeamsAppInstallation.*Self*` permissions that match user/team/chat installation targets.

## CLI Implementation Work

### Phase 0: Product and Security Decisions

Deliverables:

- Finalize OSO CLI app registration owner tenant.
- Finalize whether bot service is OSO-hosted, customer-hosted, or both.
- Decide default tenant authority:
  - `organizations` for normal commercial login.
  - Explicit tenant ID for enterprise/customer-specific login.
- Decide default message identity:
  - `--as user` for delegated Graph.
  - `--as bot` for bot service.

Acceptance criteria:

- Written permission matrix approved.
- Security review agrees that tokens are local-only for CLI delegated mode.
- Bot service hosting model and data retention rules are defined.

### Phase 1: Default OSO Delegated Auth

Deliverables:

- Add a built-in OSO public client ID constant for delegated auth.
- Allow `teams auth login` and `teams auth login --device-code` without `--client-id`.
- Default delegated tenant authority to `organizations` unless config or CLI specifies a tenant.
- Keep `--client-id` and `--tenant-id` as overrides.
- Add config fields for `auth_app = "oso" | "byo"` and optional `tenant_id`.
- Add `teams auth consent-url` to print the tenant admin consent URL for the active app.
- Add `teams auth doctor` to validate profile config, current token type, expiry, tenant, and scopes.
- Make PKCE loopback port configurable or dynamic so `localhost:8400` conflicts do not block login.

Tests:

- CLI tests for login help/defaults.
- Unit tests for app/client resolution precedence.
- Unit tests for consent URL generation.
- Mocked auth endpoint tests for device code error handling.
- Manual live delegated login against OSO test tenant.

Acceptance criteria:

- A new user can authenticate with only:
  ```bash
  teams auth login --device-code
  ```
- Existing BYO flows still work.
- No secret is embedded in the binary.

### Phase 2: Permission Guardrails and Messaging Safety

Deliverables:

- Parse access token claims locally to distinguish delegated `scp` tokens from app-only `roles` tokens.
- Add command capability checks before high-risk operations.
- Block or clearly fail `message send`, `message reply`, chat send, and channel send when the token is app-only.
- Update error text to explain that normal Graph message posting requires delegated auth or bot mode.
- Add a command-to-permission matrix in docs and man pages.
- Split default scopes into presets, for example:
  - `basic-read`
  - `messaging`
  - `admin`
  - `files`
  - `presence`

Tests:

- Unit tests for JWT claim parsing using synthetic unsigned test tokens.
- CLI tests for capability error messages.
- Mocked Graph tests to ensure app-only send failures map to auth/permission errors.

Acceptance criteria:

- The CLI no longer implies client credentials can post normal Teams messages.
- Agents get deterministic errors with next-step remediation.

### Phase 3: Enterprise BYO App Onboarding

Deliverables:

- Add `teams auth setup-byo` or documented config workflow.
- Generate an Entra app setup checklist for admins.
- Add `teams auth consent-url --client-id ... --tenant-id ...`.
- Document exact redirect URI, public-client setting, and delegated permissions.
- Document how to revoke consent and remove stored tokens.

Tests:

- CLI tests for BYO config commands.
- Manual live login with a test customer-owned app.

Acceptance criteria:

- A locked-down tenant can use the CLI without OSO's app registration.
- The admin guide can be followed without support intervention.

### Phase 4: Bot MVP for Unattended Posting

Deliverables:

- Decide repo layout:
  - Preferred: separate `oso-teams-bot` service repo or `apps/bot-service/` because Bot Framework and Teams app tooling are strongest in TypeScript/C#.
  - Keep this Rust CLI focused on local Graph operations and bot client commands.
- Create Teams app manifest, icons, and bot registration docs.
- Build bot service with:
  - `/api/messages` Bot Framework endpoint.
  - Conversation reference capture on install/conversation update.
  - Proactive send endpoint.
  - Tenant/team/chat/channel mapping.
  - Audit logging.
- Add CLI commands:
  - `teams bot status`
  - `teams bot conversations list`
  - `teams bot send --conversation <id> --text <text>`
  - `teams message send --as bot ...` as a convenience wrapper once stable.

Tests:

- Unit tests for conversation reference storage.
- Contract tests for proactive send payloads.
- Live test in OSO Teams test team:
  - install bot
  - capture conversation reference
  - send proactive message
  - verify message appears as bot

Acceptance criteria:

- Bot can post to a dedicated test team/channel without a signed-in Graph user.
- The product clearly distinguishes bot posting from user-delegated posting.

### Phase 5: Bot Installation and Admin Consent Automation

Deliverables:

- Publish app package path:
  - tenant app catalog first
  - Teams Store later, after validation
- Add admin consent docs for bot/service application permissions.
- Add optional Graph-backed installation flow for teams/users/chats where permitted.
- Store tenant installation state.
- Add uninstall and cleanup workflows.

Tests:

- Live test app installation with admin-consented Graph app permissions.
- Negative tests for missing admin consent and blocked Teams app policy.
- Cleanup tests for uninstall and conversation reference invalidation.

Acceptance criteria:

- Enterprise admin can approve and install the bot using a documented flow.
- CLI can report whether bot posting is available for a target team/chat.

### Phase 6: Commercial Hardening

Deliverables:

- Security whitepaper:
  - token storage
  - Graph scopes
  - bot data retention
  - audit logging
  - tenant isolation
- Admin onboarding guide.
- Agency quickstart.
- Enterprise BYO app guide.
- Bot installation guide.
- Support matrix for:
  - user-delegated Graph
  - BYO delegated Graph
  - bot proactive posting
  - client credentials limited operations
- Windows-specific auth validation:
  - Windows Credential Manager token storage
  - browser PKCE loopback
  - device code
  - PowerShell examples

Tests:

- CI on Ubuntu, macOS, and Windows.
- Live smoke tests in a dedicated OSO tenant.
- Live negative tests for app-only Graph message send.
- Manual Windows install/login/post test before release.

Acceptance criteria:

- No command documentation claims unsupported Graph behavior.
- Every advertised auth mode has a tested happy path and documented limitations.
- Release notes state which features are delegated-only, app-only, or bot-only.

## Documentation Work

Update these files as implementation lands:

- `README.md`
- `AGENTS.md`
- `docs/man/teams.1`
- `docs/man/teams-config.5`
- `docs/man/teams-agent-contract.7`
- `docs/teams-cli-prd.md`
- New admin guides under `docs/admin/`

Required documentation tables:

- Auth mode comparison.
- Command-to-permission matrix.
- Delegated vs application support per command.
- Admin consent scope list.
- Troubleshooting for consent blocked, missing scope, expired token, and app-only message send.

## Live Validation Plan

Use a dedicated OSO Teams test tenant/team/channel before any client tenant.

### Delegated CLI Smoke

1. `teams auth login --device-code`
2. `teams auth status --output json`
3. `teams user me --output json`
4. `teams team list --output json`
5. `teams channel list --team <test-team-id> --output json`
6. `teams message send --team <test-team-id> --channel <test-channel-id> --text "OSO delegated smoke test"`
7. `teams message list --team <test-team-id> --channel <test-channel-id> --output json`

Pass condition: message appears as signed-in user and JSON output remains parseable.

### App-Only Negative Smoke

1. Authenticate with client credentials against a test app.
2. Attempt a normal channel message send.
3. Verify the CLI blocks it locally or returns a clear unsupported-auth error.

Pass condition: no confusing Graph failure leaks to the user; remediation says to use delegated or bot mode.

### Bot Smoke

1. Install OSO bot in the test team/channel.
2. Confirm conversation reference captured.
3. Send proactive message through bot service.
4. Verify message appears as bot.
5. Remove the bot and confirm subsequent send fails clearly.

Pass condition: unattended posting works without a signed-in Graph user and cleanup is documented.

### Windows Smoke

1. Install release artifact on Windows.
2. Run device-code login.
3. Confirm token is stored in Windows Credential Manager.
4. Send delegated smoke message.
5. Logout and verify token removal.

Pass condition: same behavior as macOS/Linux, no path or keyring failures.

## Risks and Mitigations

| Risk | Impact | Mitigation |
| --- | --- | --- |
| Customers expect app-only Graph to post messages | Failed deployments | Guardrails, docs, and bot mode |
| Tenant blocks user consent | Login failure | Admin consent URL, verified publisher, BYO app option |
| Tenant blocks third-party multi-tenant apps | Login failure | BYO app guide |
| Bot install blocked by Teams app policy | Bot posting unavailable | Admin guide and status diagnostics |
| Current fixed PKCE loopback port is busy | Login failure | Dynamic/configurable loopback port |
| Default scopes are too broad for enterprise review | Consent friction | Scope presets and command permission matrix |
| Bot service stores sensitive tenant mapping | Security review blocker | Minimize stored data, encrypt at rest, audit, document retention |

## Immediate Next Tasks

1. Register OSO CLI public client app in Entra. Completed for app `fba1b5d0-fdd0-4fe2-9729-9ccdc38f9595`.
2. Configure verified publisher for the OSO tenant/app. Still required before commercial rollout.
3. Add default OSO client ID support to the CLI. In progress.
4. Add `auth consent-url` and `auth doctor`.
5. Add token claim parsing and app-only messaging guardrails.
6. Update README/man pages with the auth mode matrix.
7. Run delegated live smoke test in OSO Teams.
8. Decide OSO-hosted vs customer-hosted bot service.
9. Create Teams bot app manifest and bot service MVP.
10. Run bot proactive message smoke test in OSO Teams.
