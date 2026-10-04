use assert_cmd::Command;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use predicates::prelude::*;
use std::fs;
use std::io::Read;
use std::process::Stdio;

fn teams_process() -> std::process::Command {
    let mut cmd = std::process::Command::new(assert_cmd::cargo::cargo_bin!("teams"));
    cmd.env("TEAMS_CLI_DISABLE_KEYRING", "1");
    // Isolate tests from the developer's real environment: a configured
    // profile or exported credentials would otherwise change command output.
    // dirs resolves via $HOME on macOS and $XDG_CONFIG_HOME on Linux; Windows
    // uses the Known Folder API and is unaffected by these overrides.
    cmd.env("HOME", env!("CARGO_TARGET_TMPDIR"));
    cmd.env("XDG_CONFIG_HOME", env!("CARGO_TARGET_TMPDIR"));
    cmd.env_remove("TEAMS_CLI_PROFILE");
    cmd.env_remove("TEAMS_CLI_SCOPES");
    cmd.env_remove("TEAMS_CLI_CLIENT_ID");
    cmd.env_remove("TEAMS_CLI_CLIENT_SECRET");
    cmd.env_remove("TEAMS_CLI_TENANT_ID");
    cmd.env_remove("TEAMS_CLI_ACCESS_TOKEN");
    cmd.env_remove("TEAMS_CLI_TOKEN_STORE");
    cmd
}

fn teams() -> Command {
    Command::from_std(teams_process())
}

/// `--order-by` is a clap value enum, so a wrong value fails at parse time,
/// before a token is resolved; the message has to name the value that exists.
#[test]
fn chat_list_rejects_an_unknown_order_before_it_needs_credentials() {
    teams()
        .args(["chat", "list", "--order-by", "updated"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("activity"));
}

/// The accepted orders are listed in `--help` (and so in shell completions).
#[test]
fn chat_list_help_lists_the_orders() {
    teams()
        .args(["chat", "list", "--help"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("--order-by <ORDER_BY>").and(
                predicate::str::contains("activity")
                    .and(predicate::str::contains("Newest message first")),
            ),
        );
}

/// The expiration check is a clap `value_parser`, so it has to reject the value before anything
/// resolves a token or opens a connection. Testing the parser alone would not notice the
/// attribute being dropped.
#[test]
fn presence_set_rejects_a_bad_expiration_before_it_needs_credentials() {
    teams()
        .args([
            "presence",
            "set",
            "--availability",
            "Available",
            "--activity",
            "Available",
            "--expiration",
            "1h",
        ])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("is not an ISO 8601 duration"));

    teams()
        .args([
            "presence",
            "set",
            "--availability",
            "Available",
            "--activity",
            "Available",
            "--expiration",
            "PT10H",
        ])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("PT5M to PT4H"));
}

/// The activity is derived from the availability, so the only way to send a pair Graph rejects
/// is for the value parser to be dropped from the attribute; the checks below would notice.
#[test]
fn presence_set_preferred_rejects_bad_values_before_it_needs_credentials() {
    teams()
        .args(["presence", "set-preferred", "--availability", "InACall"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains(
            "Available, Busy, DoNotDisturb, BeRightBack, Away, Offline",
        ));

    teams()
        .args([
            "presence",
            "set-preferred",
            "--availability",
            "Away",
            "--expiration",
            "8h",
        ])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("is not an ISO 8601 duration"));

    teams()
        .args([
            "presence",
            "set-preferred",
            "--availability",
            "Away",
            "--expiration",
            "P1DT",
        ])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("is not an ISO 8601 duration"));
}

#[test]
fn preferred_presence_writes_reject_app_only_auth_before_graph() {
    let payload = serde_json::json!({ "roles": ["Presence.ReadWrite.All"] });
    let token = format!(
        "header.{}.signature",
        URL_SAFE_NO_PAD.encode(payload.to_string())
    );

    for args in [
        vec!["presence", "set-preferred", "--availability", "Away"],
        vec!["presence", "clear-preferred"],
    ] {
        teams()
            .args(args)
            .env("TEAMS_CLI_ACCESS_TOKEN", &token)
            .assert()
            .code(4)
            .stdout(
                predicate::str::contains("\"code\": \"PERMISSION_DENIED\"").and(
                    predicate::str::contains("requires delegated Microsoft Graph auth"),
                ),
            );
    }
}

