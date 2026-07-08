use std::collections::HashSet;

use serde::Serialize;

use crate::error::{Result, TeamsError};
use crate::models::chat::Chat;
use crate::models::common::PageResponse;
use crate::models::member::ConversationMember;
use crate::models::user::{Person, User};

use super::client::GraphClient;
use super::endpoints;

/// A person the resolver considers a plausible match for the query. The
/// `via` field records which lookup path produced the candidate so callers
/// can judge how much to trust it: `upn` is authoritative, `people-search`
/// is relevance-ranked, `roster` is a name/email match from shared chats.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolveCandidate {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_principal_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub department: Option<String>,
    pub via: String,
}

/// How strong a roster match is. A full-address email match identifies one
/// mailbox and ends the sweep early; alias and display-name matches can
/// collide with other people, so the sweep keeps scanning for the rest.
#[derive(Debug, Clone, Copy, PartialEq)]
enum MatchConfidence {
    Email,
    Name,
}

/// What happened at each resolution stage: `hit`, `miss`, `forbidden`
/// (token lacks the scope), or `skipped` (not attempted). For the roster
/// stage, `skipped_chats` counts rosters that could not be read (403/404),
/// marking the result as potentially incomplete.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StageReport {
    pub stage: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skipped_chats: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolveResult {
    pub query: String,
    pub candidates: Vec<ResolveCandidate>,
    pub stages: Vec<StageReport>,
}

/// Resolve a colleague reference (display name, email, UPN, or object ID)
/// to user candidates, degrading gracefully with the token's scopes:
///
/// 1. Exact `/users/{q}` lookup when the query looks like a UPN or GUID
///    (baseline User.ReadBasic.All).
/// 2. People search via `/me/people` (People.Read).
/// 3. Roster sweep over group/meeting chat members (baseline chat scopes),
///    bounded by `max_chats`; an exact email match stops it early, while
///    bare-name matches keep it scanning for namesakes.
///
/// A stage that fails with 403 is recorded as `forbidden` and the resolver
/// moves on; any other transport or server error aborts the resolution.
pub async fn resolve_user(
    client: &GraphClient,
    query: &str,
    max_chats: u64,
) -> Result<ResolveResult> {
    resolve_user_at(client, endpoints::GRAPH_V1, query, max_chats).await
}

pub(crate) async fn resolve_user_at(
    client: &GraphClient,
    base: &str,
    query: &str,
    max_chats: u64,
) -> Result<ResolveResult> {
    let mut stages: Vec<StageReport> = Vec::new();

    if looks_like_exact_reference(query) {
        match client
            .get::<User>(&format!("{base}/users/{query}"), &[])
            .await
        {
            Ok(user) => {
                stages.push(stage("upn", "hit"));
                stages.push(stage("people-search", "skipped"));
                stages.push(stage("roster", "skipped"));
                return Ok(ResolveResult {
                    query: query.to_string(),
                    candidates: vec![candidate_from_user(&user)],
                    stages,
                });
            }
            Err(e) if is_forbidden(&e) => stages.push(stage("upn", "forbidden")),
            Err(TeamsError::NotFound(_)) | Err(TeamsError::ApiError { status: 400, .. }) => {
                stages.push(stage("upn", "miss"));
            }
            Err(e) => return Err(e),
        }
    } else {
        stages.push(stage("upn", "skipped"));
    }

    match search_people(client, base, query).await {
        Ok(people) if !people.is_empty() => {
            stages.push(stage("people-search", "hit"));
            stages.push(stage("roster", "skipped"));
            return Ok(ResolveResult {
                query: query.to_string(),
                candidates: people.iter().map(candidate_from_person).collect(),
                stages,
            });
        }
        Ok(_) => stages.push(stage("people-search", "miss")),
        Err(e) if is_forbidden(&e) => stages.push(stage("people-search", "forbidden")),
        Err(e) => return Err(e),
    }

    let (candidates, skipped_chats) = sweep_rosters(client, base, query, max_chats).await?;
    stages.push(StageReport {
        stage: "roster".to_string(),
        status: if candidates.is_empty() { "miss" } else { "hit" }.to_string(),
        skipped_chats: (skipped_chats > 0).then_some(skipped_chats),
    });
    Ok(ResolveResult {
        query: query.to_string(),
        candidates,
        stages,
    })
}

