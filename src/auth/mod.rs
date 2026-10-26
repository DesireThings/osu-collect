use crate::config::constants::CONFIG_SUBDIR;
use crate::utils::{AppError, Result};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, info, warn};

const AUTH_FILE: &str = "auth.json";
/// Env override for the stored-auth path, matching every other on-disk store
/// (`OSU_COLLECT_STATE`, `OSU_COLLECT_FAILED_MAPS`, …). Without it a test that
/// builds an `App` reads the developer's real login and renders a different
/// config tab than CI does.
pub const AUTH_ENV_PATH: &str = "OSU_COLLECT_AUTH";
const REFRESH_MARGIN_SECS: u64 = 60;
const OSU_TOKEN_URL: &str = "https://osu.ppy.sh/oauth/token";

/// Base URL for osu! API v2.
pub const OSU_API_BASE: &str = "https://osu.ppy.sh/api/v2";

/// Where a user registers their own osu! OAuth app. The callback URL can stay
/// blank: the app is only used for the client-credentials grant.
pub const OAUTH_APP_URL: &str = "https://osu.ppy.sh/home/account/edit#new-oauth-application";

/// osu!lazer's own OAuth client id. Earlier builds signed in through it, which
/// ppy never allowed third-party apps to do; a token stored by one of them is
/// dropped on load (see [`load`]) and nothing here requests one any more.
const LAZER_CLIENT_ID: &str = "5";
/// The `*` scope that carries the beatmap-download privilege. osu! only grants
/// it to its own lazer client, so no token this app can obtain carries it and
/// the osu! official mirror stays locked.
pub const LAZER_SCOPE: &str = "*";
/// `x-api-version` header value sent on every api v2 request. Any recent
/// `YYYYMMDD` integer works; this mirrors a known-good osu!lazer build and is
/// **mandatory** — osu! rejects api v2 calls that omit it.
pub const X_API_VERSION: &str = "20250115";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_in: u64,
    pub token_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredAuth {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: u64,
    pub scopes: Vec<String>,
    /// Cached `/me` `is_supporter` flag. `None` means unknown — never probed,
    /// or the last probe failed — and reads as **not** supporter via
    /// [`is_supporter`](Self::is_supporter): this gates supporter-only
    /// features, so an unconfirmed account must never unlock them.
    /// `#[serde(default)]` so an `auth.json` written before this field
    /// existed still loads.
    #[serde(default)]
    pub supporter: Option<bool>,
}

impl StoredAuth {
    pub fn is_expired(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        now + REFRESH_MARGIN_SECS >= self.expires_at
    }

    pub fn bearer_token(&self) -> &str {
        &self.access_token
    }

    /// Whether this token carries the `*` (lazer-tier) scope the official-mirror
    /// download endpoint requires. The client-side gate that stops an old
    /// narrow-scope token from being attached to `MirrorKind::OsuApi`.
    pub fn has_lazer_scope(&self) -> bool {
        self.scopes.iter().any(|scope| scope == LAZER_SCOPE)
    }

    /// Whether the account carries osu!supporter, per the last successful
    /// `/me` probe. `None` (never probed, or the probe failed) reads as
    /// `false` — the conservative direction, since this gates supporter-only
    /// features.
    pub fn is_supporter(&self) -> bool {
        self.supporter == Some(true)
    }

    /// Whether this token belongs to a signed-in user. Only a user login comes
    /// with a refresh token; the user's own API app (see
    /// [`save_app_credentials`]) gets a client-credentials token, which has no
    /// user behind it and can't call `/me`.
    pub fn is_user_login(&self) -> bool {
        self.refresh_token.is_some()
    }
}

fn auth_path() -> Option<std::path::PathBuf> {
    if let Ok(custom) = std::env::var(AUTH_ENV_PATH) {
        let trimmed = custom.trim();
        if !trimmed.is_empty() {
            return Some(std::path::PathBuf::from(trimmed));
        }
    }

    dirs::config_dir().map(|d| d.join(CONFIG_SUBDIR).join(AUTH_FILE))
}

/// The stored token, if any. A token issued to osu!lazer's own client by an
/// earlier build is deleted rather than used: refreshing it would sign in as
/// lazer again.
pub fn load() -> Option<StoredAuth> {
    let path = auth_path()?;
    let contents = std::fs::read_to_string(&path).ok()?;
    let auth: StoredAuth = serde_json::from_str(&contents).ok()?;
    if auth.client_id == LAZER_CLIENT_ID {
        info!("dropping a login made through osu!lazer's client by an earlier version");
        if let Err(err) = std::fs::remove_file(&path) {
            warn!(error = %err, "could not delete the old login");
        }
        return None;
    }
    Some(auth)
}

