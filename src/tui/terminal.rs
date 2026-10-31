use crate::app::runtime::InputEvent;
use crate::tui::bg;
use crossterm::event::{
    self, DisableBracketedPaste, EnableBracketedPaste, Event as CrosstermEvent, KeyEventKind,
    KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use crossterm::execute;
use ratatui::{DefaultTerminal, style::Color};
use std::{
    io::{self, Write},
    thread,
    time::Duration,
};
use tokio::sync::mpsc;

pub type TuiTerminal = DefaultTerminal;

/// Enter the alternate screen + raw mode and layer the input seams + OSC-11
/// background on top.
///
/// [`ratatui::try_init`] enables raw mode, enters the alternate screen, and
/// installs a panic hook that restores the terminal before the default hook
/// runs. That hook only does `disable_raw_mode` + `LeaveAlternateScreen`,
/// though — bracketed paste, the kitty keyboard-enhancement flags, and the
/// OSC-11 background override are seams ratatui's lifecycle doesn't manage, so
/// they're set afterwards and need teardown of their own on every exit path.
///
/// Two exit paths are covered here:
/// - **panic**: we chain a hook *on top of* ratatui's. `take_hook` grabs
///   ratatui's freshly-installed hook, then `set_hook` installs a closure that
///   first reverses the extra escapes ([`teardown_extra_escapes`]) and then
///   calls ratatui's hook (which restores raw mode + the main screen). Chaining
///   keeps ratatui's restore behaviour intact while adding ours in front, so a
///   crash doesn't leak `DISAMBIGUATE_ESCAPE_CODES`, bracketed-paste mode, or
///   the forced background into the user's shell.
/// - **return / `?` early-return**: the caller holds a [`TerminalGuard`] whose
///   `Drop` runs the same teardown unconditionally.
///
/// `PushKeyboardEnhancementFlags` disambiguates ctrl+backspace from ctrl+h on
/// terminals that support it; the manual ctrl+h shim in `handle_key` stays as
/// the fallback for terminals that ignore the flags.
pub fn setup_terminal() -> io::Result<TuiTerminal> {
    let terminal = ratatui::try_init()?;

    // Chain on top of ratatui's panic hook: reverse our extra escapes first,
    // then defer to ratatui's hook (restore raw mode + main screen, then the
    // default hook). Best-effort — a panicking process must not panic again in
    // the hook, so teardown swallows io errors.
    let ratatui_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        teardown_extra_escapes();
        ratatui_hook(info);
    }));

    let _ = execute!(io::stdout(), EnableBracketedPaste);
    let _ = execute!(
        io::stdout(),
        PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
    );
    let _ = set_terminal_bg(&mut io::stdout(), bg());
    Ok(terminal)
}

/// Reverse the extra escapes layered by [`setup_terminal`]: pop the
/// keyboard-enhancement flags, disable bracketed paste, reset the OSC-11
/// background. Best-effort and **idempotent** — every step is a no-op when its
/// state was never set, so running it twice (panic hook *and* [`TerminalGuard`]
/// drop both firing during an unwind) is harmless. Does **not** touch raw mode
/// or the alternate screen; that stays with [`ratatui::restore`].
fn teardown_extra_escapes() {
    let _ = execute!(io::stdout(), PopKeyboardEnhancementFlags);
    let _ = execute!(io::stdout(), DisableBracketedPaste);
    let _ = reset_terminal_bg(&mut io::stdout());
}

/// RAII teardown for the terminal seams [`setup_terminal`] layers on.
///
/// Constructed in the runtime loop right after setup; its `Drop` runs
/// [`teardown_extra_escapes`] then [`ratatui::restore`] so the terminal is
/// reset on **every** non-panic exit — normal return *and* any `?`
/// early-return that skips the tail of `run`. The panic path is handled
/// separately by the chained hook in [`setup_terminal`]; teardown being
/// idempotent means an unwind that runs both is safe.
///
/// It doesn't own the [`TuiTerminal`] — the runtime loop still needs `&mut`
/// access to draw — it just owns the teardown obligation.
pub struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        teardown_extra_escapes();
        ratatui::restore();
    }
}

pub fn set_terminal_bg<W: Write>(out: &mut W, color: Color) -> io::Result<()> {
    if let Color::Rgb(r, g, b) = color {
        write!(out, "\x1b]11;rgb:{r:02x}/{g:02x}/{b:02x}\x1b\\")?;
        out.flush()?;
    }
    Ok(())
}

pub fn reset_terminal_bg<W: Write>(out: &mut W) -> io::Result<()> {
    write!(out, "\x1b]111\x1b\\")?;
    out.flush()
}

#[cfg(test)]
#[path = "../../tests/unit/tui_terminal.rs"]
mod tests;

pub fn spawn_input_thread(tx: mpsc::UnboundedSender<InputEvent>) -> Option<thread::JoinHandle<()>> {
    let tick_rate = Duration::from_millis(50);
    thread::Builder::new()
        .name("osu-shelf-input".into())
        .spawn(move || {
            loop {
                if event::poll(tick_rate).unwrap_or(false) {
                    match event::read() {
                        // Forward Press AND Repeat so a held ↑/↓ keeps scrolling
                        // (terminals with the kitty protocol emit Repeat events).
                        Ok(CrosstermEvent::Key(key))
                            if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) =>
                        {
                            if tx.send(InputEvent::Key(key)).is_err() {
                                break;
                            }
                        }
                        Ok(CrosstermEvent::Paste(text)) => {
                            if tx.send(InputEvent::Paste(text)).is_err() {
                                break;
                            }
                        }
                        Ok(CrosstermEvent::Resize(_, _)) => {
                            if tx.send(InputEvent::Resize).is_err() {
                                break;
                            }
                        }
                        Ok(_) => {}
                        Err(_) => break,
                    }
                } else if tx.send(InputEvent::Tick).is_err() {
                    break;
                }
            }
        })
        .ok()
}