/// Relevance-ranked people search via `/me/people`. Requires the People.Read
/// delegated scope; Graph answers 403 without it. The search term must be
/// wrapped in double quotes per the people API's `$search` contract.
async fn search_people(client: &GraphClient, base: &str, query: &str) -> Result<Vec<Person>> {
    let quoted = format!("\"{}\"", query.replace('"', ""));
    let resp: PageResponse<Person> = client
        .get(
            &format!("{base}/me/people"),
            &[("$search", quoted.as_str()), ("$top", "10")],
        )
        .await?;
    Ok(resp.value)
}

/// Walk group and meeting chat rosters looking for the query, scanning up
/// to `max_chats` chats and returning the candidates plus a count of
/// rosters that couldn't be read. Colleagues cluster in shared meetings, so
/// matches usually land early; a full-address email match ends the sweep
/// after the current chat, but alias and name matches keep it scanning so
/// namesakes in other chats aren't silently missed.
async fn sweep_rosters(
    client: &GraphClient,
    base: &str,
    query: &str,
    max_chats: u64,
) -> Result<(Vec<ResolveCandidate>, u64)> {
    let pb = crate::output::progress::sweep_bar(max_chats);
    let mut candidates: Vec<ResolveCandidate> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut confident = false;
    let mut skipped = 0u64;
    let mut scanned = 0u64;
    let mut next: Option<String> = Some(format!("{base}/me/chats"));
    let mut first_page = true;

    'pages: while let Some(url) = next {
        let page_query: &[(&str, &str)] = if first_page { &[("$top", "50")] } else { &[] };
        first_page = false;
        let page: PageResponse<Chat> = client.get(&url, page_query).await?;
        next = page.next_link;

        for chat in &page.value {
            if scanned >= max_chats {
                break 'pages;
            }
            let is_shared = matches!(chat.chat_type.as_deref(), Some("group") | Some("meeting"));
            let Some(chat_id) = chat.id.as_deref().filter(|_| is_shared) else {
                continue;
            };
            scanned += 1;
            pb.inc(1);

            // A single unreadable roster (403/404, e.g. a stale or
            // restricted chat) shouldn't abort the whole sweep — count it
            // and move on. Anything else (rate limiting, 5xx, network)
            // means Graph is degraded, and continuing would let the caller
            // mistake a partial sweep for a definitive miss.
            let members: Vec<ConversationMember> = match client
                .get_all_pages(&format!("{base}/chats/{chat_id}/members"), &[])
                .await
            {
                Ok(members) => members,
                Err(e @ (TeamsError::PermissionDenied(_) | TeamsError::NotFound(_))) => {
                    tracing::warn!("skipping unreadable chat {chat_id}: {e}");
                    skipped += 1;
                    continue;
                }
                Err(e) => return Err(e),
            };

            for m in &members {
                if let Some(confidence) = member_match(query, m) {
                    let key = m
                        .user_id
                        .clone()
                        .or_else(|| m.email.clone())
                        .unwrap_or_default();
                    if seen.insert(key) {
                        candidates.push(candidate_from_member(m));
                    }
                    if confidence == MatchConfidence::Email {
                        confident = true;
                    }
                }
            }
            if confident {
                break 'pages;
            }
        }
    }

    pb.finish_and_clear();
    Ok((candidates, skipped))
}

/// A query is worth an exact `/users/{q}` lookup only when it could be a
/// UPN or an object ID; display names contain whitespace and never match.
fn looks_like_exact_reference(query: &str) -> bool {
    !query.contains(char::is_whitespace) && (query.contains('@') || is_guid(query))
}

fn is_guid(s: &str) -> bool {
    let bytes = s.as_bytes();
    bytes.len() == 36
        && bytes.iter().enumerate().all(|(i, b)| match i {
            8 | 13 | 18 | 23 => *b == b'-',
            _ => b.is_ascii_hexdigit(),
        })
}

