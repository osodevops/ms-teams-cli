# teams-cli documentation

`teams` is a Microsoft Teams command line tool built for AI agents, scripts, and operators that need deterministic access to Microsoft Graph. It is not a Teams bot framework. It is a subprocess-friendly CLI that can read Teams data, send messages as a delegated user, manage Teams resources, and emit JSON envelopes that agents can parse reliably.

## Start here

- [Quickstarts](quickstarts/README.md): short paths for local setup, agents, agencies, enterprises, and Windows.
- [Authentication guide](auth.md): commercial auth direction, OSO app defaults, BYO app setup, consent, and app-only limits.
- [Command reference](command-reference.md): practical syntax for every command group.
- [Examples](examples.md): copyable shell and PowerShell examples for common workflows.
- [Use cases](use-cases.md): agency, consultancy, enterprise, support, DevOps, and agent patterns.
- [Troubleshooting](troubleshooting.md): auth, Graph permissions, Windows keyring, rate limits, and stale chats.
- [FAQ](faq.md): product and auth questions likely to come up during customer rollout.
- [Release readiness](release-readiness.md): what must pass before internal, RC, and commercial release.

## Reference docs

- [Product requirements](teams-cli-prd.md)
- [Auth implementation plan](auth-implementation-plan.md)
- [Man pages](man/)
- [Repository agent guide](../AGENTS.md)

## Commercial auth summary

The recommended commercial model is delegated Microsoft Graph auth through OSO's multi-tenant public client app. That means a user signs in once with browser or device-code flow, tenant admins can grant consent once, and messages appear as the signed-in user.

For locked-down customers, support `auth_app = "byo"` with a customer-owned Entra app registration. For unattended posting without a human user, the right future path is a Teams app/bot proactive messaging mode, not client credentials pretending to be a user.

Current OSO app client ID:

```text
fba1b5d0-fdd0-4fe2-9729-9ccdc38f9595
```

Do not place client secrets, refresh tokens, access tokens, private chat contents, customer tenant IDs, or customer user data in docs, examples, screenshots, issues, or test fixtures.

## Output contract

When stdout is piped or called by an agent, the CLI defaults to JSON:

```json
{
  "success": true,
  "data": {},
  "metadata": {
    "request_id": "uuid",
    "timestamp": "2026-05-26T00:00:00Z",
    "api_version": "v1.0",
    "duration_ms": 123
  }
}
```

Errors use the same envelope shape with `success: false` and a stable error code. See [command reference](command-reference.md#exit-codes) and `teams-agent-contract(7)`.

## Official Microsoft references

The auth and Teams messaging guidance in these docs is based on Microsoft Graph and Microsoft identity platform documentation:

- [Microsoft Graph authentication and authorization basics](https://learn.microsoft.com/en-us/graph/auth/auth-concepts)
- [Send message in a chat](https://learn.microsoft.com/en-us/graph/api/chat-post-messages?view=graph-rest-1.0)
- [Send replies to a message in a channel](https://learn.microsoft.com/en-us/graph/api/chatmessage-post-replies?view=graph-rest-1.0)
- [List joinedTeams](https://learn.microsoft.com/en-us/graph/api/user-list-joinedteams?view=graph-rest-1.0)
- [Mark your app as publisher verified](https://learn.microsoft.com/en-us/entra/identity-platform/mark-app-as-publisher-verified)