#[test]
fn help_flag_works() {
    teams().arg("--help").assert().success().stdout(
        predicate::str::contains("Microsoft Teams CLI")
            .and(predicate::str::contains("auth"))
            .and(predicate::str::contains("user"))
            .and(predicate::str::contains("config"))
            .and(predicate::str::contains("team"))
            .and(predicate::str::contains("channel"))
            .and(predicate::str::contains("message"))
            .and(predicate::str::contains("chat"))
            .and(predicate::str::contains("presence"))
            .and(predicate::str::contains("search"))
            .and(predicate::str::contains("tag"))
            .and(predicate::str::contains("meeting"))
            .and(predicate::str::contains("notify"))
            .and(predicate::str::contains("app"))
            .and(predicate::str::contains("tab"))
            .and(predicate::str::contains("file"))
            .and(predicate::str::contains("subscribe"))
            .and(predicate::str::contains("listen")),
    );
}

#[test]
fn auth_status_without_login_exits_nonzero() {
    teams().args(["auth", "status"]).assert().code(1);
}

#[test]
fn auth_list_without_keyring_reports_no_profiles() {
    teams()
        .args(["auth", "list", "--output", "json"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("\"profiles\": []")
                .and(predicate::str::contains("\"active\": \"default\"")),
        );
}

#[test]
fn auth_consent_url_uses_oso_default_client_id() {
    teams()
        .args(["auth", "consent-url", "--output", "json"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("fba1b5d0-fdd0-4fe2-9729-9ccdc38f9595")
                .and(predicate::str::contains("v2.0/adminconsent"))
                .and(predicate::str::contains("scope="))
                .and(predicate::str::contains("redirect_uri="))
                .and(predicate::str::contains("ChatMessage.Send"))
                .and(predicate::str::contains("organizations"))
                .and(predicate::str::contains("ChannelMessage.Read.All").not()),
        );
}

#[test]
fn auth_login_help_documents_scopes_flag_and_env() {
    teams()
        .args(["auth", "login", "--help"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("--scopes <SCOPES>")
                .and(predicate::str::contains("TEAMS_CLI_SCOPES")),
        );
}

#[test]
fn auth_help_shows_refresh_subcommand() {
    teams()
        .args(["auth", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("refresh"));
}

#[test]
fn auth_refresh_help_documents_scopes_flag_and_env() {
    teams()
        .args(["auth", "refresh", "--help"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("--scopes <SCOPES>")
                .and(predicate::str::contains("TEAMS_CLI_SCOPES")),
        );
}

#[test]
fn auth_refresh_without_login_fails_with_auth_error() {
    teams()
        .args(["auth", "refresh", "--output", "json"])
        .assert()
        .code(3)
        .stdout(predicate::str::contains("auth login"));
}

#[test]
fn auth_consent_url_accepts_explicit_scopes() {
    teams()
        .args([
            "auth",
            "consent-url",
            "--scopes",
            "User.Read People.Read",
            "--output",
            "json",
        ])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("People.Read").and(predicate::str::contains("offline_access")),
        );
}

#[test]
fn auth_consent_url_uses_profile_scopes_override() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    fs::write(
        &path,
        r#"
[profiles.customer]
auth_app = "byo"
client_id = "11111111-1111-1111-1111-111111111111"
tenant_id = "22222222-2222-2222-2222-222222222222"
scopes = "User.Read People.Read TeamMember.Read.All offline_access"
"#,
    )
    .unwrap();

    teams()
        .args([
            "--config",
            path.to_str().unwrap(),
            "--profile",
            "customer",
            "auth",
            "consent-url",
            "--output",
            "json",
        ])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("People.Read")
                .and(predicate::str::contains("TeamMember.Read.All"))
                .and(predicate::str::contains(
                    "11111111-1111-1111-1111-111111111111",
                )),
        );
}

fn write_customer_profile_config(dir: &tempfile::TempDir) -> std::path::PathBuf {
    let path = dir.path().join("config.toml");
    fs::write(
        &path,
        r#"
[profiles.customer]
auth_app = "byo"
client_id = "11111111-1111-1111-1111-111111111111"
tenant_id = "22222222-2222-2222-2222-222222222222"
"#,
    )
    .unwrap();
    path
}

#[test]
fn profile_env_var_selects_profile() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_customer_profile_config(&dir);

    teams()
        .env("TEAMS_CLI_PROFILE", "customer")
        .args([
            "--config",
            path.to_str().unwrap(),
            "auth",
            "consent-url",
            "--output",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "11111111-1111-1111-1111-111111111111",
        ));
}

#[test]
fn profile_flag_beats_profile_env_var() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_customer_profile_config(&dir);

    // The env var points at the BYO profile; the flag selects the built-in
    // default profile, so the OSO public client id must win.
    teams()
        .env("TEAMS_CLI_PROFILE", "customer")
        .args([
            "--config",
            path.to_str().unwrap(),
            "--profile",
            "default",
            "auth",
            "consent-url",
            "--output",
            "json",
        ])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("fba1b5d0-fdd0-4fe2-9729-9ccdc38f9595")
                .and(predicate::str::contains("11111111-1111-1111-1111-111111111111").not()),
        );
}

