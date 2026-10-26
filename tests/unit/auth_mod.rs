use super::{
    AUTH_ENV_PATH, LAZER_CLIENT_ID, LAZER_SCOPE, MeResponse, StoredAuth, X_API_VERSION, app_token,
    check_app_credentials, client_credentials_params, load, refresh_params,
    save_app_credentials_at, token_request_failed,
};
use crate::test_env::TempEnvVar;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    task::JoinHandle,
};

/// Look up a form param's value by key.
fn value<'a>(params: &'a [(&'a str, &'a str)], key: &str) -> Option<&'a str> {
    params.iter().find(|(k, _)| *k == key).map(|(_, v)| *v)
}

#[test]
fn api_constants_are_well_formed() {
    assert_eq!(LAZER_CLIENT_ID, "5");
    assert_eq!(LAZER_SCOPE, "*");
    // x-api-version must be a recent YYYYMMDD integer.
    assert_eq!(X_API_VERSION.len(), 8);
    assert!(X_API_VERSION.chars().all(|c| c.is_ascii_digit()));
}

#[test]
fn refresh_body_carries_scope_through() {
    // A `*` (lazer) token must refresh with `*`, not a narrower default.
    let params = refresh_params("5", "secret", "rt_abc", "*");
    assert_eq!(value(&params, "grant_type"), Some("refresh_token"));
    assert_eq!(value(&params, "refresh_token"), Some("rt_abc"));
    assert_eq!(value(&params, "scope"), Some("*"));
}

#[test]
fn client_credentials_body_includes_required_fields() {
    let params = client_credentials_params("42", "secret");
    assert_eq!(value(&params, "grant_type"), Some("client_credentials"));
    assert_eq!(value(&params, "scope"), Some("public"));
}

#[test]
fn token_request_error_omits_body() {
    let err = token_request_failed("login", reqwest::StatusCode::UNAUTHORIZED);
    let message = err.to_string();
    assert!(message.contains("login failed"));
    assert!(message.contains("401"));
    // The error must never leak submitted credentials or token material.
    assert!(!message.contains("access_token"));
    assert!(!message.contains("password"));
}

#[test]
fn has_lazer_scope_gates_official_mirror() {
    let mut auth = StoredAuth {
        client_id: "own-app".into(),
        client_secret: "secret".into(),
        redirect_uri: String::new(),
        access_token: "tok".into(),
        refresh_token: Some("rt".into()),
        expires_at: 0,
        scopes: vec![LAZER_SCOPE.into()],
        supporter: None,
    };
    assert!(auth.has_lazer_scope(), "a `*` token must pass the gate");

    // An old browser-OAuth token (no `*`) must be rejected for the mirror.
    auth.scopes = vec!["public".into(), "identify".into()];
    assert!(!auth.has_lazer_scope());

    auth.scopes = vec![];
    assert!(
        !auth.has_lazer_scope(),
        "a scopeless token must be rejected"
    );
}

#[test]
fn stored_auth_expiry() {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let fresh = StoredAuth {
        client_id: String::new(),
        client_secret: String::new(),
        redirect_uri: String::new(),
        access_token: "tok".into(),
        refresh_token: None,
        expires_at: now + 3600,
        scopes: vec![],
        supporter: None,
    };
    assert!(!fresh.is_expired());

    let stale = StoredAuth {
        expires_at: now - 1,
        ..fresh
    };
    assert!(stale.is_expired());
}

#[test]
fn token_persistence_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("auth.json");

    let auth = StoredAuth {
        client_id: "own-app".into(),
        client_secret: "secret".into(),
        redirect_uri: String::new(),
        access_token: "access".into(),
        refresh_token: Some("refresh".into()),
        expires_at: 9999999999,
        scopes: vec![LAZER_SCOPE.into()],
        supporter: Some(true),
    };

    let json = serde_json::to_string_pretty(&auth).unwrap();
    std::fs::write(&path, &json).unwrap();

    let loaded: StoredAuth =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(loaded.access_token, "access");
    assert_eq!(loaded.refresh_token.as_deref(), Some("refresh"));
    assert!(loaded.scopes.contains(&"*".to_string()));
    assert_eq!(loaded.supporter, Some(true));
}