pub fn save(auth: &StoredAuth) -> Result<()> {
    let path = auth_path().ok_or_else(|| AppError::config("cannot determine config dir"))?;

    if let Some(parent) = path.parent()
        && !parent.exists()
    {
        std::fs::create_dir_all(parent)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(parent)?.permissions();
            perms.set_mode(0o700);
            std::fs::set_permissions(parent, perms)?;
        }
    }

    let json = serde_json::to_string_pretty(auth)
        .map_err(|e| AppError::other_dynamic(format!("auth serialize: {e}").into_boxed_str()))?;

    // Create the temp file 0600 *before* the token is written, so it is never
    // briefly world-readable under the process umask; the rename preserves the
    // mode onto the final file.
    let tmp = path.with_extension("json.tmp");
    {
        use std::io::Write as _;
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut file = opts.open(&tmp)?;
        file.write_all(json.as_bytes())?;
        file.sync_all()?;
    }
    std::fs::rename(&tmp, &path)?;

    #[cfg(unix)]
    {
        // Belt-and-suspenders: a stale pre-existing temp would have kept its old
        // mode (OpenOptions::mode only applies on creation), so re-assert 0600.
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&path)?.permissions();
        perms.set_mode(0o600);
        std::fs::set_permissions(&path, perms)?;
    }

    debug!("auth saved to {}", path.display());
    Ok(())
}

pub fn delete() -> Result<()> {
    if let Some(path) = auth_path()
        && path.exists()
    {
        std::fs::remove_file(&path)?;
        info!("auth tokens removed");
    }
    Ok(())
}

pub fn token_request_failed(operation: &str, status: reqwest::StatusCode) -> AppError {
    AppError::other_dynamic(format!("{operation} failed ({status})").into_boxed_str())
}

/// Parsed subset of the osu! `/me` response this module reads.
#[derive(Debug, Deserialize)]
struct MeResponse {
    #[serde(default)]
    is_supporter: bool,
}

/// Single `/me` GET. `None` for a network error, a non-success status or an
/// unparseable body: every caller treats those as "no usable data".
async fn fetch_me(client: &reqwest::Client, access_token: &str) -> Option<MeResponse> {
    let resp = match client
        .get(format!("{OSU_API_BASE}/me"))
        .bearer_auth(access_token)
        .header("x-api-version", X_API_VERSION)
        .send()
        .await
    {
        Ok(resp) => resp,
        Err(err) => {
            warn!(error = %err, "/me probe failed; assuming no usable data");
            return None;
        }
    };
    if !resp.status().is_success() {
        return None;
    }
    resp.json::<MeResponse>().await.ok()
}

/// Re-probes `/me` and persists `is_supporter` on `auth`. `Some` is the answer
/// the server actually gave; `None` means the probe carried no usable body (a
/// network error, a 401, an unparseable response) and `auth.supporter` was left
/// exactly as it was.
///
/// The `Option` is the whole contract: a caller must be able to tell a confirmed
/// `false` from an unanswered probe, or a flaky network silently revokes a
/// supporter's features.
pub async fn refresh_supporter_status(
    client: &reqwest::Client,
    auth: &mut StoredAuth,
) -> Option<bool> {
    let confirmed = fetch_me(client, auth.bearer_token()).await?.is_supporter;
    auth.supporter = Some(confirmed);
    if let Err(err) = save(auth) {
        warn!(error = %err, "failed to persist supporter status");
    }
    Some(confirmed)
}

pub fn refresh_params<'a>(
    client_id: &'a str,
    client_secret: &'a str,
    refresh_token: &'a str,
    scope: &'a str,
) -> [(&'a str, &'a str); 5] {
    [
        ("client_id", client_id),
        ("client_secret", client_secret),
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token),
        ("scope", scope),
    ]
}

/// Refresh an access token. `scope` must match the stored token's scope so a
/// `*` (lazer) token refreshes with `*` rather than a narrower default.
pub async fn refresh(
    client: &reqwest::Client,
    client_id: &str,
    client_secret: &str,
    refresh_token: &str,
    scope: &str,
) -> Result<TokenResponse> {
    let params = refresh_params(client_id, client_secret, refresh_token, scope);

    let resp = client
        .post(OSU_TOKEN_URL)
        .form(&params)
        .send()
        .await
        .map_err(|e| AppError::other_dynamic(format!("refresh request: {e}").into_boxed_str()))?;

    if !resp.status().is_success() {
        return Err(token_request_failed("token refresh", resp.status()));
    }

    resp.json::<TokenResponse>()
        .await
        .map_err(|e| AppError::other_dynamic(format!("refresh parse: {e}").into_boxed_str()))
}

pub fn client_credentials_params<'a>(
    client_id: &'a str,
    client_secret: &'a str,
) -> [(&'a str, &'a str); 4] {
    [
        ("client_id", client_id),
        ("client_secret", client_secret),
        ("grant_type", "client_credentials"),
        ("scope", "public"),
    ]
}

pub async fn client_credentials(
    client: &reqwest::Client,
    client_id: &str,
    client_secret: &str,
) -> Result<TokenResponse> {
    let params = client_credentials_params(client_id, client_secret);
    request_token(client, OSU_TOKEN_URL, &params).await
}

async fn request_token(
    client: &reqwest::Client,
    token_url: &str,
    params: &[(&str, &str)],
) -> Result<TokenResponse> {
    post_token(client, token_url, params)
        .await?
        .map_err(|status| token_request_failed("token request", status))
}

