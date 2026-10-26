use crate::{
    app::runtime::ProbeResult,
    app::{ConfigField, ConfigTab},
    config::{LogFormat, LogLevel, RetryFailedOnDownload, ThemeMode},
    download::ArchiveValidation,
};
use ratatui::{Frame, layout::Rect, style::Stylize, text::Span};
use std::collections::HashMap;

use super::widgets;
use super::{HELP_CUSTOM_MIRROR, danger, mirror_label, success, text_dim};
use osu_downloader::MirrorKind;

const PANEL_TITLE: &str = " CONFIG ";

const SECTION_API: &str = "osu! api";
const SECTION_DISPLAY: &str = "display";
const SECTION_DOWNLOAD: &str = "download";
const SECTION_MIRRORS: &str = "mirrors";
const SECTION_LOGGING: &str = "logging";
const SECTION_UPDATE: &str = "updates";

/// The API app's two inputs share one label column so their values line up.
const API_LABEL_WIDTH: usize = "client secret".len();
const LABEL_API_SAVE: &str = "check & save";
const LABEL_API_CHECKING: &str = "checking with osu!…";
const LABEL_API_REMOVE: &str = "remove saved app";
const HELP_API_REGISTER: &str = "make one at osu.ppy.sh/home/account/edit → OAuth → new application; leave the callback URL blank";

const LABEL_THEME: &str = "theme";
const LABEL_VIM_KEYS: &str = "vim keys";
const LABEL_JUMP_TO_DOWNLOADS: &str = "jump to downloads on start";
const HELP_VIM_KEYS: &str = "[hjkl] move · [gg]/[G] top/bottom · [⌃d]/[⌃u] page · [i]/[a] edit";
const HELP_JUMP_TO_DOWNLOADS: &str = "auto switch focus to the new download";

const LABEL_VIDEO: &str = "video";
const LABEL_VERIFY_INTEGRITY: &str = "verify .osz integrity";
const LABEL_RETRY_FAILED: &str = "retry failed mapsets";
const LABEL_AUTO_DEFER_RATE_LIMITED: &str = "auto-defer rate-limited";
const LABEL_SKIP_IMPORTED: &str = "skip already imported";
const LABEL_LOGGING_ENABLED: &str = "enable logging";
const LABEL_LOGGING_LEVEL: &str = "log level";
const LABEL_LOGGING_FORMAT: &str = "log format";
const LABEL_AUTO_UPDATE: &str = "auto-update";
const LABEL_PRERELEASES: &str = "prerelease channel";

const THEME_MODE_LABELS: &[&str] = &["full", "compatible"];

const LOG_LEVELS: &[&str] = &["error", "warn", "info", "debug", "trace"];
const LOG_FORMATS: &[&str] = &["compact", "pretty"];
const ARCHIVE_VALIDATION_LABELS: &[&str] = &["off", "basic", "strict"];
const RETRY_FAILED_LABELS: &[&str] = &["ask", "yes", "no"];

/// State-specific hint for the archive-validation cycle: each describes only
/// what the currently selected mode does.
fn archive_validation_help(mode: ArchiveValidation) -> &'static str {
    match mode {
        ArchiveValidation::Off => "only checks the file is not empty",
        ArchiveValidation::Magic => "verifies archive headers",
        ArchiveValidation::Eocd => {
            "also verifies eocd footer; turn off if many mapsets fail validation"
        }
    }
}

/// State-specific hint for the auto-update toggle: each describes what happens
/// on the next launch in the current state.
fn auto_update_help(auto: bool) -> &'static str {
    if auto {
        "download & install update on launch"
    } else {
        "notify only; press [u] for changelog & update"
    }
}

/// State-specific hint for the prerelease-channel toggle: describes what the
/// next update check will offer in the current state.
fn prereleases_help(on: bool) -> &'static str {
    if on {
        "include pre-release builds"
    } else {
        "stable releases only"
    }
}

/// State-specific hint for the retry-failed cycle: each describes only what the
/// currently selected mode does.
fn retry_failed_help(mode: RetryFailedOnDownload) -> &'static str {
    match mode {
        RetryFailedOnDownload::Ask => "prompts before each download",
        RetryFailedOnDownload::Yes => "always retries failed mapsets",
        RetryFailedOnDownload::No => "never retries failed mapsets",
    }
}

