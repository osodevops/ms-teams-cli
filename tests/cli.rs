use assert_cmd::Command;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use predicates::prelude::*;
use std::fs;

fn teams() -> Command {
    let mut cmd = Command::cargo_bin("teams").unwrap();
    cmd.env("TEAMS_CLI_DISABLE_KEYRING", "1");
    cmd
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