/// A query containing '@' is an explicit address and only matches the
/// member's full email, case-insensitively — matching on the local part
/// alone could silently resolve jane@vendor.com to jane@corp.com. Bare
/// queries match an email local part (e.g. "jsmith") or a display name in
/// which every whitespace-separated token appears; both are suggestive
/// rather than authoritative, since aliases and names collide across
/// domains and people.
fn member_match(query: &str, member: &ConversationMember) -> Option<MatchConfidence> {
    let email = member.email.as_deref().unwrap_or_default();

    if query.contains('@') {
        return email
            .eq_ignore_ascii_case(query)
            .then_some(MatchConfidence::Email);
    }

    let member_local = email.split('@').next().unwrap_or_default();
    if !member_local.is_empty() && member_local.eq_ignore_ascii_case(query) {
        return Some(MatchConfidence::Name);
    }

    let name = member
        .display_name
        .as_deref()
        .unwrap_or_default()
        .to_lowercase();
    (!name.is_empty()
        && query
            .split_whitespace()
            .all(|token| name.contains(&token.to_lowercase())))
    .then_some(MatchConfidence::Name)
}

fn stage(name: &str, status: &str) -> StageReport {
    StageReport {
        stage: name.to_string(),
        status: status.to_string(),
        skipped_chats: None,
    }
}

fn is_forbidden(err: &TeamsError) -> bool {
    matches!(
        err,
        TeamsError::PermissionDenied(_) | TeamsError::ApiError { status: 403, .. }
    )
}

fn candidate_from_user(user: &User) -> ResolveCandidate {
    ResolveCandidate {
        id: user.id.clone(),
        display_name: user.display_name.clone(),
        mail: user.mail.clone(),
        user_principal_name: user.user_principal_name.clone(),
        job_title: user.job_title.clone(),
        department: None,
        via: "upn".to_string(),
    }
}

fn candidate_from_person(person: &Person) -> ResolveCandidate {
    let mail = person
        .scored_email_addresses
        .as_ref()
        .and_then(|addrs| addrs.first())
        .and_then(|a| a.address.clone());
    ResolveCandidate {
        id: person.id.clone(),
        display_name: person.display_name.clone(),
        mail,
        user_principal_name: person.user_principal_name.clone(),
        job_title: person.job_title.clone(),
        department: person.department.clone(),
        via: "people-search".to_string(),
    }
}