pub fn render(
    frame: &mut Frame,
    area: Rect,
    form: &ConfigTab,
    editing: bool,
    library_db_hint: &str,
    mirror_latency: &HashMap<MirrorKind, Option<ProbeResult>>,
) {
    let show_chrome = area.height >= super::COMPACT_HEIGHT;
    let items = build_config_items(
        form,
        show_chrome,
        editing,
        library_db_hint,
        mirror_latency,
        widgets::panel_content_width(area),
    );

    let label_width = if form.focus.is_api_app() {
        API_LABEL_WIDTH
    } else {
        0
    };
    let cursor_col = editing
        .then(|| {
            form.focused_input()
                .map(|f| widgets::input_cursor_col(f, label_width))
        })
        .flatten();
    let (items, focused_index) = items.into_parts();
    widgets::render_scrollable_panel(
        frame,
        area,
        PANEL_TITLE,
        None,
        items,
        focused_index,
        true,
        cursor_col,
        true,
        true,
        &form.list_offset,
    )
}

/// What the API app section says about the saved app, under its buttons.
fn api_app_status(form: &ConfigTab) -> String {
    match &form.api_app {
        Some(client_id) => format!("saved: app {client_id} · find searches osu! with it"),
        None => "not set · find's osu! route needs your own API app".to_string(),
    }
}

/// The osu! API app rows: two inputs (the secret masked), the save button and,
/// while an app is saved, the remove button. Field order matches
/// [`ConfigTab::api_app_fields`].
fn push_api_app_items(
    items: &mut widgets::FormItems<ConfigField>,
    form: &ConfigTab,
    show_chrome: bool,
    editing: bool,
) {
    let focus = form.focus;
    items.push_focusable(
        ConfigField::ApiClientId,
        widgets::input_item(
            &form.api_client_id,
            focus == ConfigField::ApiClientId,
            editing,
            API_LABEL_WIDTH,
        ),
    );
    items.push_focusable(
        ConfigField::ApiClientSecret,
        widgets::masked_input_item(
            &form.api_client_secret,
            focus == ConfigField::ApiClientSecret,
            editing,
            API_LABEL_WIDTH,
        ),
    );
    if show_chrome
        && matches!(
            focus,
            ConfigField::ApiClientId | ConfigField::ApiClientSecret
        )
    {
        items.push(widgets::help_item(HELP_API_REGISTER));
    }
    let save_label = if form.api_app_checking {
        LABEL_API_CHECKING
    } else {
        LABEL_API_SAVE
    };
    items.push_focusable(
        ConfigField::ApiAppSave,
        widgets::button_item(
            save_label,
            focus == ConfigField::ApiAppSave,
            form.api_app_ready(),
            widgets::ButtonProminence::Primary,
        ),
    );
    if form.api_app.is_some() {
        items.push_focusable(
            ConfigField::ApiAppRemove,
            widgets::button_item(
                LABEL_API_REMOVE,
                focus == ConfigField::ApiAppRemove,
                true,
                widgets::ButtonProminence::Secondary,
            ),
        );
    }
    if show_chrome {
        items.push(widgets::help_item(api_app_status(form)));
    }
}

/// Latency suffix appended to a built-in mirror row, mirroring
/// [`HomeTab::mirror_latency`](crate::app::HomeTab) semantics: outer `None` =
/// never probed (no suffix), `Some(None)` = probe in flight (`…`),
/// `Some(Some(_))` = a result.
fn latency_span(latency: Option<Option<ProbeResult>>) -> Option<Span<'static>> {
    match latency? {
        None => Some("  …".fg(text_dim())),
        Some(ProbeResult::Ms(ms)) => {
            let mut s = String::with_capacity(10);
            s.push_str("  ");
            s.push_str(&ms.to_string());
            s.push_str("ms");
            Some(s.fg(success()))
        }
        Some(ProbeResult::Timeout) => Some("  timeout".fg(danger())),
        Some(ProbeResult::Error) => Some("  —".fg(danger())),
    }
}