/// One token POST. The outer `Err` is a network or parse failure; the inner one
/// is the HTTP status of a refused request, so callers can word a rejection.
async fn post_token(
    client: &reqwest::Client,
    token_url: &str,
    params: &[(&str, &str)],
) -> Result<std::result::Result<TokenResponse, reqwest::StatusCode>> {
    let resp = client
        .post(token_url)
        .form(params)
        .send()
        .await
        .map_err(|e| AppError::other_dynamic(format!("token request: {e}").into_boxed_str()))?;

    if !resp.status().is_success() {
        return Ok(Err(resp.status()));
    }

    resp.json::<TokenResponse>()
        .await
        .map(Ok)
        .map_err(|e| AppError::other_dynamic(format!("token parse: {e}").into_boxed_str()))
}

/// A client-credentials token for the user's own API app, with osu!'s refusal
/// (`401 invalid_client`, or a 400) worded for the settings screen.
async fn app_token(
    client: &reqwest::Client,
    token_url: &str,
    client_id: &str,
    client_secret: &str,
) -> Result<TokenResponse> {
    let params = client_credentials_params(client_id, client_secret);
    match post_token(client, token_url, &params).await? {
        Ok(token) => Ok(token),
        Err(reqwest::StatusCode::UNAUTHORIZED | reqwest::StatusCode::BAD_REQUEST) => Err(
            AppError::api("osu! didn't accept the API app's client ID and secret"),
        ),
        Err(status) => Err(token_request_failed("token request", status)),
    }
}

/// Check the user's own osu! OAuth app by getting a token with it, then store
/// it in place of whatever was saved before. Search and enrichment pick it up
/// through [`load`], and [`ensure_valid`] gets a new token the same way when
/// this one runs out. Nothing is saved when osu! refuses the app.
pub async fn save_app_credentials(
    client: &reqwest::Client,
    client_id: &str,
    client_secret: &str,
) -> Result<()> {
    save_app_credentials_at(client, OSU_TOKEN_URL, client_id, client_secret).await
}

async fn save_app_credentials_at(
    client: &reqwest::Client,
    token_url: &str,
    client_id: &str,
    client_secret: &str,
) -> Result<()> {
    let (client_id, client_secret) = check_app_credentials(client_id, client_secret)?;
    let token = app_token(client, token_url, client_id, client_secret).await?;
    let mut auth = StoredAuth {
        client_id: client_id.to_owned(),
        client_secret: client_secret.to_owned(),
        redirect_uri: String::new(),
        access_token: String::new(),
        refresh_token: None,
        expires_at: 0,
        scopes: vec!["public".into()],
        supporter: None,
    };
    apply_token_response(&mut auth, token);
    save_off_thread(&auth).await?;
    info!("saved the osu! API app");
    Ok(())
}

/// Trim and sanity-check typed-in app credentials before asking osu!.
fn check_app_credentials<'a>(
    client_id: &'a str,
    client_secret: &'a str,
) -> Result<(&'a str, &'a str)> {
    let (client_id, client_secret) = (client_id.trim(), client_secret.trim());
    if client_id.is_empty() || client_secret.is_empty() {
        return Err(AppError::config(
            "enter both the client ID and the client secret",
        ));
    }
    if !client_id.bytes().all(|b| b.is_ascii_digit()) {
        return Err(AppError::config(
            "the client ID is a number; copy it from your osu! account settings",
        ));
    }
    if client_id == LAZER_CLIENT_ID {
        return Err(AppError::config(
            "that's osu!lazer's own client; register an app of your own",
        ));
    }
    Ok((client_id, client_secret))
}

/// [`save`] on a blocking thread, for async callers.
async fn save_off_thread(auth: &StoredAuth) -> Result<()> {
    let snap = auth.clone();
    tokio::task::spawn_blocking(move || save(&snap))
        .await
        .map_err(|e| AppError::other_dynamic(format!("save task panicked: {e}").into_boxed_str()))?
}

pub async fn ensure_valid(client: &reqwest::Client, auth: &mut StoredAuth) -> Result<()> {
    if !auth.is_expired() {
        return Ok(());
    }

    let token_resp = if let Some(refresh_token) = auth.refresh_token.as_deref() {
        info!("refreshing OAuth token");
        // Refresh with the token's own scope so a `*` (lazer) token keeps `*`.
        let scope = auth.scopes.join(" ");
        refresh(
            client,
            &auth.client_id,
            &auth.client_secret,
            refresh_token,
            &scope,
        )
        .await?
    } else {
        info!("refreshing OAuth token with client credentials");
        app_token(client, OSU_TOKEN_URL, &auth.client_id, &auth.client_secret).await?
    };
    apply_token_response(auth, token_resp);
    save_off_thread(auth).await
}

fn apply_token_response(auth: &mut StoredAuth, resp: TokenResponse) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    auth.access_token = resp.access_token;
    if let Some(rt) = resp.refresh_token {
        auth.refresh_token = Some(rt);
    }
    auth.expires_at = now + resp.expires_in;
}

#[cfg(test)]
#[path = "../../tests/unit/auth_mod.rs"]
mod tests;

#[cfg(test)]
#[path = "../../tests/unit/auth.rs"]
mod integration;
