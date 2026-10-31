/// Quit confirmation toast behaviour.
///
/// First `q` shows a toast and does NOT quit; second `q` while the toast is
/// visible quits. `esc` is back-only: it never arms the toast and never quits,
/// but it does cancel a `q`-armed toast. Any other key while the toast is
/// visible dismisses it and falls through.
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
use osu_shelf::{
    app::{App, AppCommand, HomeField},
    config::Config,
};

fn make_app() -> App {
    App::new(Config::default())
}

/// App focused on a non-text home field, where `q` triggers the quit toast
/// (on a text field `q` types instead).
fn make_app_quittable() -> App {
    let mut app = make_app();
    app.home.focus = HomeField::Video;
    app
}

fn press(code: KeyCode) -> KeyEvent {
    KeyEvent {
        code,
        modifiers: KeyModifiers::empty(),
        kind: KeyEventKind::Press,
        state: KeyEventState::empty(),
    }
}

// ── first q shows toast, does not quit ───────────────────────────────────────

#[test]
fn first_q_sets_quit_prompt_no_downloads() {
    let mut app = make_app_quittable();
    let cmd = app.handle_key(press(KeyCode::Char('q')));
    assert!(cmd.is_none(), "first q must not quit");
    assert!(app.home.quit_prompt, "first q must raise the quit toast");
}

#[test]
fn esc_does_not_arm_quit_prompt() {
    let mut app = make_app();
    let cmd = app.handle_key(press(KeyCode::Esc));
    assert!(cmd.is_none(), "esc must not quit");
    assert!(
        !app.home.quit_prompt,
        "esc is back-only; it must never raise the quit toast"
    );
}

// ── second q while toast visible quits ───────────────────────────────────────

#[test]
fn second_q_quits() {
    let mut app = make_app_quittable();
    app.handle_key(press(KeyCode::Char('q')));
    assert!(app.home.quit_prompt);
    let cmd = app.handle_key(press(KeyCode::Char('q')));
    assert!(matches!(cmd, Some(AppCommand::Quit)));
    assert!(!app.home.quit_prompt, "quit_prompt must be cleared on quit");
}

#[test]
fn esc_cancels_q_armed_quit_prompt_without_quitting() {
    let mut app = make_app_quittable();
    app.handle_key(press(KeyCode::Char('q')));
    assert!(app.home.quit_prompt, "first q must arm the quit toast");
    let cmd = app.handle_key(press(KeyCode::Esc));
    assert!(
        !matches!(cmd, Some(AppCommand::Quit)),
        "esc must not quit a q-armed prompt: {cmd:?}"
    );
    assert!(
        !app.home.quit_prompt,
        "esc must cancel the armed quit toast"
    );
}

// ── unrelated key clears toast and falls through ──────────────────────────────

#[test]
fn arrow_clears_toast_and_switches_tab() {
    let mut app = make_app_quittable();
    app.handle_key(press(KeyCode::Char('q')));
    assert!(app.home.quit_prompt);
    let tab_before = app.active_tab();
    // ←/→ switch screens (tab no longer does); a non-text focus is required so
    // the arrow switches a tab instead of moving a caret.
    app.home.focus = HomeField::Video;
    app.handle_key(press(KeyCode::Right));
    assert!(
        !app.home.quit_prompt,
        "switching screens must clear the quit toast"
    );
    assert_ne!(
        app.active_tab(),
        tab_before,
        "→ must still switch the active tab"
    );
}

#[test]
fn any_char_key_clears_toast_without_quitting() {
    let mut app = make_app_quittable();
    app.handle_key(press(KeyCode::Char('q')));
    assert!(app.home.quit_prompt);
    // pressing a letter key (not q) should clear the toast
    let cmd = app.handle_key(press(KeyCode::Char('a')));
    assert!(!app.home.quit_prompt, "non-quit key must clear the toast");
    assert!(
        !matches!(cmd, Some(AppCommand::Quit)),
        "non-quit key after toast must not quit: {cmd:?}"
    );
}

// ── modal takes priority over quit toast ─────────────────────────────────────

#[test]
fn q_with_help_open_closes_modal_not_toast() {
    let mut app = make_app();
    app.help_open = true;
    let cmd = app.handle_key(press(KeyCode::Char('q')));
    assert!(!app.help_open, "q must close the help modal");
    assert!(
        !app.home.quit_prompt,
        "quit toast must not be raised when a modal was closed"
    );
    assert!(cmd.is_none());
}

#[test]
fn q_after_modal_closed_then_shows_toast() {
    let mut app = make_app_quittable();
    // open and close modal, then press q again
    app.help_open = true;
    app.handle_key(press(KeyCode::Char('q'))); // closes modal
    assert!(!app.help_open);
    assert!(!app.home.quit_prompt);
    // now q with no modal → toast
    let cmd = app.handle_key(press(KeyCode::Char('q')));
    assert!(
        cmd.is_none(),
        "q after modal closes must show toast, not quit"
    );
    assert!(
        app.home.quit_prompt,
        "toast must be raised after modal is gone"
    );
}