/// Builds the config form item list. `show_chrome` gates the decorative section
/// headers, spacers, and focus-conditional help lines; the focusable field rows
/// are identical in both modes so the field list lives here once.
///
/// Compact mode (`show_chrome == false`) strips only that chrome — every field
/// stays focusable and navigable.
fn build_config_items(
    form: &ConfigTab,
    show_chrome: bool,
    editing: bool,
    library_db_hint: &str,
    mirror_latency: &HashMap<MirrorKind, Option<ProbeResult>>,
    width: u16,
) -> widgets::FormItems<ConfigField> {
    let focus = form.focus;
    let active_section = focus_section(focus);
    let mut items = widgets::FormItems::new(focus);

    if show_chrome {
        items.push(widgets::section_header(
            SECTION_API,
            active_section == Some(SECTION_API),
        ));
    }
    push_api_app_items(&mut items, form, show_chrome, editing);
    if show_chrome {
        items.push(widgets::spacer());
        items.push(widgets::section_header(
            SECTION_DISPLAY,
            active_section == Some(SECTION_DISPLAY),
        ));
    }

    items.push_focusable(
        ConfigField::Theme,
        widgets::cycle_item(
            LABEL_THEME,
            THEME_MODE_LABELS,
            theme_mode_label(form.theme),
            focus == ConfigField::Theme,
            0,
            width,
        ),
    );
    items.push_focusable(
        ConfigField::VimKeys,
        widgets::row_item(
            LABEL_VIM_KEYS,
            None,
            form.vim_keys,
            focus == ConfigField::VimKeys,
            0,
        ),
    );
    if show_chrome && focus == ConfigField::VimKeys {
        items.push(widgets::help_item_keyed(HELP_VIM_KEYS));
    }
    items.push_focusable(
        ConfigField::JumpToDownloads,
        widgets::row_item(
            LABEL_JUMP_TO_DOWNLOADS,
            None,
            form.jump_to_downloads,
            focus == ConfigField::JumpToDownloads,
            0,
        ),
    );
    if show_chrome && focus == ConfigField::JumpToDownloads {
        items.push(widgets::help_item(HELP_JUMP_TO_DOWNLOADS));
    }
    if show_chrome {
        items.push(widgets::spacer());
        items.push(widgets::section_header(
            SECTION_MIRRORS,
            active_section == Some(SECTION_MIRRORS),
        ));
    }

    // Rows follow the configured try-order (`ordered_mirror_rows`), matching the
    // nav order and the pipeline so what the user reorders is what gets tried.
    for (kind, field, on) in form.ordered_mirror_rows() {
        // Host is an informational hint, not a configurable value, so it is NOT
        // column-aligned (label_width 0) — it trails the mirror name.
        let latency = mirror_latency.get(&kind).copied();
        let item = widgets::row_item_with_suffix(
            mirror_label(kind),
            Some(kind.host()),
            on,
            focus == field,
            latency_span(latency),
            0,
        );
        items.push_focusable(field, item);
    }
    for (idx, row) in form.custom_mirrors.rows().iter().enumerate() {
        let field = ConfigField::MirrorCustomUrl(idx);
        let focused = focus == field;
        items.push_focusable(field, widgets::input_item(row, focused, editing, 0));
        if show_chrome && focused {
            items.push(widgets::help_item(HELP_CUSTOM_MIRROR));
        }
    }
    if show_chrome {
        items.push(widgets::spacer());
        items.push(widgets::section_header(
            SECTION_DOWNLOAD,
            active_section == Some(SECTION_DOWNLOAD),
        ));
    }

    items.push_focusable(
        ConfigField::DownloadVideo,
        widgets::row_item(
            LABEL_VIDEO,
            None,
            form.video,
            focus == ConfigField::DownloadVideo,
            0,
        ),
    );
    items.push_focusable(
        ConfigField::DownloadThreads,
        widgets::stepper_item(
            form.threads.label,
            form.resolved_threads(),
            form.default_threads,
            focus == ConfigField::DownloadThreads,
            0,
        ),
    );
    items.push_focusable(
        ConfigField::DownloadArchiveValidation,
        widgets::cycle_item(
            LABEL_VERIFY_INTEGRITY,
            ARCHIVE_VALIDATION_LABELS,
            archive_validation_label(form.archive_validation),
            focus == ConfigField::DownloadArchiveValidation,
            0,
            width,
        ),
    );
    if show_chrome && focus == ConfigField::DownloadArchiveValidation {
        items.push(widgets::help_item(archive_validation_help(
            form.archive_validation,
        )));
    }
    items.push_focusable(
        ConfigField::RetryFailedOnDownload,
        widgets::cycle_item(
            LABEL_RETRY_FAILED,
            RETRY_FAILED_LABELS,
            retry_failed_label(form.retry_failed_on_download),
            focus == ConfigField::RetryFailedOnDownload,
            0,
            width,
        ),
    );
    if show_chrome && focus == ConfigField::RetryFailedOnDownload {
        items.push(widgets::help_item(retry_failed_help(
            form.retry_failed_on_download,
        )));
    }
    items.push_focusable(
        ConfigField::DownloadSkipAlreadyImported,
        widgets::row_item(
            LABEL_SKIP_IMPORTED,
            None,
            form.skip_already_imported,
            focus == ConfigField::DownloadSkipAlreadyImported,
            0,
        ),
    );
    if show_chrome && focus == ConfigField::DownloadSkipAlreadyImported {
        items.push(widgets::help_item(format!("checks {library_db_hint}")));
    }
    items.push_focusable(
        ConfigField::DownloadAutoSkipRateLimited,
        widgets::row_item(
            LABEL_AUTO_DEFER_RATE_LIMITED,
            None,
            form.auto_skip_rate_limited,
            focus == ConfigField::DownloadAutoSkipRateLimited,
            0,
        ),
    );
    items.push_focusable(
        ConfigField::DownloadRateLimitSkipSecs,
        widgets::input_item(
            &form.rate_limit_skip_secs,
            focus == ConfigField::DownloadRateLimitSkipSecs,
            editing,
            0,
        ),
    );
    if show_chrome {
        items.push(widgets::spacer());
        items.push(widgets::section_header(
            SECTION_LOGGING,
            active_section == Some(SECTION_LOGGING),
        ));
    }

    items.push_focusable(
        ConfigField::LoggingEnabled,
        widgets::row_item(
            LABEL_LOGGING_ENABLED,
            None,
            form.logging_enabled,
            focus == ConfigField::LoggingEnabled,
            0,
        ),
    );
    items.push_focusable(
        ConfigField::LoggingLevel,
        widgets::cycle_item(
            LABEL_LOGGING_LEVEL,
            LOG_LEVELS,
            log_level_label(form.logging_level),
            focus == ConfigField::LoggingLevel,
            0,
            width,
        ),
    );
    items.push_focusable(
        ConfigField::LoggingFormat,
        widgets::cycle_item(
            LABEL_LOGGING_FORMAT,
            LOG_FORMATS,
            log_format_label(form.logging_format),
            focus == ConfigField::LoggingFormat,
            0,
            width,
        ),
    );
    items.push_focusable(
        ConfigField::LoggingDirectory,
        widgets::input_item(
            &form.logging_dir,
            focus == ConfigField::LoggingDirectory,
            editing,
            0,
        ),
    );

    if show_chrome {
        items.push(widgets::spacer());
        items.push(widgets::section_header(
            SECTION_UPDATE,
            active_section == Some(SECTION_UPDATE),
        ));
    }
    items.push_focusable(
        ConfigField::AutoUpdate,
        widgets::row_item(
            LABEL_AUTO_UPDATE,
            None,
            form.auto_update,
            focus == ConfigField::AutoUpdate,
            0,
        ),
    );
    if show_chrome && focus == ConfigField::AutoUpdate {
        items.push(widgets::help_item_keyed(auto_update_help(form.auto_update)));
    }
    items.push_focusable(
        ConfigField::Prereleases,
        widgets::row_item(
            LABEL_PRERELEASES,
            None,
            form.prereleases,
            focus == ConfigField::Prereleases,
            0,
        ),
    );
    if show_chrome && focus == ConfigField::Prereleases {
        items.push(widgets::help_item(prereleases_help(form.prereleases)));
    }

    items
}