fn candidate_from_member(member: &ConversationMember) -> ResolveCandidate {
    ResolveCandidate {
        id: member.user_id.clone(),
        display_name: member.display_name.clone(),
        mail: member.email.clone(),
        user_principal_name: None,
        job_title: None,
        department: None,
        via: "roster".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::token::TokenInfo;
    use crate::config::NetworkConfig;
    use reqwest::Client;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn test_client() -> GraphClient {
        GraphClient {
            http: Client::new(),
            token: TokenInfo {
                access_token: "test-token".into(),
                expires_at: None,
                token_type: "Bearer".into(),
                scope: None,
                refresh_token: None,
                profile: "default".into(),
            },
            network: NetworkConfig {
                timeout: 30,
                max_retries: 0,
                retry_backoff_base: 2,
            },
        }
    }

    const GUID: &str = "99bb638b-5ca8-4bb9-8ee6-7a2d15dab139";

    #[test]
    fn exact_reference_detection() {
        assert!(looks_like_exact_reference("someone@example.com"));
        assert!(looks_like_exact_reference(GUID));
        assert!(!looks_like_exact_reference("Jane Smith"));
        assert!(!looks_like_exact_reference("jsmith"));
        assert!(!looks_like_exact_reference("Jane Smith <jane@example.com>"));
    }

    #[test]
    fn member_matching_rules() {
        let member = ConversationMember {
            id: None,
            display_name: Some("Jane von Smith".into()),
            roles: None,
            user_id: Some("u1".into()),
            email: Some("JSmith@example.com".into()),
        };
        assert_eq!(
            member_match("jsmith@example.com", &member),
            Some(MatchConfidence::Email)
        );
        // Same local part on a different domain is a different mailbox.
        assert_eq!(member_match("jsmith@other-domain.com", &member), None);
        // A bare alias is suggestive, never email-confident.
        assert_eq!(member_match("jsmith", &member), Some(MatchConfidence::Name));
        assert_eq!(
            member_match("jane smith", &member),
            Some(MatchConfidence::Name)
        );
        assert_eq!(member_match("SMITH", &member), Some(MatchConfidence::Name));
        assert_eq!(member_match("jane.smith@example.com", &member), None);
        assert_eq!(member_match("john smith", &member), None);
    }

    #[tokio::test]
    async fn guid_query_resolves_via_exact_lookup_and_skips_other_stages() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path(format!("/users/{GUID}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": GUID,
                "displayName": "Abe Ingersoll",
                "mail": "abe@example.com",
                "userPrincipalName": "abe@example.com"
            })))
            .expect(1)
            .mount(&server)
            .await;

        let result = resolve_user_at(&test_client(), &server.uri(), GUID, 10)
            .await
            .unwrap();

        assert_eq!(result.candidates.len(), 1);
        assert_eq!(result.candidates[0].via, "upn");
        assert_eq!(result.candidates[0].id.as_deref(), Some(GUID));
        let statuses: Vec<(&str, &str)> = result
            .stages
            .iter()
            .map(|s| (s.stage.as_str(), s.status.as_str()))
            .collect();
        assert_eq!(
            statuses,
            vec![
                ("upn", "hit"),
                ("people-search", "skipped"),
                ("roster", "skipped")
            ]
        );
    }

    #[tokio::test]
    async fn upn_miss_falls_through_to_people_search() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/users/ghost@example.com"))
            .respond_with(ResponseTemplate::new(404).set_body_string("not found"))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/me/people"))
            .and(query_param("$search", "\"ghost@example.com\""))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "value": [{
                    "id": "person-1",
                    "displayName": "Ghost Writer",
                    "userPrincipalName": "ghostw@example.com",
                    "scoredEmailAddresses": [
                        { "address": "ghost@example.com", "relevanceScore": 20.0 }
                    ]
                }]
            })))
            .mount(&server)
            .await;

        let result = resolve_user_at(&test_client(), &server.uri(), "ghost@example.com", 10)
            .await
            .unwrap();

        assert_eq!(result.candidates.len(), 1);
        assert_eq!(result.candidates[0].via, "people-search");
        assert_eq!(
            result.candidates[0].mail.as_deref(),
            Some("ghost@example.com")
        );
        assert_eq!(result.stages[0].status, "miss");
        assert_eq!(result.stages[1].status, "hit");
        assert_eq!(result.stages[2].status, "skipped");
    }

    #[tokio::test]
    async fn forbidden_people_search_degrades_to_roster_sweep_collecting_namesakes() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/me/people"))
            .respond_with(ResponseTemplate::new(403).set_body_string("no People.Read"))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/me/chats"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "value": [
                    { "id": "dm-1", "chatType": "oneOnOne" },
                    { "id": "grp-1", "chatType": "group" },
                    { "id": "grp-2", "chatType": "group" }
                ]
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/chats/grp-1/members"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "value": [
                    {
                        "id": "m1",
                        "displayName": "Jane Smith",
                        "userId": "u-jane",
                        "email": "Springsteenw@example.com"
                    }
                ]
            })))
            .expect(1)
            .mount(&server)
            .await;
        // A bare-name match is not confident, so the sweep continues into
        // grp-2 and picks up the namesake there too. u-jane reappearing in
        // grp-2 must not produce a duplicate candidate.
        Mock::given(method("GET"))
            .and(path("/chats/grp-2/members"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "value": [
                    {
                        "id": "m1b",
                        "displayName": "Jane Smith",
                        "userId": "u-jane",
                        "email": "Springsteenw@example.com"
                    },
                    {
                        "id": "m2",
                        "displayName": "Jane Smith",
                        "userId": "u-jane-2",
                        "email": "jane.smith2@example.com"
                    }
                ]
            })))
            .expect(1)
            .mount(&server)
            .await;

        let result = resolve_user_at(&test_client(), &server.uri(), "jane smith", 10)
            .await
            .unwrap();

        let ids: Vec<&str> = result
            .candidates
            .iter()
            .map(|c| c.id.as_deref().unwrap())
            .collect();
        assert_eq!(ids, vec!["u-jane", "u-jane-2"]);
        assert!(result.candidates.iter().all(|c| c.via == "roster"));
        let statuses: Vec<(&str, &str)> = result
            .stages
            .iter()
            .map(|s| (s.stage.as_str(), s.status.as_str()))
            .collect();
        assert_eq!(
            statuses,
            vec![
                ("upn", "skipped"),
                ("people-search", "forbidden"),
                ("roster", "hit")
            ]
        );
    }

    #[tokio::test]
    async fn email_confident_roster_match_ends_sweep_early() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/me/people"))
            .respond_with(ResponseTemplate::new(403).set_body_string("no People.Read"))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/me/chats"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "value": [
                    { "id": "grp-1", "chatType": "group" },
                    { "id": "grp-2", "chatType": "group" }
                ]
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/chats/grp-1/members"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "value": [
                    {
                        "id": "m1",
                        "displayName": "Will Haynes",
                        "userId": "u-will",
                        "email": "Springsteenw@example.com"
                    }
                ]
            })))
            .expect(1)
            .mount(&server)
            .await;
        // A full-address match identifies one mailbox; grp-2 must not be
        // fetched.
        Mock::given(method("GET"))
            .and(path("/chats/grp-2/members"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "value": []
            })))
            .expect(0)
            .mount(&server)
            .await;

        let result = resolve_user_at(
            &test_client(),
            &server.uri(),
            "springsteenw@example.com",
            10,
        )
        .await
        .unwrap();

        assert_eq!(result.candidates.len(), 1);
        assert_eq!(result.candidates[0].id.as_deref(), Some("u-will"));
    }

    #[tokio::test]
    async fn sweep_respects_max_chats_bound() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/me/people"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "value": []
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/me/chats"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "value": [
                    { "id": "grp-1", "chatType": "group" },
                    { "id": "grp-2", "chatType": "group" }
                ]
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/chats/grp-1/members"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "value": []
            })))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/chats/grp-2/members"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "value": []
            })))
            .expect(0)
            .mount(&server)
            .await;

        let result = resolve_user_at(&test_client(), &server.uri(), "nobody here", 1)
            .await
            .unwrap();

        assert!(result.candidates.is_empty());
        assert_eq!(result.stages[2].status, "miss");
    }

    #[tokio::test]
    async fn unreadable_roster_is_skipped_not_fatal() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/me/people"))
            .respond_with(ResponseTemplate::new(403).set_body_string("no scope"))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/me/chats"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "value": [
                    { "id": "grp-bad", "chatType": "group" },
                    { "id": "grp-good", "chatType": "meeting" }
                ]
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/chats/grp-bad/members"))
            .respond_with(ResponseTemplate::new(403).set_body_string("nope"))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/chats/grp-good/members"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "value": [
                    {
                        "id": "m1",
                        "displayName": "Target Person",
                        "userId": "u-target",
                        "email": "target@example.com"
                    }
                ]
            })))
            .mount(&server)
            .await;

        let result = resolve_user_at(&test_client(), &server.uri(), "target person", 10)
            .await
            .unwrap();

        assert_eq!(result.candidates.len(), 1);
        assert_eq!(result.candidates[0].id.as_deref(), Some("u-target"));
        let roster = result.stages.iter().find(|s| s.stage == "roster").unwrap();
        assert_eq!(roster.skipped_chats, Some(1));
    }

    #[tokio::test]
    async fn degraded_graph_during_sweep_propagates_instead_of_reporting_miss() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/me/people"))
            .respond_with(ResponseTemplate::new(403).set_body_string("no scope"))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/me/chats"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "value": [{ "id": "grp-1", "chatType": "group" }]
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/chats/grp-1/members"))
            .respond_with(ResponseTemplate::new(500).set_body_string("Graph is having a day"))
            .mount(&server)
            .await;

        let err = resolve_user_at(&test_client(), &server.uri(), "somebody real", 10)
            .await
            .unwrap_err();

        assert!(
            matches!(
                err,
                TeamsError::ApiError { status: 500, .. } | TeamsError::ServerError { .. }
            ),
            "expected a server error, got: {err:?}"
        );
    }
}
