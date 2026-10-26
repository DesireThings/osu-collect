use crate::{
    app::runtime::{MirrorProbeEvent, ProbeResult},
    app::{App, AppCommand, HomeTab, Tab},
    config::Config,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use osu_downloader::MirrorKind;

// handle_mirror_probe_event is not re-exported; import directly via the module
// (accessible because this file is compiled inside mirror_probe via #[path]).
use super::{handle_mirror_probe_event, probe_client};
use crate::config::constants::USER_AGENT;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn char_key(ch: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE)
}

// ── MirrorProbeEvent → HomeTab::mirror_latency ────────────────────────────────

/// `Started` marks all built-in mirrors as in-flight (None).
#[test]
fn probe_started_marks_all_mirrors_inflight() {
    let config = Config::default();
    let mut home = HomeTab::new(&config);
    assert!(home.mirror_latency.is_empty());

    handle_mirror_probe_event(MirrorProbeEvent::Started, &mut home);

    for kind in MirrorKind::BUILTINS {
        assert_eq!(
            home.mirror_latency.get(kind).copied(),
            Some(None),
            "{kind:?} should be in-flight after Started"
        );
    }
}

/// `Result` stores the ProbeResult for the given mirror.
#[test]
fn probe_result_event_stores_ms_result() {
    let config = Config::default();
    let mut home = HomeTab::new(&config);

    handle_mirror_probe_event(
        MirrorProbeEvent::Result {
            kind: MirrorKind::Nerinyan,
            result: ProbeResult::Ms(123),
        },
        &mut home,
    );

    assert_eq!(
        home.mirror_latency.get(&MirrorKind::Nerinyan).copied(),
        Some(Some(ProbeResult::Ms(123)))
    );
}

/// `Result` with Timeout stores Timeout.
#[test]
fn probe_result_event_stores_timeout() {
    let config = Config::default();
    let mut home = HomeTab::new(&config);

    handle_mirror_probe_event(
        MirrorProbeEvent::Result {
            kind: MirrorKind::OsuDirect,
            result: ProbeResult::Timeout,
        },
        &mut home,
    );

    assert_eq!(
        home.mirror_latency.get(&MirrorKind::OsuDirect).copied(),
        Some(Some(ProbeResult::Timeout))
    );
}

/// `Result` with Error stores Error.
#[test]
fn probe_result_event_stores_error() {
    let config = Config::default();
    let mut home = HomeTab::new(&config);

    handle_mirror_probe_event(
        MirrorProbeEvent::Result {
            kind: MirrorKind::Sayobot,
            result: ProbeResult::Error,
        },
        &mut home,
    );

    assert_eq!(
        home.mirror_latency.get(&MirrorKind::Sayobot).copied(),
        Some(Some(ProbeResult::Error))
    );
}

/// Stale probe event for a mirror that is not enabled is accepted and stored.
/// The user might enable the mirror later and see the latency result.
#[test]
fn probe_result_stored_even_when_mirror_disabled() {
    let config = Config::default();
    let mut home = HomeTab::new(&config);
    // Explicitly disable Nekoha
    home.nekoha = false;

    handle_mirror_probe_event(
        MirrorProbeEvent::Result {
            kind: MirrorKind::Nekoha,
            result: ProbeResult::Ms(42),
        },
        &mut home,
    );

    // Still stored — enabling the mirror later would show the value.
    assert_eq!(
        home.mirror_latency.get(&MirrorKind::Nekoha).copied(),
        Some(Some(ProbeResult::Ms(42)))
    );
}

/// Multiple results arrive; each overwrites only its own slot.
#[test]
fn probe_results_are_independent_per_mirror() {
    let config = Config::default();
    let mut home = HomeTab::new(&config);

    handle_mirror_probe_event(MirrorProbeEvent::Started, &mut home);
    handle_mirror_probe_event(
        MirrorProbeEvent::Result {
            kind: MirrorKind::Nerinyan,
            result: ProbeResult::Ms(50),
        },
        &mut home,
    );

    // Nerinyan has a result; others are still in-flight.
    assert_eq!(
        home.mirror_latency.get(&MirrorKind::Nerinyan).copied(),
        Some(Some(ProbeResult::Ms(50)))
    );
    assert_eq!(
        home.mirror_latency.get(&MirrorKind::OsuDirect).copied(),
        Some(None),
        "OsuDirect should still be in-flight"
    );
}