/// The section a focused field belongs to, driving the active-section header
/// cue.
fn focus_section(field: ConfigField) -> Option<&'static str> {
    use ConfigField::*;
    Some(match field {
        ApiClientId | ApiClientSecret | ApiAppSave | ApiAppRemove => SECTION_API,
        Theme | VimKeys | JumpToDownloads => SECTION_DISPLAY,
        MirrorOsuDirect | MirrorNerinyan | MirrorSayobot | MirrorNekoha | MirrorBeatconnect
        | MirrorOsudl | MirrorCatboy | MirrorHinamizawa | MirrorNzbasic | MirrorCustomUrl(_) => {
            SECTION_MIRRORS
        }
        DownloadThreads
        | DownloadVideo
        | DownloadArchiveValidation
        | RetryFailedOnDownload
        | DownloadAutoSkipRateLimited
        | DownloadRateLimitSkipSecs
        | DownloadSkipAlreadyImported => SECTION_DOWNLOAD,
        LoggingEnabled | LoggingLevel | LoggingFormat | LoggingDirectory => SECTION_LOGGING,
        AutoUpdate | Prereleases => SECTION_UPDATE,
    })
}

fn log_level_label(level: LogLevel) -> &'static str {
    match level {
        LogLevel::Error => "error",
        LogLevel::Warn => "warn",
        LogLevel::Info => "info",
        LogLevel::Debug => "debug",
        LogLevel::Trace => "trace",
    }
}

fn log_format_label(format: LogFormat) -> &'static str {
    match format {
        LogFormat::Compact => "compact",
        LogFormat::Pretty => "pretty",
    }
}

fn archive_validation_label(mode: ArchiveValidation) -> &'static str {
    match mode {
        ArchiveValidation::Off => "off",
        ArchiveValidation::Magic => "basic",
        ArchiveValidation::Eocd => "strict",
    }
}

fn retry_failed_label(mode: RetryFailedOnDownload) -> &'static str {
    match mode {
        RetryFailedOnDownload::Ask => "ask",
        RetryFailedOnDownload::Yes => "yes",
        RetryFailedOnDownload::No => "no",
    }
}

fn theme_mode_label(mode: ThemeMode) -> &'static str {
    match mode {
        ThemeMode::Full => "full",
        ThemeMode::Compatible => "compatible",
    }
}