/// An `auth.json` written before the `supporter` field existed must still
/// load, defaulting to unknown rather than failing to parse.
#[test]
fn stored_auth_deserializes_without_supporter_field() {
    let json = r#"{
        "client_id": "5",
        "client_secret": "secret",
        "redirect_uri": "",
        "access_token": "tok",
        "refresh_token": null,
        "expires_at": 9999999999,
        "scopes": ["*"]
    }"#;
    let loaded: StoredAuth = serde_json::from_str(json).unwrap();
    assert_eq!(loaded.supporter, None);
    assert!(!loaded.is_supporter());
}

#[test]
fn is_supporter_gates_conservatively() {
    let mut auth = StoredAuth {
        client_id: "own-app".into(),
        client_secret: "secret".into(),
        redirect_uri: String::new(),
        access_token: "tok".into(),
        refresh_token: None,
        expires_at: 0,
        scopes: vec![],
        supporter: None,
    };
    assert!(
        !auth.is_supporter(),
        "unknown status must not unlock supporter features"
    );

    auth.supporter = Some(false);
    assert!(!auth.is_supporter());

    auth.supporter = Some(true);
    assert!(auth.is_supporter());
}

#[test]
fn me_response_parses_is_supporter_from_fixture() {
    let json = r#"{"id":1,"username":"someone","is_supporter":true}"#;
    let body: MeResponse = serde_json::from_str(json).unwrap();
    assert!(body.is_supporter);
}

#[test]
fn me_response_defaults_is_supporter_false_when_field_absent() {
    let json = r#"{"id":1,"username":"someone"}"#;
    let body: MeResponse = serde_json::from_str(json).unwrap();
    assert!(!body.is_supporter);
}

fn stored(client_id: &str) -> StoredAuth {
    StoredAuth {
        client_id: client_id.into(),
        client_secret: "secret".into(),
        redirect_uri: String::new(),
        access_token: "tok".into(),
        refresh_token: Some("rt".into()),
        expires_at: u64::MAX,
        scopes: vec![LAZER_SCOPE.into()],
        supporter: Some(true),
    }
}

/// Earlier builds signed in through osu!lazer's own client. Such a token must
/// never be used again (refreshing it would sign in as lazer), so `load`
/// deletes it and reports no login.
#[test]
fn load_drops_a_login_made_through_the_lazer_client() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("auth.json");
    std::fs::write(&path, serde_json::to_string(&stored("5")).unwrap()).unwrap();
    let _auth = TempEnvVar::set(AUTH_ENV_PATH, path.to_str().unwrap());

    assert!(load().is_none());
    assert!(!path.exists(), "the old login is deleted, not kept around");
}

#[test]
fn load_keeps_a_login_from_any_other_client() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("auth.json");
    std::fs::write(&path, serde_json::to_string(&stored("own-app")).unwrap()).unwrap();
    let _auth = TempEnvVar::set(AUTH_ENV_PATH, path.to_str().unwrap());

    assert_eq!(
        load().map(|auth| auth.client_id).as_deref(),
        Some("own-app")
    );
    assert!(path.exists());
}

#[test]
fn only_a_login_with_a_refresh_token_is_a_user_login() {
    let mut auth = stored("own-app");
    assert!(auth.is_user_login());
    auth.refresh_token = None;
    assert!(
        !auth.is_user_login(),
        "an API-app token has no user behind it"
    );
}

#[test]
fn app_credentials_are_trimmed_before_use() {
    assert_eq!(
        check_app_credentials("  12345 \n", "\tsecret ").unwrap(),
        ("12345", "secret")
    );
}

#[test]
fn app_credentials_must_both_be_filled_in() {
    for (id, secret) in [("", "secret"), ("12345", "   "), ("", "")] {
        let err = check_app_credentials(id, secret).unwrap_err().to_string();
        assert!(err.contains("both"), "{id:?}/{secret:?}: {err}");
    }
}

#[test]
fn an_app_client_id_must_be_a_number() {
    let err = check_app_credentials("my-app", "secret")
        .unwrap_err()
        .to_string();
    assert!(err.contains("number"), "{err}");
}