#[test]
fn explicit_default_profile_ignores_config_default() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    fs::write(
        &path,
        r#"
[default]
profile = "customer"

[profiles.customer]
auth_app = "byo"
client_id = "11111111-1111-1111-1111-111111111111"
tenant_id = "22222222-2222-2222-2222-222222222222"
"#,
    )
    .unwrap();

    // Without the flag the config default applies; with an explicit
    // --profile default the profile named "default" must be addressable.
    teams()
        .args([
            "--config",
            path.to_str().unwrap(),
            "auth",
            "consent-url",
            "--output",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "11111111-1111-1111-1111-111111111111",
        ));

    teams()
        .args([
            "--config",
            path.to_str().unwrap(),
            "--profile",
            "default",
            "auth",
            "consent-url",
            "--output",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "fba1b5d0-fdd0-4fe2-9729-9ccdc38f9595",
        ));
}

#[test]
fn auth_doctor_reports_resolved_delegated_scopes() {
    teams()
        .args(["auth", "doctor", "--output", "json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"resolved_delegated_scopes\""));
}

#[test]
fn auth_doctor_reports_oso_default_without_login() {
    teams()
        .args(["auth", "doctor", "--output", "json"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("\"auth_app\": \"oso\"")
                .and(predicate::str::contains("\"authenticated\": false")),
        );
}

#[test]
fn auth_doctor_reports_token_audience() {
    let payload = serde_json::json!({
        "aud": "https://graph.microsoft.com",
        "tid": "tenant-1",
        "scp": "User.Read"
    });
    let token = format!(
        "header.{}.signature",
        URL_SAFE_NO_PAD.encode(payload.to_string())
    );

    teams()
        .args(["auth", "doctor", "--output", "json"])
        .env("TEAMS_CLI_ACCESS_TOKEN", token)
        .assert()
        .success()
        .stdout(
            predicate::str::contains("https://graph.microsoft.com")
                .and(predicate::str::contains("\"is_graph_audience\": true")),
        );
}

#[test]
fn completions_generates_output() {
    teams()
        .args(["completions", "bash"])
        .assert()
        .success()
        .stdout(predicate::str::contains("teams"));
}

#[test]
fn closed_stdout_pipe_does_not_panic() {
    let mut child = teams_process()
        .args(["completions", "bash"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    let mut stdout = child.stdout.take().unwrap();
    let mut first_byte = [0_u8; 1];
    stdout.read_exact(&mut first_byte).unwrap();
    drop(stdout);

    let output = child.wait_with_output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "status: {}; stderr: {stderr}",
        output.status
    );
    assert!(stderr.is_empty(), "unexpected stderr: {stderr}");
}

#[test]
fn unknown_subcommand_fails() {
    teams().arg("nonexistent").assert().failure();
}

#[test]
fn config_show_returns_valid_json_like_output() {
    teams()
        .args(["config", "show", "--output", "json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"success\": true"));
}

#[test]
fn config_init_respects_custom_config_path() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("custom-config.toml");
    let path_str = path.to_str().unwrap();

    let assert = teams()
        .args(["--config", path_str, "--output", "json", "config", "init"])
        .assert()
        .success();
    let stdout: serde_json::Value = serde_json::from_slice(&assert.get_output().stdout).unwrap();
    assert_eq!(stdout["data"]["path"].as_str(), Some(path_str));

    assert!(path.exists());
}

#[test]
fn config_set_preserves_numeric_value_types() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let path_str = path.to_str().unwrap();

    teams()
        .args([
            "--config",
            path_str,
            "--output",
            "json",
            "config",
            "set",
            "network.timeout",
            "60",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"success\": true"));

    teams()
        .args([
            "--config",
            path_str,
            "--output",
            "json",
            "config",
            "get",
            "network.timeout",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"data\": 60"));
}

