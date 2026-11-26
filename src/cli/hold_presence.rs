//! Keeps the signed-in user shown as Available while the CLI is in use.
//!
//! Teams shows an account as Offline unless some client holds a presence session for it, so an
//! account driven only through this CLI looks absent while it is plainly at work. Before a Graph
//! command runs, the CLI sets this application's session to Available for [`HOLD`], so the
//! account stays green for an hour after its last command. The write is best-effort: a failure
//! is logged and never fails the command it accompanies.
//!
//! The session is refreshed at most once per [`REFRESH_AFTER`] per profile, which keeps an agent
//! that runs many commands from sending a write with each of them. A session set explicitly with
//! `teams presence set` is left alone until it expires, so a deliberate Busy is not overwritten.
//! Graph ranks a user's preferred presence, such as "Appear offline" chosen in a Teams client,
//! above every session, so that choice still wins.

use std::path::PathBuf;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::api::{self, GraphClient};
use crate::auth;
use crate::config::{self, ConfigFile};
use crate::models::presence::SetPresenceRequest;

/// How long each refresh keeps the session Available. Graph accepts up to four hours.
const HOLD: &str = "PT1H";

/// The shortest interval between two refreshes for one profile.
const REFRESH_AFTER: chrono::Duration = chrono::Duration::minutes(10);

/// The longest the refresh may delay the command it accompanies.
const WRITE_TIMEOUT: Duration = Duration::from_secs(10);

/// What the state file remembers about a profile's session.
#[derive(Debug, Default, Serialize, Deserialize)]
struct HoldState {
    /// When the CLI last tried to refresh the session, whether or not the write succeeded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    attempted_at: Option<DateTime<Utc>>,
    /// When a session set with `teams presence set` lapses.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    explicit_until: Option<DateTime<Utc>>,
}

/// A refresh that is due, ready to send alongside the command.
pub struct PendingHold {
    client: GraphClient,
    request: SetPresenceRequest,
    state_path: PathBuf,
}

/// Decide whether the profile's session needs a refresh, and prepare it if so.
///
/// This resolves the token before the command does, so that a token near expiry is refreshed
/// once, here, rather than twice in parallel.
pub async fn prepare(profile: &str, config: &ConfigFile) -> Option<PendingHold> {
    let state_path = state_path(profile)?;
    let now = Utc::now();
    if !refresh_due(&read_state(&state_path), now) {
        return None;
    }

    let token = match auth::resolve_token(profile).await {
        Ok(token) => token,
        Err(e) => {
            tracing::debug!("Not holding presence: no usable token for `{profile}`: {e}");
            return None;
        }
    };
    if auth::require_delegated_token(&token, "Holding presence").is_err() {
        tracing::debug!("Not holding presence: `{profile}` holds an app-only token");
        return None;
    }

    let configured_client_id = config::resolve_client_id(None, profile, config);
    let session_id = match super::presence::presence_session_id(&token, configured_client_id) {
        Ok(id) => id,
        Err(e) => {
            tracing::warn!("Not holding presence for `{profile}`: {e}");
            return None;
        }
    };
    let client = match GraphClient::new(token, &config.network) {
        Ok(client) => client,
        Err(e) => {
            tracing::warn!("Not holding presence for `{profile}`: {e}");
            return None;
        }
    };

    Some(PendingHold {
        client,
        request: SetPresenceRequest {
            session_id,
            availability: "Available".to_string(),
            activity: "Available".to_string(),
            expiration_duration: Some(HOLD.to_string()),
        },
        state_path,
    })
}

impl PendingHold {
    /// Send the refresh. The attempt is recorded before the write, so a profile whose writes
    /// fail, for example because Presence.ReadWrite was never consented, warns at most once per
    /// refresh interval rather than on every command.
    pub async fn send(self) {
        let mut state = read_state(&self.state_path);
        state.attempted_at = Some(Utc::now());
        write_state(&self.state_path, &state);

        let write = api::presence::set_presence(&self.client, &self.request);
        match tokio::time::timeout(WRITE_TIMEOUT, write).await {
            Ok(Ok(())) => tracing::debug!("Presence held Available for {HOLD}"),
            Ok(Err(e)) => tracing::warn!(
                "Could not set presence to Available: {e}. Pass --appear-offline to stop trying."
            ),
            Err(_) => tracing::warn!(
                "Setting presence to Available took over {}s; abandoned it",
                WRITE_TIMEOUT.as_secs()
            ),
        }
    }
}