#[test]
fn lazers_own_client_is_refused_as_an_app() {
    let err = check_app_credentials(LAZER_CLIENT_ID, "secret")
        .unwrap_err()
        .to_string();
    assert!(err.contains("osu!lazer"), "{err}");
}

/// A one-request stand-in for osu!'s token endpoint: answers `status` with
/// `body` and hands back the form body it was sent.
async fn token_endpoint(status: u16, body: &'static str) -> (String, JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/oauth/token", listener.local_addr().unwrap());
    let handle = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = Vec::new();
        let mut buf = [0u8; 4096];
        // Headers, then however much body Content-Length announced.
        let form = loop {
            let read = socket.read(&mut buf).await.unwrap();
            assert!(read > 0, "client hung up mid-request");
            request.extend_from_slice(&buf[..read]);
            let text = String::from_utf8_lossy(&request).into_owned();
            let Some((head, rest)) = text.split_once("\r\n\r\n") else {
                continue;
            };
            let length = head
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().ok())?
                })
                .unwrap_or(0);
            if rest.len() >= length {
                break rest.to_owned();
            }
        };
        let response = format!(
            "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        socket.write_all(response.as_bytes()).await.unwrap();
        form
    });
    (url, handle)
}

const TOKEN_BODY: &str = r#"{"token_type":"Bearer","expires_in":86400,"access_token":"app-token"}"#;
const INVALID_CLIENT_BODY: &str =
    r#"{"error":"invalid_client","error_description":"Client authentication failed"}"#;

#[tokio::test]
async fn saving_an_api_app_stores_a_token_with_no_user() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("auth.json");
    let _auth = TempEnvVar::set(AUTH_ENV_PATH, path.to_str().unwrap());
    let (url, server) = token_endpoint(200, TOKEN_BODY).await;

    save_app_credentials_at(&reqwest::Client::new(), &url, " 12345 ", "s3cret")
        .await
        .unwrap();

    let form = server.await.unwrap();
    assert!(form.contains("grant_type=client_credentials"), "{form}");
    assert!(form.contains("client_id=12345&"), "sent trimmed: {form}");
    let saved = load().expect("the app is saved");
    assert_eq!(saved.client_id, "12345");
    assert_eq!(saved.client_secret, "s3cret");
    assert_eq!(saved.access_token, "app-token");
    assert_eq!(saved.scopes, ["public"]);
    assert!(!saved.is_user_login());
    assert!(!saved.is_expired(), "expires_in was applied");
}

#[tokio::test]
async fn a_refused_api_app_is_not_saved_over_the_old_one() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("auth.json");
    std::fs::write(&path, serde_json::to_string(&stored("777")).unwrap()).unwrap();
    let _auth = TempEnvVar::set(AUTH_ENV_PATH, path.to_str().unwrap());
    let (url, server) = token_endpoint(401, INVALID_CLIENT_BODY).await;

    let err = save_app_credentials_at(&reqwest::Client::new(), &url, "12345", "wrong")
        .await
        .unwrap_err()
        .to_string();

    server.await.unwrap();
    assert!(err.contains("didn't accept"), "{err}");
    assert!(
        !err.contains("wrong"),
        "the secret never appears in the error"
    );
    assert_eq!(load().map(|auth| auth.client_id).as_deref(), Some("777"));
}

#[tokio::test]
async fn bad_input_never_reaches_osu() {
    // No endpoint is listening: a request would fail with a network error,
    // not the input message.
    let err = save_app_credentials_at(
        &reqwest::Client::new(),
        "http://127.0.0.1:9/oauth/token",
        "abc",
        "secret",
    )
    .await
    .unwrap_err()
    .to_string();
    assert!(err.contains("number"), "{err}");
}

#[tokio::test]
async fn any_other_refusal_keeps_its_status() {
    let (url, server) = token_endpoint(503, "{}").await;
    let err = app_token(&reqwest::Client::new(), &url, "12345", "secret")
        .await
        .unwrap_err()
        .to_string();
    server.await.unwrap();
    assert!(err.contains("503"), "{err}");
}