#[test]
fn config_output_format_is_honored_without_cli_output_flag() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let path_str = path.to_str().unwrap();
    fs::write(&path, "[output]\nformat = \"plain\"\n").unwrap();

    teams()
        .args(["--config", path_str, "config", "path"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("path:").and(predicate::str::contains("\"success\"").not()),
        );
}

// --- Phase 2: Team subcommand tests ---

#[test]
fn user_help_shows_subcommands() {
    teams().args(["user", "--help"]).assert().success().stdout(
        predicate::str::contains("me")
            .and(predicate::str::contains("get"))
            .and(predicate::str::contains("list"))
            .and(predicate::str::contains("resolve")),
    );
}

#[test]
fn user_resolve_help_shows_max_chats() {
    teams()
        .args(["user", "resolve", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--max-chats <MAX_CHATS>"));
}

#[test]
fn team_help_shows_subcommands() {
    teams().args(["team", "--help"]).assert().success().stdout(
        predicate::str::contains("list")
            .and(predicate::str::contains("get"))
            .and(predicate::str::contains("create"))
            .and(predicate::str::contains("delete"))
            .and(predicate::str::contains("clone"))
            .and(predicate::str::contains("archive"))
            .and(predicate::str::contains("members")),
    );
}

#[test]
fn channel_help_shows_subcommands() {
    teams()
        .args(["channel", "--help"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("list")
                .and(predicate::str::contains("get"))
                .and(predicate::str::contains("create"))
                .and(predicate::str::contains("delete"))
                .and(predicate::str::contains("members")),
        );
}

#[test]
fn message_help_shows_subcommands() {
    teams()
        .args(["message", "--help"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("send")
                .and(predicate::str::contains("list"))
                .and(predicate::str::contains("get"))
                .and(predicate::str::contains("reply"))
                .and(predicate::str::contains("react"))
                .and(predicate::str::contains("pin"))
                .and(predicate::str::contains("delete"))
                .and(predicate::str::contains("undelete")),
        );
}

#[test]
fn message_send_help_advertises_repeatable_mention_flag() {
    teams()
        .args(["message", "send", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--mention <USER>"));
}

#[test]
fn message_send_help_advertises_subject_flag() {
    teams()
        .args(["message", "send", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--subject <SUBJECT>"));
}

#[test]
fn help_json_includes_message_subject_flag() {
    let result = teams().arg("--help-json").assert().success();
    let help: serde_json::Value = serde_json::from_slice(&result.get_output().stdout).unwrap();
    let message = help["commands"]
        .as_array()
        .unwrap()
        .iter()
        .find(|command| command["name"] == "message")
        .unwrap();
    let send = message["subcommands"]
        .as_array()
        .unwrap()
        .iter()
        .find(|command| command["name"] == "send")
        .unwrap();
    assert!(send["flags"]
        .as_array()
        .unwrap()
        .iter()
        .any(|flag| flag["name"] == "--subject"));
}

#[test]
fn message_send_rejects_subject_on_chat_messages() {
    teams()
        .args([
            "message",
            "send",
            "--chat",
            "19:chat@thread.v2",
            "--subject",
            "Release plan",
            "--body",
            "hi",
        ])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("--subject"));
}

#[test]
fn message_send_rejects_subject_without_a_channel() {
    teams()
        .args([
            "message",
            "send",
            "--subject",
            "Release plan",
            "--body",
            "hi",
        ])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("--subject"));
}

#[test]
fn message_send_rejects_quote_without_a_chat() {
    teams()
        .args([
            "message",
            "send",
            "--team",
            "team-1",
            "--channel",
            "19:channel@thread.tacv2",
            "--quote",
            "1790330813814",
            "--body",
            "hi",
        ])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("--quote"));
}

/// `--quote` repeats to quote several chat messages in one reply; with
/// credentials absent the command gets past parsing to the auth error.
#[test]
fn message_send_accepts_repeated_quotes_before_authentication() {
    teams()
        .args([
            "message",
            "send",
            "--chat",
            "19:chat@thread.v2",
            "--quote",
            "1790667379654",
            "--quote",
            "1790667031882",
            "--body",
            "hi",
            "--output",
            "json",
        ])
        .assert()
        .code(3)
        .stdout(predicate::str::contains("auth login"));
}

#[test]
fn message_send_rejects_quote_with_both_chat_and_channel_targets() {
    teams()
        .args([
            "message",
            "send",
            "--chat",
            "19:chat@thread.v2",
            "--team",
            "team-1",
            "--channel",
            "19:channel@thread.tacv2",
            "--quote",
            "1790330813814",
            "--body",
            "hi",
        ])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("--quote"));
}

#[test]
fn message_reply_help_advertises_repeatable_mention_flag() {
    teams()
        .args(["message", "reply", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--mention <USER>"));
}

#[test]
fn message_list_help_advertises_thread_replies_flag() {
    teams()
        .args(["message", "list", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--message-id <MESSAGE_ID>"));
}

#[test]
fn message_list_message_id_requires_channel() {
    teams()
        .args([
            "message",
            "list",
            "--team",
            "team-id",
            "--message-id",
            "1234",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--channel"));
}

#[test]
fn message_list_rejects_message_id_with_chat() {
    teams()
        .args([
            "message",
            "list",
            "--chat",
            "19:abc@thread.v2",
            "--channel",
            "channel-id",
            "--message-id",
            "1234",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));
}

#[test]
fn documented_chat_attachment_command_parses_before_authentication() {
    teams()
        .args([
            "message",
            "send",
            "--chat",
            "chat-id",
            "--attach",
            "example.txt",
            "--output",
            "json",
        ])
        .assert()
        .code(3)
        .stdout(predicate::str::contains("auth login"));
}

#[test]
fn message_delete_and_undelete_accept_chat_and_reply_targets() {
    for sub in ["delete", "undelete"] {
        teams()
            .args(["message", sub, "--help"])
            .assert()
            .success()
            .stdout(
                predicate::str::contains("--chat <CHAT>")
                    .and(predicate::str::contains("--reply <REPLY>"))
                    .and(predicate::str::contains("--message <MESSAGE>")),
            );
    }
}

/// Deletion must be confirmed explicitly. Without `--yes` the command is
/// refused before any token is resolved or request is sent, so the exit code
/// is 2 (not the auth error a bare environment would otherwise produce) and
/// the refusal arrives in the normal error envelope.
#[test]
fn message_delete_requires_yes() {
    teams()
        .args([
            "message",
            "delete",
            "--chat",
            "19:abc@thread.v2",
            "1700000000000",
            "--output",
            "json",
        ])
        .assert()
        .code(2)
        .stdout(
            predicate::str::contains("\"success\": false")
                .and(predicate::str::contains("INVALID_INPUT"))
                .and(predicate::str::contains("--yes")),
        );

    teams()
        .args([
            "message",
            "delete",
            "--chat",
            "19:abc@thread.v2",
            "1700000000000",
            "--output",
            "human",
        ])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("--yes"));
}

#[test]
fn message_delete_rejects_incomplete_or_mixed_targets() {
    teams()
        .args(["message", "delete", "--team", "team-id", "1", "--yes"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("--channel"));

    teams()
        .args([
            "message",
            "delete",
            "--chat",
            "19:abc@thread.v2",
            "--reply",
            "2",
            "1",
            "--yes",
        ])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("cannot be used with"));

    teams()
        .args(["message", "undelete", "--channel", "channel-id", "1"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("--team"));
}

#[test]
fn message_documented_flags_are_available() {
    teams()
        .args(["message", "get", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--message <MESSAGE>"));

    teams()
        .args(["message", "react", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--reaction <REACTION>"));

    teams()
        .args(["message", "unpin", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "--pinned-message-id <PINNED_MESSAGE_ID>",
        ));
}

#[test]
fn message_reactions_accept_chat() {
    for sub in ["react", "unreact"] {
        teams()
            .args(["message", sub, "--help"])
            .assert()
            .success()
            .stdout(
                predicate::str::contains("--chat <CHAT>").and(predicate::str::contains("emoji")),
            );
    }
}

#[test]
fn message_react_rejects_incomplete_target() {
    for args in [
        vec!["message", "react", "--message-id", "1", "eyes"],
        vec![
            "message",
            "react",
            "--team",
            "team-id",
            "--message-id",
            "1",
            "eyes",
        ],
        vec![
            "message",
            "unreact",
            "--channel",
            "channel-id",
            "--message-id",
            "1",
            "eyes",
        ],
    ] {
        teams()
            .args(&args)
            .assert()
            .code(2)
            .stderr(predicate::str::contains("required"));
    }
}

#[test]
fn message_react_rejects_chat_with_team() {
    teams()
        .args([
            "message",
            "react",
            "--chat",
            "19:abc@thread.v2",
            "--team",
            "team-id",
            "--message-id",
            "1",
            "eyes",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));
}

#[test]
fn chat_help_shows_subcommands() {
    teams().args(["chat", "--help"]).assert().success().stdout(
        predicate::str::contains("list")
            .and(predicate::str::contains("get"))
            .and(predicate::str::contains("create"))
            .and(predicate::str::contains("hide"))
            .and(predicate::str::contains("unhide"))
            .and(predicate::str::contains("members")),
    );
}

#[test]
fn team_unknown_subcommand_fails() {
    teams().args(["team", "nonexistent"]).assert().failure();
}

#[test]
fn channel_unknown_subcommand_fails() {
    teams().args(["channel", "nonexistent"]).assert().failure();
}

#[test]
fn message_unknown_subcommand_fails() {
    teams().args(["message", "nonexistent"]).assert().failure();
}

#[test]
fn chat_unknown_subcommand_fails() {
    teams().args(["chat", "nonexistent"]).assert().failure();
}

// --- Phase 3: Presence & Search subcommand tests ---

#[test]
fn presence_help_shows_subcommands() {
    teams()
        .args(["presence", "--help"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("get")
                .and(predicate::str::contains("set"))
                .and(predicate::str::contains("set-preferred"))
                .and(predicate::str::contains("clear"))
                .and(predicate::str::contains("clear-preferred"))
                .and(predicate::str::contains("status")),
        );
}

#[test]
fn presence_documented_batch_command_is_available() {
    teams()
        .args(["presence", "get-batch", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--user-ids <USER_IDS>"));
}

#[test]
fn search_help_shows_subcommands() {
    teams()
        .args(["search", "--help"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("messages")
                .and(predicate::str::contains("users"))
                .and(predicate::str::contains("teams")),
        );
}

#[test]
fn search_documented_query_flag_is_available() {
    teams()
        .args(["search", "messages", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--query <QUERY>"));
}

#[test]
fn presence_unknown_subcommand_fails() {
    teams().args(["presence", "nonexistent"]).assert().failure();
}

#[test]
fn search_unknown_subcommand_fails() {
    teams().args(["search", "nonexistent"]).assert().failure();
}

// --- Phase 4: Tags, Meetings, Notifications, Apps, Tabs, Files ---

#[test]
fn tag_help_shows_subcommands() {
    teams().args(["tag", "--help"]).assert().success().stdout(
        predicate::str::contains("list")
            .and(predicate::str::contains("get"))
            .and(predicate::str::contains("create"))
            .and(predicate::str::contains("delete"))
            .and(predicate::str::contains("add-member"))
            .and(predicate::str::contains("remove-member")),
    );
}

#[test]
fn meeting_help_shows_subcommands() {
    teams()
        .args(["meeting", "--help"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("list")
                .and(predicate::str::contains("get"))
                .and(predicate::str::contains("create"))
                .and(predicate::str::contains("delete"))
                .and(predicate::str::contains("join-url"))
                .and(predicate::str::contains("attendance")),
        );
}

#[test]
fn notify_help_shows_subcommands() {
    teams()
        .args(["notify", "--help"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("send")
                .and(predicate::str::contains("send-to-team"))
                .and(predicate::str::contains("send-to-chat")),
        );
}

#[test]
fn app_help_shows_subcommands() {
    teams().args(["app", "--help"]).assert().success().stdout(
        predicate::str::contains("list")
            .and(predicate::str::contains("install"))
            .and(predicate::str::contains("uninstall")),
    );
}

#[test]
fn tab_help_shows_subcommands() {
    teams().args(["tab", "--help"]).assert().success().stdout(
        predicate::str::contains("list")
            .and(predicate::str::contains("create"))
            .and(predicate::str::contains("delete")),
    );
}

#[test]
fn file_help_shows_subcommands() {
    teams().args(["file", "--help"]).assert().success().stdout(
        predicate::str::contains("list")
            .and(predicate::str::contains("get"))
            .and(predicate::str::contains("upload"))
            .and(predicate::str::contains("download"))
            .and(predicate::str::contains("delete"))
            .and(predicate::str::contains("share")),
    );
}

#[test]
fn file_download_uses_path_without_shadowing_global_output() {
    teams()
        .args(["file", "download", "--help"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("--path <PATH>")
                .and(predicate::str::contains("-o, --output <OUTPUT>")),
        );
}

#[test]
fn tag_unknown_subcommand_fails() {
    teams().args(["tag", "nonexistent"]).assert().failure();
}

#[test]
fn meeting_unknown_subcommand_fails() {
    teams().args(["meeting", "nonexistent"]).assert().failure();
}

#[test]
fn notify_unknown_subcommand_fails() {
    teams().args(["notify", "nonexistent"]).assert().failure();
}

#[test]
fn app_unknown_subcommand_fails() {
    teams().args(["app", "nonexistent"]).assert().failure();
}

#[test]
fn tab_unknown_subcommand_fails() {
    teams().args(["tab", "nonexistent"]).assert().failure();
}

#[test]
fn file_unknown_subcommand_fails() {
    teams().args(["file", "nonexistent"]).assert().failure();
}

// --- Phase 5: Subscribe & Listen ---

#[test]
fn subscribe_help_shows_subcommands() {
    teams()
        .args(["subscribe", "--help"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("create")
                .and(predicate::str::contains("list"))
                .and(predicate::str::contains("renew"))
                .and(predicate::str::contains("delete")),
        );
}

#[test]
fn listen_help_shows_options() {
    teams()
        .args(["listen", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--port"));
}

#[test]
fn subscribe_unknown_subcommand_fails() {
    teams()
        .args(["subscribe", "nonexistent"])
        .assert()
        .failure();
}

#[test]
fn message_update_accepts_chat_target() {
    teams()
        .args(["message", "update", "--help"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("--chat")
                .and(predicate::str::contains("for chat messages"))
                .and(predicate::str::contains("for channel messages")),
        );
}

#[test]
fn message_update_without_a_target_is_rejected() {
    teams()
        .args(["message", "update", "1234", "--body", "x"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--chat"));
}

#[test]
fn message_update_rejects_mixing_chat_and_channel_targets() {
    teams()
        .args([
            "message",
            "update",
            "1234",
            "--body",
            "x",
            "--chat",
            "19:abc@thread.v2",
            "--team",
            "team-id",
            "--channel",
            "channel-id",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));
}

#[test]
fn message_update_requires_channel_alongside_team() {
    teams()
        .args([
            "message", "update", "1234", "--body", "x", "--team", "team-id",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--channel"));
}

/// A mistyped `TEAMS_CLI_TOKEN_STORE` is refused before any command runs, in
/// the error envelope with exit code 2. Token reads treat every store error as
/// "not signed in", so without the check a typo would read as exit code 3 and
/// send the caller to log in again.
#[test]
fn unknown_token_store_is_invalid_input_not_an_auth_error() {
    for args in [["chat", "list"], ["auth", "list"]] {
        teams()
            .env("TEAMS_CLI_TOKEN_STORE", "flie")
            .args(args)
            .args(["--output", "json"])
            .assert()
            .code(2)
            .stdout(
                predicate::str::contains("INVALID_INPUT")
                    .and(predicate::str::contains("TEAMS_CLI_TOKEN_STORE=flie"))
                    .and(predicate::str::contains("`file`")),
            );
    }
}

/// With `TEAMS_CLI_TOKEN_STORE=file` the CLI finds a profile's token under the
/// config directory's `tokens/` and `auth logout` removes it, without touching
/// the OS keyring. Not on Windows, whose config directory ignores `HOME`.
#[cfg(not(windows))]
#[test]
fn file_token_store_lists_and_logs_out_a_profile() {
    let home = tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR")).unwrap();
    let config_path = teams()
        .env("HOME", home.path())
        .env("XDG_CONFIG_HOME", home.path())
        .args(["config", "path", "--output", "json"])
        .output()
        .unwrap();
    let config_path: serde_json::Value = serde_json::from_slice(&config_path.stdout).unwrap();
    let config_path = std::path::PathBuf::from(config_path["data"]["path"].as_str().unwrap());
    assert!(config_path.starts_with(home.path()));
    let tokens = config_path.parent().unwrap().join("tokens");
    fs::create_dir_all(&tokens).unwrap();
    fs::write(tokens.join("profile-index"), r#"["work"]"#).unwrap();
    fs::write(
        tokens.join("work.token"),
        r#"{"access_token":"not-a-jwt","token_type":"Bearer","profile":"work"}"#,
    )
    .unwrap();

    let file_store = || {
        let mut cmd = teams();
        cmd.env_remove("TEAMS_CLI_DISABLE_KEYRING")
            .env("TEAMS_CLI_TOKEN_STORE", "file")
            .env("HOME", home.path())
            .env("XDG_CONFIG_HOME", home.path());
        cmd
    };

    file_store()
        .args(["auth", "list", "--output", "json"])
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""name": "work""#));

    file_store()
        .args(["auth", "logout", "--profile", "work", "--output", "json"])
        .assert()
        .success();
    assert!(!tokens.join("work.token").exists());
    assert_eq!(
        fs::read_to_string(tokens.join("profile-index")).unwrap(),
        "[]"
    );
}

/// `--help` names the environment variable a client secret can come from, but
/// never prints the secret itself, which would put it in a terminal
/// scrollback, a pasted bug report or an agent transcript.
#[test]
fn login_help_does_not_print_the_client_secret() {
    teams()
        .env("TEAMS_CLI_CLIENT_SECRET", "sentinel-secret-value")
        .args(["auth", "login", "--help"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("TEAMS_CLI_CLIENT_SECRET")
                .and(predicate::str::contains("sentinel-secret-value").not()),
        );
}

/// The integration tests run the binary cargo built for them, so its storage
/// namespace follows the same rule as the crate: the build-time override if
/// one is set, otherwise the development namespace for a debug build.
fn expected_namespace() -> &'static str {
    match option_env!("TEAMS_CLI_BUILD_NAMESPACE") {
        Some(namespace) => namespace,
        None if cfg!(debug_assertions) => "teams-cli-dev",
        None => "teams-cli",
    }
}

/// `teams --version` names a non-release storage namespace, so a person or an
/// agent can tell which tokens and config a binary uses without opening the
/// keyring.
#[test]
fn version_reports_a_non_release_storage_namespace() {
    let namespace = expected_namespace();
    let version = format!("teams {}", env!("CARGO_PKG_VERSION"));
    let expected = if namespace == "teams-cli" {
        format!("{version}\n")
    } else {
        format!("{version} (storage namespace {namespace})\n")
    };
    teams().arg("--version").assert().success().stdout(expected);
}

/// `config path` reports the namespace, and the config file sits in a
/// directory named after it.
#[test]
fn config_path_reports_the_storage_namespace() {
    let namespace = expected_namespace();
    let result = teams()
        .args(["config", "path", "--output", "json"])
        .assert()
        .success();
    let data: serde_json::Value = serde_json::from_slice(&result.get_output().stdout).unwrap();
    assert_eq!(data["data"]["namespace"], namespace);
    let path = std::path::PathBuf::from(data["data"]["path"].as_str().unwrap());
    assert_eq!(path.parent().unwrap().file_name().unwrap(), namespace);
}

/// A command that finds no token names the storage namespace it looked in when
/// that is not the release one, so a developer whose debug build cannot see
/// the installed release's login is told why instead of only "log in".
#[test]
fn missing_token_names_a_non_release_storage_namespace() {
    let namespace = expected_namespace();
    let result = teams()
        .args(["user", "me", "--output", "json"])
        .assert()
        .code(3);
    let output: serde_json::Value = serde_json::from_slice(&result.get_output().stdout).unwrap();
    let message = output["error"]["message"].as_str().unwrap();
    assert!(message.contains("Not authenticated."), "{message}");
    assert_eq!(
        message.contains(&format!("storage namespace `{namespace}`")),
        namespace != "teams-cli",
        "{message}"
    );
}

/// The client and tenant IDs are saved only after a login succeeds. A login
/// refused before it signs in, here for want of a client secret, leaves the
/// config file untouched.
#[test]
fn a_failed_login_does_not_save_the_given_ids() {
    let dir = tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR")).unwrap();
    let config_path = dir.path().join("config.toml");
    teams()
        .args(["--config", config_path.to_str().unwrap()])
        .args(["auth", "login", "--client-credentials"])
        .args(["--client-id", "app-1", "--tenant-id", "tenant-1"])
        .args(["--output", "json"])
        .assert()
        .code(2)
        .stdout(predicate::str::contains("Client secret is required"));
    assert!(!config_path.exists());
}
