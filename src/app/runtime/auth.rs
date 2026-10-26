use super::super::{ApiAppCredentials, App};
use crate::auth;
use tokio::sync::mpsc;
use tracing::{debug, warn};

#[derive(Debug)]
pub(super) enum AuthEvent {
    /// A background `/me` re-probe of an already-stored token answered. Only
    /// ever sent for a CONFIRMED answer — an unreachable or unauthorized probe
    /// sends nothing, so a flaky network can never revoke supporter features.
    SupporterRefreshed(bool),
    /// osu! accepted the user's API app and it's saved; carries its client id.
    ApiAppSaved(String),
    /// The API app check failed; the reason is user-facing.
    ApiAppSaveFailed(String),
}

pub(super) fn handle_auth_event(event: AuthEvent, app: &mut App) {
    match event {
        AuthEvent::SupporterRefreshed(supporter) => {
            app.set_supporter(supporter);
        }
        AuthEvent::ApiAppSaved(client_id) => app.api_app_saved(client_id),
        AuthEvent::ApiAppSaveFailed(reason) => app.api_app_save_failed(reason),
    }
}

/// Check the typed-in API app with osu! and save it, off the UI thread. The
/// config tab marks the check as running before this starts, so a second
/// press can't race it; there's no handle to keep.
pub(super) fn spawn_api_app_save(
    credentials: ApiAppCredentials,
    tx: mpsc::UnboundedSender<AuthEvent>,
) {
    tokio::spawn(async move {
        let client = reqwest::Client::new();
        let ApiAppCredentials {
            client_id,
            client_secret,
        } = credentials;
        let event = match auth::save_app_credentials(&client, &client_id, &client_secret).await {
            Ok(()) => AuthEvent::ApiAppSaved(client_id.trim().to_owned()),
            Err(err) => {
                warn!(error = %err, "saving the osu! API app failed");
                AuthEvent::ApiAppSaveFailed(err.to_string())
            }
        };
        let _ = tx.send(event);
    });
}

/// Re-probe `/me` for the supporter flag of a session that was ALREADY logged in
/// when the app started. Without this the flag is whatever the last completed
/// login wrote: a token stored before the field existed carries no answer at all
/// and reads as not-a-supporter forever, and an account that gained or lost
/// supporter since keeps the stale one — in both cases with no in-app way out
/// short of a full logout/login.
///
/// Fire-and-forget: no stored handle, no await on the startup path, and a probe
/// that answers nothing sends no event, so startup is unaffected by a failure.
pub(super) fn spawn_supporter_refresh_task(tx: mpsc::UnboundedSender<AuthEvent>) {
    tokio::spawn(async move {
        // An API-app token has no user, so `/me` would only 401.
        let Some(mut stored) = auth::load().filter(auth::StoredAuth::is_user_login) else {
            return;
        };
        let client = reqwest::Client::new();
        // `/me` on an expired token 401s and carries no supporter data, so a
        // session idle past the token lifetime would learn nothing. A refresh
        // failure is not fatal — the probe below then just answers nothing.
        if let Err(err) = auth::ensure_valid(&client, &mut stored).await {
            debug!(error = %err, "token refresh before the supporter probe failed");
        }
        if let Some(supporter) = auth::refresh_supporter_status(&client, &mut stored).await {
            let _ = tx.send(AuthEvent::SupporterRefreshed(supporter));
        }
    });
}

#[cfg(test)]
#[path = "../../../tests/unit/runtime_auth.rs"]
mod tests;
