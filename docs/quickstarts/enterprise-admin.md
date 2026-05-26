# Enterprise admin consent quickstart

This guide is for tenant admins evaluating `teams`.

## 1. Decide OSO app or BYO app

Use the OSO app when you are comfortable granting consent to OSO's multi-tenant public client:

```text
Client ID: fba1b5d0-fdd0-4fe2-9729-9ccdc38f9595
```

Use BYO when your policy requires an app registration owned by your tenant.

## 2. Generate admin consent URL

```bash
teams auth consent-url --tenant-id <tenant-id-or-domain> --output json
```

Open the returned `admin_consent_url` as an admin.

## 3. Validate login

```bash
teams auth login --device-code --tenant-id <tenant-id-or-domain>
teams auth doctor --output json
```

Expected:

- `authenticated: true`
- `auth_app: "oso"` for the default app
- token `auth_type: "delegated"`
- tenant ID matches the expected tenant

## 4. Run read-only checks

```bash
teams user me --output json
teams team list --output json
teams chat list --page-size 10 --output json
```

## 5. Run a controlled write check

Use a dedicated test team/channel.

```bash
teams message send \
  --team "$TEAM_ID" \
  --channel "$CHANNEL_ID" \
  --body "teams-cli enterprise smoke test" \
  --output json
```

## 6. Review audit and permissions

Confirm:

- The message appears as the signed-in user.
- Consent was granted to the expected app.
- The permission list matches the approved scope.
- No client credentials were used for normal message sending.

## 7. BYO app checklist

For a customer-owned app:

- Create an Entra app registration.
- Configure public client redirect URI `http://localhost:8400/callback`.
- Enable public client flows.
- Add delegated Graph permissions required by your chosen command set.
- Grant admin consent.
- Configure the CLI profile with `auth_app = "byo"`.
