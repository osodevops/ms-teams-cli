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
    cmd
}

fn teams() -> Command {
    Command::from_std(teams_process())
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
fn version_flag_works() {
    teams()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn config_path_works() {
    teams()
        .args(["config", "path"])
        .assert()
        .success()
        .stdout(predicate::str::contains("teams-cli"));
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
                .and(predicate::str::contains("delete")),
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
                .and(predicate::str::contains("clear"))
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