/// Record that `teams presence set` set the session explicitly, so that the hold leaves it alone
/// until `expires_at`.
pub fn record_explicit(profile: &str, expires_at: DateTime<Utc>) {
    let Some(path) = state_path(profile) else {
        return;
    };
    let mut state = read_state(&path);
    state.explicit_until = Some(expires_at);
    write_state(&path, &state);
}

/// Forget the profile's session after `teams presence clear`, so the next command sets it again.
pub fn forget(profile: &str) {
    let Some(path) = state_path(profile) else {
        return;
    };
    if let Err(e) = std::fs::remove_file(&path) {
        if e.kind() != std::io::ErrorKind::NotFound {
            tracing::warn!("Could not remove presence state {}: {e}", path.display());
        }
    }
}

fn refresh_due(state: &HoldState, now: DateTime<Utc>) -> bool {
    let explicit = state.explicit_until.is_some_and(|until| until > now);
    let recent = state
        .attempted_at
        .is_some_and(|at| at <= now && now - at < REFRESH_AFTER);
    !explicit && !recent
}

fn state_path(profile: &str) -> Option<PathBuf> {
    match config::config_dir() {
        Ok(dir) => Some(
            dir.join("presence")
                .join(format!("{}.json", file_stem(profile))),
        ),
        Err(e) => {
            tracing::debug!("Not holding presence: {e}");
            None
        }
    }
}

/// Profile names are free text, so anything outside a conservative set becomes `_` before the
/// name is used as a file name.
fn file_stem(profile: &str) -> String {
    profile
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// A missing or unreadable state file reads as empty, which makes a refresh due.
fn read_state(path: &PathBuf) -> HoldState {
    std::fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

fn write_state(path: &PathBuf, state: &HoldState) {
    let result = path
        .parent()
        .map_or(Ok(()), std::fs::create_dir_all)
        .and_then(|()| std::fs::write(path, serde_json::to_vec(state).unwrap_or_default()));
    if let Err(e) = result {
        tracing::warn!("Could not save presence state {}: {e}", path.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(minutes_ago: i64, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
        Some(now - chrono::Duration::minutes(minutes_ago))
    }

    #[test]
    fn refresh_is_due_with_no_state() {
        assert!(refresh_due(&HoldState::default(), Utc::now()));
    }

    #[test]
    fn refresh_waits_out_the_interval_after_an_attempt() {
        let now = Utc::now();
        let recent = HoldState {
            attempted_at: at(9, now),
            ..Default::default()
        };
        let stale = HoldState {
            attempted_at: at(10, now),
            ..Default::default()
        };
        assert!(!refresh_due(&recent, now));
        assert!(refresh_due(&stale, now));
    }

    #[test]
    fn an_attempt_dated_in_the_future_does_not_block_refreshes() {
        let now = Utc::now();
        let skewed = HoldState {
            attempted_at: at(-60, now),
            ..Default::default()
        };
        assert!(refresh_due(&skewed, now));
    }

    #[test]
    fn an_explicit_session_is_left_alone_until_it_lapses() {
        let now = Utc::now();
        let live = HoldState {
            explicit_until: at(-5, now),
            ..Default::default()
        };
        let lapsed = HoldState {
            explicit_until: at(1, now),
            ..Default::default()
        };
        assert!(!refresh_due(&live, now));
        assert!(refresh_due(&lapsed, now));
    }

    #[test]
    fn file_stem_keeps_safe_names_and_replaces_the_rest() {
        assert_eq!(file_stem("benbot"), "benbot");
        assert_eq!(file_stem("work-2_a"), "work-2_a");
        assert_eq!(file_stem("../etc/x y"), "___etc_x_y");
    }

    #[test]
    fn state_round_trips_and_garbage_reads_as_empty() {
        let dir = std::env::temp_dir().join(format!("teams-hold-{}", uuid::Uuid::new_v4()));
        let path = dir.join("p.json");
        let now = Utc::now();
        write_state(
            &path,
            &HoldState {
                attempted_at: Some(now),
                explicit_until: None,
            },
        );
        assert_eq!(read_state(&path).attempted_at, Some(now));

        std::fs::write(&path, b"not json").unwrap();
        assert!(read_state(&path).attempted_at.is_none());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