// ── r keypress emits ProbeMirrors only on home tab ────────────────────────────

/// `r` on the home tab (non-text-input focused) emits ProbeMirrors.
#[test]
fn r_on_home_tab_emits_probe_mirrors() {
    let mut app = App::new(Config::default());
    app.active_tab = Tab::Home;
    // Default focus is Collection (text input) — move to a toggle field.
    app.home.focus = crate::app::HomeField::Video;

    let cmd = app.handle_key(char_key('r'));

    assert!(
        matches!(cmd, Some(AppCommand::ProbeMirrors)),
        "expected ProbeMirrors on r, got {cmd:?}"
    );
}

/// `r` while EDITING a text input types the char instead of probing mirrors.
/// (Outside edit mode `r` is a global hotkey that probes — see the next test.)
#[test]
fn r_while_editing_text_input_types_instead_of_probing() {
    let mut app = App::new(Config::default());
    app.active_tab = Tab::Home;
    app.home.focus = crate::app::HomeField::Collection;
    app.editing = true; // edit mode: keys type into the field

    let cmd = app.handle_key(char_key('r'));

    assert!(
        !matches!(cmd, Some(AppCommand::ProbeMirrors)),
        "ProbeMirrors must not fire while editing a text input"
    );
    // The character should have been inserted.
    assert_eq!(app.home.collection.value, "r");
}

/// `r` on a text-input row that is selected-not-editing is a global hotkey and
/// probes mirror latency (edit mode is off until `enter` is pressed).
#[test]
fn r_on_selected_not_editing_text_input_probes() {
    let mut app = App::new(Config::default());
    app.active_tab = Tab::Home;
    app.home.focus = crate::app::HomeField::Collection;
    // editing defaults false

    let cmd = app.handle_key(char_key('r'));

    assert!(
        matches!(cmd, Some(AppCommand::ProbeMirrors)),
        "r must probe when the text input is selected-not-editing"
    );
    assert_eq!(
        app.home.collection.value, "",
        "r must not type when not editing"
    );
}

/// `r` on the update source does NOT emit ProbeMirrors (it rechecks failed maps).
#[test]
fn r_on_update_source_does_not_emit_probe_mirrors() {
    use crate::app::GetMapsSource;
    let mut app = App::new(Config::default());
    app.home.source = GetMapsSource::Update;

    let cmd = app.handle_key(char_key('r'));

    assert!(
        !matches!(cmd, Some(AppCommand::ProbeMirrors)),
        "ProbeMirrors must not fire on the update source"
    );
}

// ── tab activation triggers probe ─────────────────────────────────────────────

/// Switching to the home tab via Left arrow emits ProbeMirrors.
#[test]
fn switching_to_home_tab_emits_probe_mirrors() {
    let mut app = App::new(Config::default());
    // Start on the adjacent tab; Left lands on home.
    app.active_tab = Tab::Downloads;

    let cmd = app.handle_key(key(KeyCode::Left));

    assert_eq!(app.active_tab, Tab::Home);
    assert!(
        matches!(cmd, Some(AppCommand::ProbeMirrors)),
        "expected ProbeMirrors when switching to home, got {cmd:?}"
    );
}

#[tokio::test]
async fn probes_tell_the_mirror_which_app_is_asking() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = Vec::new();
        let mut buf = [0u8; 1024];
        while !request.windows(4).any(|w| w == b"\r\n\r\n") {
            let n = socket.read(&mut buf).await.unwrap();
            if n == 0 {
                break;
            }
            request.extend_from_slice(&buf[..n]);
        }
        socket
            .write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 0\r\nconnection: close\r\n\r\n")
            .await
            .unwrap();
        String::from_utf8_lossy(&request).into_owned()
    });

    probe_client().head(&url).send().await.unwrap();

    let request = server.await.unwrap();
    let user_agent = request.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        name.eq_ignore_ascii_case("user-agent")
            .then(|| value.trim())
    });
    assert_eq!(user_agent, Some(USER_AGENT));
}
