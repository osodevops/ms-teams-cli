# Windows quickstart

This guide uses PowerShell.

## 1. Check the binary

```powershell
teams --help
teams --version
```

From source:

```powershell
cargo build
.\target\debug\teams.exe --help
```

## 2. Sign in

Device-code flow is recommended for first validation:

```powershell
teams auth login --device-code
```

Then:

```powershell
teams auth doctor --output json | ConvertFrom-Json
```

## 3. Config path

```powershell
teams config path --output json | ConvertFrom-Json
```

Default Windows path:

```text
%APPDATA%\teams-cli\config.toml
```

## 4. Environment variables

PowerShell session:

```powershell
$env:TEAMS_CLI_TENANT_ID = "<tenant-id>"
$env:TEAMS_CLI_CLIENT_ID = "<client-id>"
teams auth login --device-code
```

Persistent user variable:

```powershell
[Environment]::SetEnvironmentVariable("TEAMS_CLI_TENANT_ID", "<tenant-id>", "User")
```

## 5. Read-only smoke test

```powershell
teams user me --output json | ConvertFrom-Json
teams chat list --page-size 10 --output json | ConvertFrom-Json
```

## 6. Send to a test chat

```powershell
teams message send --chat $env:TEAMS_TEST_CHAT_ID --body "teams-cli Windows smoke test" --output json
```

## 7. Credential Manager

Tokens are stored in Windows Credential Manager through the Rust `keyring` crate. If auth commands hang or fail:

- Run PowerShell as the same Windows user that performed login.
- Check Credential Manager for `teams-cli` entries.
- Try `teams auth logout --all`, then log in again.
- Do not set `TEAMS_CLI_DISABLE_KEYRING=1` for real use.

`TEAMS_CLI_DISABLE_KEYRING=1` is only for tests that must avoid real OS credential storage.
