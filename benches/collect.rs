use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use std::sync::LazyLock;

// ── panel_block title formatting ─────────────────────────────────────────────
//
// src/tui/widgets.rs:panel_block — called on every TUI render frame.
// Three panels in the download view (overview, active, results) each call
// panel_block with a &'static str title.  At ~30 fps that is ≥90 calls/sec.
//
// Old shape (baseline, no longer in production):
//   format!(" {} ", title.to_uppercase())   — 2 allocations per call
//
// New shape: callers pass pre-uppercased, space-padded &'static str constants.
//   panel_block(" OVERVIEW ")               — zero allocations, pointer pass

fn bench_panel_block_title_format(c: &mut Criterion) {
    // Old pattern (baseline) — kept for before/after delta measurement.
    let lowercase_titles: &[&str] = &["overview", "active", "results", "config", "updates"];
    // New pattern — static constants as callers now supply them.
    let static_titles: &[&'static str] = &[
        " OVERVIEW ",
        " ACTIVE ",
        " RESULTS ",
        " CONFIG ",
        " UPDATES ",
    ];

    let mut group = c.benchmark_group("panel_block_title_format");

    // Baseline: old production pattern (2 allocations per call).
    for &title in lowercase_titles {
        group.bench_with_input(
            BenchmarkId::new("format_to_uppercase", title),
            title,
            |b, title| {
                b.iter(|| {
                    let s: String = format!(" {} ", black_box(title).to_uppercase());
                    black_box(s)
                })
            },
        );
    }

    // New shape: static constant pass-through — zero allocations, pointer return.
    for &title in static_titles {
        let key = title.trim();
        group.bench_with_input(
            BenchmarkId::new("static_constant", key),
            &title,
            |b, title| {
                b.iter(|| black_box(title));
            },
        );
    }

    group.finish();
}

// ── message_style to_lowercase ────────────────────────────────────────────────
//
// src/tui/widgets.rs:message_style — called once per active download slot per
// render frame (concurrent × ~30 fps). Old implementation:
//   let lower = message.to_lowercase();
//   if lower.contains("error") || lower.starts_with("failed") { ... }
// allocated on every call; the em-dash in "done — downloaded …" hit the UTF-8
// slow path, making that case 5× slower (123 ns vs ~22 ns).
//
// New shape: classify on BeatmapStage enum — zero string scan, zero alloc.

// Mirrors BeatmapStage without importing the binary crate.
#[derive(Clone, Copy)]
#[repr(u8)]
#[allow(dead_code)]
enum StageMock {
    Pending,
    Downloading,
    Verifying,
    Success,
    Skipped,
    Failed,
    Aborted,
}

fn bench_message_style_classify(c: &mut Criterion) {
    // Representative messages from active_download_item in production, paired
    // with the BeatmapStage the producer would have set at that point.
    let cases: &[(&str, &str, StageMock)] = &[
        (
            "downloading from nerinyan",
            "downloading",
            StageMock::Downloading,
        ),
        ("checking nerinyan", "checking", StageMock::Downloading),
        ("skipped: already exists", "skipped", StageMock::Skipped),
        (
            "done — downloaded from nerinyan",
            "done",
            StageMock::Success,
        ),
        ("failed: checksum mismatch", "failed", StageMock::Failed),
        (
            "network error: connection reset",
            "error",
            StageMock::Failed,
        ),
        (
            "verifying from osu!direct",
            "verifying",
            StageMock::Verifying,
        ),
        (
            "retrying nerinyan after connection reset (attempt 2/3)",
            "retrying",
            StageMock::Downloading,
        ),
    ];

    let mut group = c.benchmark_group("message_style_classify");

    for &(message, label, stage) in cases {
        // Baseline: old production pattern (to_lowercase allocates).
        group.bench_with_input(
            BenchmarkId::new("to_lowercase", label),
            message,
            |b, message| {
                b.iter(|| {
                    let lower = black_box(message).to_lowercase();
                    let result = if lower.contains("error") || lower.starts_with("failed") {
                        1u8
                    } else if lower.starts_with("done") {
                        2u8
                    } else if lower.starts_with("skipped") {
                        3u8
                    } else {
                        0u8
                    };
                    black_box(result)
                })
            },
        );

        // New pattern: stage enum match — zero alloc, zero string scan.
        group.bench_with_input(
            BenchmarkId::new("stage_match", label),
            &stage,
            |b, &stage| {
                b.iter(|| {
                    let result = match black_box(stage) {
                        StageMock::Failed | StageMock::Aborted => 1u8,
                        StageMock::Success => 2u8,
                        StageMock::Skipped => 3u8,
                        StageMock::Pending | StageMock::Downloading | StageMock::Verifying => 0u8,
                    };
                    black_box(result)
                })
            },
        );
    }

    group.finish();
}

// ── render_separator ──────────────────────────────────────────────────────────
//
// src/tui/widgets.rs:render_separator — called on every TUI render frame to
// draw a horizontal rule.  The current implementation allocates a fresh
// `String` via `"─".repeat(width)` on every call — ~30 allocs/sec at 30 fps.
//
// Baseline: `"─".repeat(n)` — 1 allocation per call, cost grows with width.
// Candidate: `String::with_capacity(n * 3)` + `extend(iter::repeat_n('─', n))`
//   or a lazily-built max-width buffer sliced to `n` — zero allocation on the
//   hot path.
//
// Bench inputs: representative terminal widths (80, 160, 220).

fn bench_render_separator(c: &mut Criterion) {
    let widths: &[usize] = &[80, 160, 220];
    let mut group = c.benchmark_group("render_separator");

    // Baseline: production pattern — fresh String allocation per call.
    for &w in widths {
        group.bench_with_input(BenchmarkId::new("repeat_alloc", w), &w, |b, &w| {
            b.iter(|| {
                let s: String = black_box("─").repeat(black_box(w));
                black_box(s);
            })
        });
    }

    // Candidate: build once with capacity, extend — still 1 alloc but
    // avoids the hidden realloc inside `repeat` when the multibyte char
    // is repeated into an undersized buffer.
    for &w in widths {
        group.bench_with_input(BenchmarkId::new("with_capacity_extend", w), &w, |b, &w| {
            b.iter(|| {
                let mut s = String::with_capacity(w * 3); // '─' is 3 bytes
                s.extend(std::iter::repeat_n('─', black_box(w)));
                black_box(s);
            })
        });
    }

    // Candidate: reuse a thread-local scratch buffer — zero alloc on reuse.
    for &w in widths {
        group.bench_with_input(BenchmarkId::new("reuse_scratch", w), &w, |b, &w| {
            let mut scratch = String::with_capacity(220 * 3);
            b.iter(|| {
                scratch.clear();
                scratch.extend(std::iter::repeat_n('─', black_box(w)));
                black_box(scratch.as_str());
            })
        });
    }

    // New shape: slice a pre-built LazyLock<String> — zero allocation on hot path.
    static H_LINE_BUF: LazyLock<String> = LazyLock::new(|| "─".repeat(256));
    for &w in widths {
        group.bench_with_input(BenchmarkId::new("static_slice", w), &w, |b, &w| {
            b.iter(|| {
                let end = black_box(w) * 3; // '─' is 3 bytes
                let s = &H_LINE_BUF.as_str()[..end];
                black_box(s);
            })
        });
    }

    group.finish();
}

// ── indeterminate_bar_spans ───────────────────────────────────────────────────
//
// src/tui/widgets.rs:indeterminate_bar_spans — called per active download slot
// per render frame for beatmapsets whose size is unknown.  Three `String`
// allocations per call: `"░".repeat(offset)` + `"█".repeat(segment)` +
// `"░".repeat(right)`.  With 4 active slots at 30 fps that is 360 allocs/sec.
//
// Baseline: 3× `char.repeat(n)` — up to 3 heap allocations per call.
// Candidate: 1× `String::with_capacity(width * 3)` filled in three passes —
//   single alloc regardless of segment count.  Strings are `'static`-lifetime
//   in `Span::styled`; the candidate measures the construction cost only.
//
// Bench inputs: (offset, segment, right) tuples matching BAR_WIDTH=16 travel.

fn bench_indeterminate_bar_spans(c: &mut Criterion) {
    // (left_empty, filled_segment, right_empty) — sum == bar_width
    let cases: &[(&str, usize, usize, usize)] = &[
        ("start", 0, 4, 12),
        ("mid", 6, 4, 6),
        ("end", 12, 4, 0),
        ("wide", 50, 4, 50), // wider terminal bar
    ];

    let mut group = c.benchmark_group("indeterminate_bar_spans");

    // Baseline: 3× repeat — current production code path.
    for &(label, left, seg, right) in cases {
        group.bench_with_input(
            BenchmarkId::new("three_repeat", label),
            &(left, seg, right),
            |b, &(left, seg, right)| {
                b.iter(|| {
                    let mut v: Vec<String> = Vec::with_capacity(3);
                    if black_box(left) > 0 {
                        v.push("░".repeat(black_box(left)));
                    }
                    v.push("█".repeat(black_box(seg)));
                    if black_box(right) > 0 {
                        v.push("░".repeat(black_box(right)));
                    }
                    black_box(v);
                })
            },
        );
    }

    // Candidate: single pre-sized String filled in three extend passes.
    for &(label, left, seg, right) in cases {
        group.bench_with_input(
            BenchmarkId::new("single_scratch", label),
            &(left, seg, right),
            |b, &(left, seg, right)| {
                b.iter(|| {
                    let total = black_box(left) + black_box(seg) + black_box(right);
                    let mut s = String::with_capacity(total * 3);
                    s.extend(std::iter::repeat_n('░', black_box(left)));
                    s.extend(std::iter::repeat_n('█', black_box(seg)));
                    s.extend(std::iter::repeat_n('░', black_box(right)));
                    black_box(s);
                })
            },
        );
    }

    // New shape: slice pre-built LazyLock<String> buffers — zero allocation.
    static SHADE_BUF: LazyLock<String> = LazyLock::new(|| "░".repeat(256));
    static BLOCK_BUF: LazyLock<String> = LazyLock::new(|| "█".repeat(256));
    for &(label, left, seg, right) in cases {
        group.bench_with_input(
            BenchmarkId::new("static_slice", label),
            &(left, seg, right),
            |b, &(left, seg, right)| {
                b.iter(|| {
                    let left_s = &SHADE_BUF.as_str()[..black_box(left) * 3];
                    let seg_s = &BLOCK_BUF.as_str()[..black_box(seg) * 3];
                    let right_s = &SHADE_BUF.as_str()[..black_box(right) * 3];
                    black_box((left_s, seg_s, right_s));
                })
            },
        );
    }

    group.finish();
}

// ── tab_titles ────────────────────────────────────────────────────────────────
//
// src/app/state.rs:tab_titles — called on every render frame to build the tab
// bar.  Current shape: `Vec<String>` + `"Home".to_string()` × 3 static tabs
// + `page.title.clone()` × N dynamic tabs.  At 30 fps with N=0 collections
// that is 90 static-string allocs/sec; with N=5 it is 240/sec.
//
// Baseline: current production pattern — Vec<String> with .to_string() for
//   static tabs and .clone() for dynamic tabs.
// Candidate A: return `Vec<&str>` / mixed-lifetime approach, static tabs as
//   `&'static str` — zero alloc for static portion.
// Candidate B: return an iterator of `Cow<'_, str>` — static tabs borrow,
//   dynamic tabs are `Owned`.
//
// Bench inputs: N ∈ {0, 3, 10} to show per-tab scaling.

fn bench_tab_titles(c: &mut Criterion) {
    let dynamic_counts: &[usize] = &[0, 3, 10];
    let mut group = c.benchmark_group("tab_titles");

    // Baseline: production pattern — Vec<String> + to_string() + clone().
    for &n in dynamic_counts {
        let pages: Vec<String> = (0..n).map(|i| format!("collection {i}")).collect();

        group.bench_with_input(
            BenchmarkId::new("vec_string_clone", n),
            &pages,
            |b, pages| {
                b.iter(|| {
                    let mut titles = Vec::with_capacity(pages.len() + 3);
                    titles.push(black_box("Home").to_string());
                    titles.push(black_box("Updates").to_string());
                    titles.push(black_box("Config").to_string());
                    for page in pages {
                        titles.push(black_box(page).clone());
                    }
                    black_box(titles)
                })
            },
        );
    }

    // Candidate: static tabs as &'static str, dynamic as &str borrows —
    //   zero allocation for the static portion; Vec itself is still allocated.
    for &n in dynamic_counts {
        let pages: Vec<String> = (0..n).map(|i| format!("collection {i}")).collect();

        group.bench_with_input(
            BenchmarkId::new("static_str_borrow", n),
            &pages,
            |b, pages| {
                b.iter(|| {
                    let mut titles: Vec<&str> = Vec::with_capacity(pages.len() + 3);
                    titles.push(black_box("Home"));
                    titles.push(black_box("Updates"));
                    titles.push(black_box("Config"));
                    for page in pages {
                        titles.push(black_box(page.as_str()));
                    }
                    black_box(titles)
                })
            },
        );
    }

    group.finish();
}

// ── emit_status_format ────────────────────────────────────────────────────────
//
// src/download/events.rs:emit_status — called once per BeatmapsetStatus library
// event with a mirror kind.  Current shape allocates a new String via format!()
// for each status variant:
//   format!("checking {}", mirror.label())
//   format!("{} from {}", status::DOWNLOADING, mirror.label())
//   format!("verifying from {}", mirror.label())
// mirror.label() returns &'static str; the prefix constants are also &'static
// str.  With 4 concurrent downloads × ~3 status events each per mirror pass
// (contacting + downloading + verifying) × 3 retries ≈ 36 allocs/sec minimum.
//
// Candidate: pre-concatenate the common per-mirror strings into static storage
// (a LazyLock<HashMap<MirrorKind, String>>) or use Cow<'static, str> for the
// common prefixes.  For the bench we measure the raw format! overhead vs
// a manual write_to approach (single allocation per call, no intermediate
// capture).
//
// Bench inputs: the 3 main status variants × 4 built-in mirror labels.

fn bench_emit_status_format(c: &mut Criterion) {
    // Mirror labels as returned by MirrorKind::label() — &'static str.
    let labels: &[(&str, &str)] = &[
        ("Nerinyan", "nerinyan"),
        ("osu.direct", "osu_direct"),
        ("Sayobot", "sayobot"),
        ("Nekoha", "nekoha"),
    ];

    const DOWNLOADING: &str = "downloading";

    let mut group = c.benchmark_group("emit_status_format");

    for &(label, key) in labels {
        // Baseline: current shape — one format! per variant.
        group.bench_with_input(
            BenchmarkId::new("format_contacting", key),
            label,
            |b, label| {
                b.iter(|| {
                    let s = format!("checking {}", black_box(label));
                    black_box(s)
                })
            },
        );
        group.bench_with_input(
            BenchmarkId::new("format_downloading", key),
            label,
            |b, label| {
                b.iter(|| {
                    let s = format!("{} from {}", DOWNLOADING, black_box(label));
                    black_box(s)
                })
            },
        );
        group.bench_with_input(
            BenchmarkId::new("format_verifying", key),
            label,
            |b, label| {
                b.iter(|| {
                    let s = format!("verifying from {}", black_box(label));
                    black_box(s)
                })
            },
        );

        // Candidate A: String::with_capacity + push_str — same alloc count but
        // avoids format machinery; measures raw String construction cost.
        group.bench_with_input(
            BenchmarkId::new("push_str_contacting", key),
            label,
            |b, label| {
                b.iter(|| {
                    let mut s = String::with_capacity(9 + label.len());
                    s.push_str("checking ");
                    s.push_str(black_box(label));
                    black_box(s)
                })
            },
        );
        group.bench_with_input(
            BenchmarkId::new("push_str_downloading", key),
            label,
            |b, label| {
                b.iter(|| {
                    const FROM: &str = " from ";
                    let mut s = String::with_capacity(DOWNLOADING.len() + FROM.len() + label.len());
                    s.push_str(DOWNLOADING);
                    s.push_str(FROM);
                    s.push_str(black_box(label));
                    black_box(s)
                })
            },
        );
        group.bench_with_input(
            BenchmarkId::new("push_str_verifying", key),
            label,
            |b, label| {
                b.iter(|| {
                    const PREFIX: &str = "verifying from ";
                    let mut s = String::with_capacity(PREFIX.len() + label.len());
                    s.push_str(PREFIX);
                    s.push_str(black_box(label));
                    black_box(s)
                })
            },
        );
    }

    group.finish();
}

// ── render_constraints_vec ────────────────────────────────────────────────────
//
// src/tui/download.rs:render — called on every TUI render frame per open
// download tab.  Current shape:
//   let mut constraints = Vec::with_capacity(4);
//   if show_disk_warning { constraints.push(Constraint::Length(1)); }
//   constraints.push(Constraint::Length(INFO_HEIGHT));
//   constraints.push(Constraint::Length(GAUGE_HEIGHT));
//   constraints.push(Constraint::Min(0));
//   Layout::vertical(constraints).split(area)
// Vec<Constraint> is heap-allocated every frame; layout is structurally fixed
// (3 or 4 elements).  At 30 fps with 3 open tabs: 90 allocs/sec.
//
// Candidate: use a fixed-size stack array and pass a slice, avoiding the heap
// allocation entirely.  Layout::vertical() accepts &[Constraint] via Into<Layout>.
//
// Bench inputs: both branches (show_disk_warning = true / false) to capture the
// common case (false) and the warnings case (true).

fn bench_render_constraints_vec(c: &mut Criterion) {
    use ratatui::layout::Constraint;

    const INFO_HEIGHT: u16 = 8;
    const GAUGE_HEIGHT: u16 = 3;

    let mut group = c.benchmark_group("render_constraints_vec");

    // Baseline: Vec::with_capacity + conditional push — production shape.
    for show_disk_warning in [false, true] {
        let label = if show_disk_warning {
            "with_warning"
        } else {
            "no_warning"
        };
        group.bench_with_input(
            BenchmarkId::new("vec_with_capacity", label),
            &show_disk_warning,
            |b, &show_disk_warning| {
                b.iter(|| {
                    let mut constraints = Vec::with_capacity(4);
                    if black_box(show_disk_warning) {
                        constraints.push(Constraint::Length(1));
                    }
                    constraints.push(Constraint::Length(INFO_HEIGHT));
                    constraints.push(Constraint::Length(GAUGE_HEIGHT));
                    constraints.push(Constraint::Min(0));
                    black_box(constraints)
                })
            },
        );

        // Candidate: two static arrays; branch selects the right slice — zero
        // heap alloc.  Arrays are the bench input so they live long enough.
        let with_warn: [Constraint; 4] = [
            Constraint::Length(1),
            Constraint::Length(INFO_HEIGHT),
            Constraint::Length(GAUGE_HEIGHT),
            Constraint::Min(0),
        ];
        let without_warn: [Constraint; 3] = [
            Constraint::Length(INFO_HEIGHT),
            Constraint::Length(GAUGE_HEIGHT),
            Constraint::Min(0),
        ];
        let arrays = (with_warn, without_warn, show_disk_warning);
        group.bench_with_input(
            BenchmarkId::new("fixed_array_slice", label),
            &arrays,
            |b, (with_warn, without_warn, show_disk_warning)| {
                b.iter(|| {
                    let slice: &[Constraint] = if black_box(*show_disk_warning) {
                        &with_warn[..]
                    } else {
                        &without_warn[..]
                    };
                    black_box(slice)
                })
            },
        );
    }

    group.finish();
}

// ── emit_status_retrying ──────────────────────────────────────────────────────
//
// src/download/events.rs:emit_status — the RetryingTransient and RateLimited
// arms were not covered by iter 21 (which fixed Contacting/Downloading/
// Verifying/BeatmapsetCompleted).  Both arms still use format!:
//
//   RetryingTransient:
//     format!("retrying {} after {reason} (attempt {attempt}/{max_attempts})",
//             mirror.label())
//   RateLimited:
//     format!("{} on all mirrors, waiting {}s", status::RATE_LIMITED, cooldown_secs)
//
// RetryingTransient fires on every transient HTTP error per mirror attempt;
// with 4 concurrent downloads × 3 retry attempts each, this is ~12 format!
// calls per beatmapset in the worst-case network path.
//
// Candidate: String::with_capacity + push_str for both arms — same semantics,
// avoids format! machinery and the implicit capacity estimate it uses.
//
// Bench inputs: mirror label sizes representative of the 4 built-in mirrors;
// reason strings of short (~10 chars) and long (~35 chars).

fn bench_emit_status_retrying(c: &mut Criterion) {
    let labels: &[(&str, &str)] = &[
        ("Nerinyan", "nerinyan"),
        ("osu.direct", "osu_direct"),
        ("Sayobot", "sayobot"),
        ("Nekoha", "nekoha"),
    ];
    let reasons: &[(&str, &str)] = &[
        ("connection reset", "short"),
        ("connection reset by peer (os error 104)", "long"),
    ];
    let attempt: u32 = 2;
    let max_attempts: u32 = 3;
    let cooldown_secs: u64 = 60;
    const RATE_LIMITED: &str = "rate limited";

    let mut group = c.benchmark_group("emit_status_retrying");

    // ── RetryingTransient ────────────────────────────────────────────────────
    for &(label, label_key) in labels {
        for &(reason, reason_key) in reasons {
            let bench_key = format!("{label_key}/{reason_key}");

            // Baseline: production format! call.
            group.bench_with_input(
                BenchmarkId::new("format_retrying", &bench_key),
                &(label, reason),
                |b, &(label, reason)| {
                    b.iter(|| {
                        let s = format!(
                            "retrying {} after {reason} (attempt {attempt}/{max_attempts})",
                            black_box(label)
                        );
                        black_box(s)
                    })
                },
            );

            // Candidate: push_str with exact pre-computed capacity.
            group.bench_with_input(
                BenchmarkId::new("push_str_retrying", &bench_key),
                &(label, reason),
                |b, &(label, reason)| {
                    b.iter(|| {
                        let attempt_s = attempt.to_string();
                        let max_s = max_attempts.to_string();
                        let mut s = String::with_capacity(
                            "retrying ".len()
                                + label.len()
                                + " after ".len()
                                + reason.len()
                                + " (attempt ".len()
                                + attempt_s.len()
                                + "/".len()
                                + max_s.len()
                                + ")".len(),
                        );
                        s.push_str("retrying ");
                        s.push_str(black_box(label));
                        s.push_str(" after ");
                        s.push_str(black_box(reason));
                        s.push_str(" (attempt ");
                        s.push_str(&attempt_s);
                        s.push('/');
                        s.push_str(&max_s);
                        s.push(')');
                        black_box(s)
                    })
                },
            );
        }
    }

    // ── RateLimited ──────────────────────────────────────────────────────────
    // Baseline: production format! call.
    group.bench_function("format_rate_limited", |b| {
        b.iter(|| {
            let s = format!(
                "{} on all mirrors, waiting {}s",
                black_box(RATE_LIMITED),
                black_box(cooldown_secs)
            );
            black_box(s)
        })
    });

    // Candidate: push_str.
    group.bench_function("push_str_rate_limited", |b| {
        b.iter(|| {
            let secs_s = black_box(cooldown_secs).to_string();
            let mut s = String::with_capacity(
                RATE_LIMITED.len() + " on all mirrors, waiting ".len() + secs_s.len() + "s".len(),
            );
            s.push_str(black_box(RATE_LIMITED));
            s.push_str(" on all mirrors, waiting ");
            s.push_str(&secs_s);
            s.push('s');
            black_box(s)
        })
    });

    group.finish();
}

// ── current_snapshots beatmapset vec clone ────────────────────────────────────
//
// src/app/state.rs:build_selective_download_request (and scan.rs re_scan_task)
// both materialise the full `HashMap<u32, LocalBeatmapset>` into a `Vec` before
// passing it to `snapshots::current_snapshots`:
//
//   let beatmapsets: Vec<_> = self.updates.scan.local_beatmapsets
//       .values().cloned().collect();            // ← O(N) clone
//   current_snapshots(client, collections, &beatmapsets, …)
//
// `current_snapshots` only needs to iterate by reference to build the
// `checksum → beatmapset_id` index.  Changing the signature to accept
// `impl IntoIterator<Item = &LocalBeatmapset>` lets callers pass
// `map.values()` directly — zero clone, zero Vec alloc.
//
// `LocalBeatmapset` = { id: u32, beatmaps: Vec<LocalBeatmap> }
// `LocalBeatmap`    = { checksum: String }
// Each clone allocates an inner Vec + one String per beatmap.
//
// Bench inputs: (beatmapsets, beatmaps_per_set) = (500, 5), (5000, 5),
//   (10000, 5) — representative of small, medium, and large installs.
// Both callers run this once per "start download" trigger, not per-tick.

fn make_local_beatmapsets(
    n: usize,
    per_set: usize,
) -> std::collections::HashMap<u32, osu_shelf::osu_db::LocalBeatmapset> {
    use osu_shelf::osu_db::{LocalBeatmap, LocalBeatmapset, Md5};
    (0..n as u32)
        .map(|id| {
            let beatmapset = LocalBeatmapset {
                id,
                beatmaps: (0..per_set)
                    .map(|b| {
                        let v = (id as usize * per_set + b) as u64;
                        let mut md5: Md5 = [0u8; 16];
                        md5[..8].copy_from_slice(&v.to_le_bytes());
                        LocalBeatmap { checksum: md5 }
                    })
                    .collect(),
            };
            (id, beatmapset)
        })
        .collect()
}

fn bench_current_snapshots_beatmapset_clone(c: &mut Criterion) {
    use osu_shelf::osu_db::LocalBeatmapset;

    let configs: &[(&str, usize, usize)] = &[
        ("500x5", 500, 5),
        ("5000x5", 5000, 5),
        ("10000x5", 10000, 5),
    ];

    let mut group = c.benchmark_group("current_snapshots_beatmapset_clone");

    for &(label, n, per_set) in configs {
        let map = make_local_beatmapsets(n, per_set);

        // Baseline: current callers — values().cloned().collect() into a Vec.
        group.bench_with_input(
            BenchmarkId::new("values_cloned_collect", label),
            &map,
            |b, map| {
                b.iter(|| {
                    let v: Vec<LocalBeatmapset> = black_box(map).values().cloned().collect();
                    black_box(v)
                })
            },
        );

        // Candidate: pass values() iterator directly — no Vec, no clone.
        // Simulates checksum_beatmapset_index(map.values()) after the
        // signature change.  We build the same HashMap<Md5, u32> index
        // as the real function to ensure the work is equivalent.
        group.bench_with_input(
            BenchmarkId::new("values_iter_direct", label),
            &map,
            |b, map| {
                b.iter(|| {
                    use osu_shelf::osu_db::{Md5, checksum};
                    let mut index: std::collections::HashMap<Md5, u32> =
                        std::collections::HashMap::new();
                    for beatmapset in black_box(map).values() {
                        for beatmap in &beatmapset.beatmaps {
                            if !checksum::is_empty(&beatmap.checksum) {
                                index.insert(beatmap.checksum, beatmapset.id);
                            }
                        }
                    }
                    black_box(index)
                })
            },
        );
    }

    group.finish();
}

// ── summary_spans format! per u32 stat ───────────────────────────────────────
//
// src/tui/download.rs:summary_spans — called once per render_info per open
// download tab per frame (~30 fps).  Four format! calls convert u32 counters
// to owned Strings on every call:
//   format!("{label}: ")
//   format!("{downloaded} downloaded")
//   format!("{displayed_skipped} skipped")
//   format!("{failed} failed")
// plus optionally format!("{unverified} unverified") when unverified > 0.
//
// That is 4–5 heap allocations per frame per tab.  With 3 concurrent downloads
// open: ≥120 allocs/sec from this function alone.
//
// Candidate: pre-allocate the full String with String::with_capacity and use
// push_str + itoa-style integer formatting, or write! into a single buffer.
// A simpler fix: replace each format!("{n} word") with a push_str pair —
// one to_string() call (cheaper than format!) + push_str of the suffix.
//
// Bench inputs: typical counter values (small, mid-range, large) plus the
// case with unverified > 0 to capture the conditional fifth allocation.

fn bench_summary_spans_format(c: &mut Criterion) {
    // Representative counter values: downloaded, skipped, failed, unverified
    let cases: &[(&str, u32, u32, u32, u32)] = &[
        ("small", 12, 3, 1, 0),
        ("mid", 3456, 78, 9, 0),
        ("large", 98765, 4321, 100, 0),
        ("with_unverified", 500, 50, 5, 3),
    ];

    let mut group = c.benchmark_group("summary_spans_format");

    for &(label, downloaded, skipped, failed, unverified) in cases {
        let displayed_skipped = skipped.saturating_add(unverified);

        // Baseline: current production shape — 4–5 independent format! calls.
        group.bench_with_input(
            BenchmarkId::new("format_calls", label),
            &(downloaded, displayed_skipped, failed, unverified),
            |b, &(downloaded, displayed_skipped, failed, unverified)| {
                b.iter(|| {
                    let s0 = format!("{}: ", black_box("progress"));
                    let s1 = format!("{} downloaded", black_box(downloaded));
                    let s2 = format!("{} skipped", black_box(displayed_skipped));
                    let s3 = format!("{} failed", black_box(failed));
                    let mut v = vec![s0, s1, s2, s3];
                    if unverified > 0 {
                        v.push(format!("{} unverified", black_box(unverified)));
                    }
                    black_box(v)
                })
            },
        );

        // Candidate A: to_string() + push_str pairs — one alloc per number,
        // avoids the format! machinery overhead.
        group.bench_with_input(
            BenchmarkId::new("to_string_push_str", label),
            &(downloaded, displayed_skipped, failed, unverified),
            |b, &(downloaded, displayed_skipped, failed, unverified)| {
                b.iter(|| {
                    let mut s1 = black_box(downloaded).to_string();
                    s1.push_str(" downloaded");
                    let mut s2 = black_box(displayed_skipped).to_string();
                    s2.push_str(" skipped");
                    let mut s3 = black_box(failed).to_string();
                    s3.push_str(" failed");
                    let mut v = vec!["progress: ".to_string(), s1, s2, s3];
                    if unverified > 0 {
                        let mut s4 = black_box(unverified).to_string();
                        s4.push_str(" unverified");
                        v.push(s4);
                    }
                    black_box(v)
                })
            },
        );

        // Candidate B: write! into a single with_capacity String — single
        // allocation per stat string, no format! overhead.
        group.bench_with_input(
            BenchmarkId::new("write_with_capacity", label),
            &(downloaded, displayed_skipped, failed, unverified),
            |b, &(downloaded, displayed_skipped, failed, unverified)| {
                use std::fmt::Write as _;
                b.iter(|| {
                    // 12 digits max for u32, plus suffix
                    let mut s1 = String::with_capacity(12 + " downloaded".len());
                    write!(s1, "{} downloaded", black_box(downloaded)).unwrap();
                    let mut s2 = String::with_capacity(12 + " skipped".len());
                    write!(s2, "{} skipped", black_box(displayed_skipped)).unwrap();
                    let mut s3 = String::with_capacity(12 + " failed".len());
                    write!(s3, "{} failed", black_box(failed)).unwrap();
                    let mut v = vec!["progress: ".to_string(), s1, s2, s3];
                    if unverified > 0 {
                        let mut s4 = String::with_capacity(12 + " unverified".len());
                        write!(s4, "{} unverified", black_box(unverified)).unwrap();
                        v.push(s4);
                    }
                    black_box(v)
                })
            },
        );
    }

    group.finish();
}

// ── active_download_item prefix format! ──────────────────────────────────────
//
// src/tui/widgets.rs:active_download_item (line 323) — called once per
// active download slot per render frame:
//   let prefix = format!("  #{:<7} ", line.beatmapset_id);
//
// With concurrent = 4 and ~30 fps: 120 format! calls/sec allocating a ~12-byte
// String from a u32.  The padding is always 7 chars wide, so beatmapset_ids
// that fit in ≤7 digits (≤9_999_999) produce a fixed 11-char string.
//
// Candidate: replace with to_string() + manual padding or a with_capacity
// String + write! using the same :<7 format spec.  Measuring whether the
// format! dispatch overhead is measurable vs manual with_capacity + write!.

fn bench_active_download_prefix(c: &mut Criterion) {
    // Representative beatmapset IDs: small (5 digits), typical (7 digits),
    // large (9 digits — hypothetical future IDs beyond current osu! range).
    let ids: &[(&str, u32)] = &[
        ("5digit", 12345),
        ("7digit", 9876543),
        ("9digit", 987654321),
    ];

    let mut group = c.benchmark_group("active_download_prefix");

    for &(label, id) in ids {
        // Baseline: current production shape — format! with left-pad specifier.
        group.bench_with_input(BenchmarkId::new("format_pad", label), &id, |b, &id| {
            b.iter(|| {
                let prefix = format!("  #{:<7} ", black_box(id));
                black_box(prefix)
            })
        });

        // Candidate: String::with_capacity + write! — same output, avoids
        // format! machinery (no format_args! allocation for the spec string).
        group.bench_with_input(
            BenchmarkId::new("write_with_capacity", label),
            &id,
            |b, &id| {
                use std::fmt::Write as _;
                b.iter(|| {
                    let mut s = String::with_capacity(12);
                    write!(s, "  #{:<7} ", black_box(id)).unwrap();
                    black_box(s)
                })
            },
        );

        // Candidate B: to_string + manual space-pad to 7 chars.
        group.bench_with_input(BenchmarkId::new("to_string_pad", label), &id, |b, &id| {
            b.iter(|| {
                let id_s = black_box(id).to_string();
                let pad = 7usize.saturating_sub(id_s.len());
                let mut s = String::with_capacity(3 + id_s.len() + pad + 1);
                s.push_str("  #");
                s.push_str(&id_s);
                for _ in 0..pad {
                    s.push(' ');
                }
                s.push(' ');
                black_box(s)
            })
        });
    }

    group.finish();
}

// ── render_gauge title strings format! ───────────────────────────────────────
//
// src/tui/download.rs:render_gauge (lines 327–339) — called once per render
// frame per open download tab (~30 fps).  Two format! calls build the gauge
// block title strings with multiple usize/u32 numbers each:
//
//   top_title:
//     format!(" {downloaded} downloaded  {queue_remaining} queued ")
//   verified_title (no avg):
//     format!(" {verified_display}/{total_collection} verified ")
//   verified_title (with avg):
//     format!(" {verified_display}/{total_collection} verified ({avg} avg) ")
//
// With 3 concurrent download tabs open: 90–180 format! allocs/sec here.
//
// Candidate: String::with_capacity + write! — same strings, single allocation
// with exact capacity, no implicit format_args! overhead.

fn bench_render_gauge_titles(c: &mut Criterion) {
    // Representative values: small numbers vs large numbers (more digits).
    let cases: &[(&str, usize, usize, usize, usize)] = &[
        // (label, downloaded, queue_remaining, verified_display, total_collection)
        ("small", 12, 88, 12, 100),
        ("mid", 3456, 544, 3456, 4000),
        ("large", 98765, 1235, 98765, 100000),
    ];

    let mut group = c.benchmark_group("render_gauge_titles");

    for &(label, downloaded, queue_remaining, verified_display, total_collection) in cases {
        // Baseline: current production shape — two separate format! calls.
        group.bench_with_input(
            BenchmarkId::new("format_calls", label),
            &(
                downloaded,
                queue_remaining,
                verified_display,
                total_collection,
            ),
            |b, &(downloaded, queue_remaining, verified_display, total_collection)| {
                b.iter(|| {
                    let top = format!(
                        " {} downloaded  {} queued ",
                        black_box(downloaded),
                        black_box(queue_remaining)
                    );
                    let verified = format!(
                        " {}/{} verified ",
                        black_box(verified_display),
                        black_box(total_collection)
                    );
                    black_box((top, verified))
                })
            },
        );

        // Candidate: String::with_capacity + write! — avoids format! machinery
        // by pre-sizing to exact byte count before writing.
        group.bench_with_input(
            BenchmarkId::new("write_with_capacity", label),
            &(
                downloaded,
                queue_remaining,
                verified_display,
                total_collection,
            ),
            |b, &(downloaded, queue_remaining, verified_display, total_collection)| {
                use std::fmt::Write as _;
                b.iter(|| {
                    // 20 digits max for two usize values
                    let mut top = String::with_capacity(20 + " downloaded   queued ".len() + 2);
                    write!(
                        top,
                        " {} downloaded  {} queued ",
                        black_box(downloaded),
                        black_box(queue_remaining)
                    )
                    .unwrap();
                    let mut verified = String::with_capacity(20 + " /  verified ".len() + 2);
                    write!(
                        verified,
                        " {}/{} verified ",
                        black_box(verified_display),
                        black_box(total_collection)
                    )
                    .unwrap();
                    black_box((top, verified))
                })
            },
        );
    }

    group.finish();
}

// ── active_download_item message clone ───────────────────────────────────────
//
// src/tui/widgets.rs:active_download_item (line 344-348) — per active download
// slot per render frame.  `truncate_to_width` returns an owned `String`;
// `message_w` is then computed via `.chars().count()`, and the string is cloned
// into `Span::styled`.  The original `message` binding is never used again after
// the clone — the clone is redundant.
//
// Baseline: compute message_w, then clone message into Span.
// Candidate: compute message_w, then move message into Span (no clone).
//
// NOTE: the bench isolates string ownership cost only; `Span::styled` itself is
// not called since that would require pulling in ratatui.  We measure the clone
// vs move of the underlying String that becomes the Span's content.
//
// Bench inputs: short (fits budget, no truncation), long (truncated with …),
// and empty (zero-length budget) messages.

fn bench_active_download_message_clone(c: &mut Criterion) {
    // Simulate truncate_to_width return values.
    let cases: &[(&str, &str)] = &[
        ("short", "downloading from Beatconnect"),
        (
            "long",
            "downloading very-long-mirror-name-that-got-truncated…",
        ),
        ("empty", ""),
    ];

    let mut group = c.benchmark_group("active_download_message_clone");

    for &(label, msg) in cases {
        // Baseline: compute char count, then clone into span content.
        group.bench_with_input(BenchmarkId::new("clone_into_span", label), msg, |b, msg| {
            b.iter(|| {
                let message = black_box(msg).to_string(); // simulates truncate_to_width output
                let message_w = message.chars().count() as u16;
                // clone needed because message_w is computed first (current production shape)
                let span_content = message.clone();
                black_box((message_w, span_content))
            })
        });

        // Candidate: compute char count first, then move message — no clone.
        group.bench_with_input(BenchmarkId::new("move_into_span", label), msg, |b, msg| {
            b.iter(|| {
                let message = black_box(msg).to_string(); // simulates truncate_to_width output
                // Reorder: chars().count() can be computed before ownership moves
                let message_w = message.chars().count() as u16;
                // move message directly — message_w already captured
                let span_content = message; // no clone
                black_box((message_w, span_content))
            })
        });
    }

    group.finish();
}

// ── active_download_item progress percent format! ────────────────────────────
//
// src/tui/widgets.rs:active_download_item (line 370) — per active download slot
// per render frame when `progress_ratio` is Some:
//   format!(" {:>3}%", (ratio * 100.0).round() as u16)
//
// Produces a 5-byte string " NNN%" for a u16 in 0..=100.
// With 4 concurrent slots at 30 fps: ~120 format! calls/sec.
//
// Baseline: format!(" {:>3}%", value) — allocates with format machinery.
// Candidate A: String::with_capacity(5) + push + push_str itoa-style.
// Candidate B: String::with_capacity(5) + write! — avoids format! overhead
//   while keeping the right-align padding.
//
// NOTE: this bench measures only string construction, not Span creation.

fn bench_progress_percent_format(c: &mut Criterion) {
    // Representative ratio values covering 1-, 2-, 3-digit percentages.
    let cases: &[(&str, u16)] = &[("1digit", 5), ("2digit", 42), ("3digit", 100)];

    let mut group = c.benchmark_group("progress_percent_format");

    for &(label, pct) in cases {
        // Baseline: current production shape.
        group.bench_with_input(
            BenchmarkId::new("format_right_align", label),
            &pct,
            |b, &pct| {
                b.iter(|| {
                    let s = format!(" {:>3}%", black_box(pct));
                    black_box(s)
                })
            },
        );

        // Candidate A: with_capacity + manual right-pad push.
        // " {:>3}%" always produces exactly 5 chars: space + 3-char right-aligned
        // number + '%'.  For values 0–100 the number fits in 1–3 digits.
        group.bench_with_input(
            BenchmarkId::new("with_capacity_push", label),
            &pct,
            |b, &pct| {
                b.iter(|| {
                    let n = black_box(pct);
                    let mut s = String::with_capacity(5);
                    s.push(' ');
                    // right-align in 3 chars: pad with spaces on the left
                    if n < 10 {
                        s.push_str("  ");
                    } else if n < 100 {
                        s.push(' ');
                    }
                    // itoa-style: avoid format! for the number itself
                    let n_s = n.to_string();
                    s.push_str(&n_s);
                    s.push('%');
                    black_box(s)
                })
            },
        );

        // Candidate B: write! into with_capacity — keeps the {:>3} semantic.
        group.bench_with_input(
            BenchmarkId::new("write_with_capacity", label),
            &pct,
            |b, &pct| {
                use std::fmt::Write as _;
                b.iter(|| {
                    let mut s = String::with_capacity(5);
                    write!(s, " {:>3}%", black_box(pct)).unwrap();
                    black_box(s)
                })
            },
        );

        // Candidate C: static lookup table — zero allocation, pointer + bounds check only.
        // pct is always 0–100 (clamped by .min(BAR_WIDTH) upstream); table has 101 entries.
        group.bench_with_input(
            BenchmarkId::new("static_table_lookup", label),
            &pct,
            |b, &pct| {
                b.iter(|| {
                    let s: &'static str = PCT_TABLE[black_box(pct) as usize];
                    black_box(s)
                })
            },
        );
    }

    group.finish();
}

// 101-entry table of " {:>3}%" strings for pct in 0..=100.
// Used by bench_progress_percent_format/static_table_lookup.
const PCT_TABLE: [&str; 101] = [
    "   0%", "   1%", "   2%", "   3%", "   4%", "   5%", "   6%", "   7%", "   8%", "   9%",
    "  10%", "  11%", "  12%", "  13%", "  14%", "  15%", "  16%", "  17%", "  18%", "  19%",
    "  20%", "  21%", "  22%", "  23%", "  24%", "  25%", "  26%", "  27%", "  28%", "  29%",
    "  30%", "  31%", "  32%", "  33%", "  34%", "  35%", "  36%", "  37%", "  38%", "  39%",
    "  40%", "  41%", "  42%", "  43%", "  44%", "  45%", "  46%", "  47%", "  48%", "  49%",
    "  50%", "  51%", "  52%", "  53%", "  54%", "  55%", "  56%", "  57%", "  58%", "  59%",
    "  60%", "  61%", "  62%", "  63%", "  64%", "  65%", "  66%", "  67%", "  68%", "  69%",
    "  70%", "  71%", "  72%", "  73%", "  74%", "  75%", "  76%", "  77%", "  78%", "  79%",
    "  80%", "  81%", "  82%", "  83%", "  84%", "  85%", "  86%", "  87%", "  88%", "  89%",
    "  90%", "  91%", "  92%", "  93%", "  94%", "  95%", "  96%", "  97%", "  98%", "  99%",
    " 100%",
];

// ── status_pill format! ───────────────────────────────────────────────────────
//
// src/tui/widgets.rs:status_pill (line 297) — called once per render frame
// (render_info).  Current shape:
//   format!(" {} ", label.into())
// where label is a &'static str.  Always pads a static string with one space
// on each side.
//
// Baseline: format!(" {} ", label) — allocates via format machinery.
// Candidate: String::with_capacity(label.len() + 2) + push_str — single alloc,
//   no format machinery.
//
// NOTE: called only once per frame (not concurrent), so savings are small in
// absolute terms; this bench verifies the idiomatic direction is still faster.

fn bench_status_pill_format(c: &mut Criterion) {
    // Representative status labels from DownloadStage display strings.
    let labels: &[&str] = &["PENDING", "RESOLVING", "DOWNLOADING", "COMPLETED", "FAILED"];

    let mut group = c.benchmark_group("status_pill_format");

    for label in labels {
        // Baseline: current production shape.
        group.bench_with_input(BenchmarkId::new("format_pad", label), label, |b, label| {
            b.iter(|| {
                let s = format!(" {} ", black_box(*label));
                black_box(s)
            })
        });

        // Candidate: with_capacity + push_str — avoids format! machinery.
        group.bench_with_input(
            BenchmarkId::new("with_capacity_push_str", label),
            label,
            |b, label| {
                b.iter(|| {
                    let label = black_box(*label);
                    let mut s = String::with_capacity(label.len() + 2);
                    s.push(' ');
                    s.push_str(label);
                    s.push(' ');
                    black_box(s)
                })
            },
        );
    }

    group.finish();
}

// ── render_info page title clone ─────────────────────────────────────────────
//
// src/tui/download.rs:render_info (line 152) — called once per render frame per
// open download tab (~30 fps).  Current shape:
//   Span::styled(page.title.clone(), ...)
// `page` is `&CollectionPage`, so `page.title` is `&String`.  `Span::styled`
// takes `T: Into<Cow<'a, str>>`, which both `String` (→ Cow::Owned) and `&str`
// (→ Cow::Borrowed) satisfy.  Cloning allocates a new String every frame.
//
// Same pattern appears for `page.uploader` (as_deref().unwrap_or(...).to_owned())
// and `page.output_dir` (as_deref().unwrap_or(...).to_owned()) on lines 157, 165.
// All three borrow a &str that lives for the duration of the render call and
// can be passed directly to Span::styled as Cow::Borrowed — zero allocation.
//
// Candidate: pass `page.title.as_str()` / the already-borrowed `&str` directly.
//
// NOTE: bench measures the raw Cow construction cost without Span, since pulling
// in ratatui would add unrelated overhead.  The allocation savings are real —
// Cow::Borrowed is a pointer + length, Cow::Owned is a heap alloc.
//
// Frequency: 3 allocations per frame (title + uploader + output_dir) × 30 fps
// = ~90 avoidable heap allocs/sec per open download tab.

fn bench_render_info_title_clone(c: &mut Criterion) {
    use std::borrow::Cow;

    // Representative title strings (collection names from osu!collector).
    let titles: &[(&str, &str)] = &[
        ("short", "Easy Maps"),
        ("typical", "Loved Maps 2023 — Various Artists"),
        (
            "long",
            "ABSOLUTE MEGA COLLECTION of every ranked map ever uploaded by top players 2019-2024",
        ),
    ];

    let mut group = c.benchmark_group("render_info_title_clone");

    for &(label, title) in titles {
        let owned = title.to_string();

        // Baseline: current production shape — clone owned String into Cow::Owned.
        group.bench_with_input(
            BenchmarkId::new("clone_cow_owned", label),
            &owned,
            |b, owned| {
                b.iter(|| {
                    // Simulates page.title.clone() → Into<Cow<'_, str>>
                    let cow: Cow<'_, str> = Cow::Owned(black_box(owned).clone());
                    black_box(cow)
                })
            },
        );

        // Candidate: borrow as &str → Cow::Borrowed — zero allocation.
        group.bench_with_input(
            BenchmarkId::new("borrow_cow_borrowed", label),
            &owned,
            |b, owned| {
                b.iter(|| {
                    // Simulates page.title.as_str() → Into<Cow<'_, str>>
                    let cow: Cow<'_, str> = Cow::Borrowed(black_box(owned.as_str()));
                    black_box(cow)
                })
            },
        );
    }

    group.finish();
}

// ── render_indeterminate_block title to_string ────────────────────────────────
//
// src/tui/download.rs:render_indeterminate_block (line 481) — called once per
// render frame during Pending and Resolving stages (~30 fps).  Current shape:
//   Block::default().title(Line::from(Span::styled(title.to_string(), style)))
// `title` is `&str` (from a format! result at the call site).  `Span::styled`
// takes `T: Into<Cow<'a, str>>`, so passing `title` directly yields a borrowed
// Cow — no allocation.  The `.to_string()` allocates a redundant owned String.
//
// Candidate: pass `title` (a `&str`) directly to `Span::styled` — Cow::Borrowed.
//
// NOTE: bench isolates Cow construction cost; Block/Span not imported.
// The frame lifetime is short-lived (render call), so borrowing is safe.
//
// Frequency: 1 allocation per frame during Pending/Resolving (~30 fps).

fn bench_indeterminate_title_to_string(c: &mut Criterion) {
    use std::borrow::Cow;

    // Representative spinner+label title strings (as produced by format! at call site).
    let titles: &[(&str, &str)] = &[
        ("short", "⠋ preparing "),
        ("typical", "⠹ resolving collection "),
        ("resolve_progress", "⠸ resolving 12/45 collections "),
    ];

    let mut group = c.benchmark_group("indeterminate_title_to_string");

    for &(label, title) in titles {
        // Baseline: current production shape — &str → to_string() → Cow::Owned.
        group.bench_with_input(
            BenchmarkId::new("to_string_cow_owned", label),
            title,
            |b, title| {
                b.iter(|| {
                    let cow: Cow<'_, str> = Cow::Owned(black_box(title).to_string());
                    black_box(cow)
                })
            },
        );

        // Candidate: &str directly → Cow::Borrowed — zero allocation.
        group.bench_with_input(
            BenchmarkId::new("borrow_cow_borrowed", label),
            title,
            |b, title| {
                b.iter(|| {
                    let cow: Cow<'_, str> = Cow::Borrowed(black_box(title));
                    black_box(cow)
                })
            },
        );
    }

    group.finish();
}

// ── tui/mod.rs Layout::split to_vec ──────────────────────────────────────────
//
// src/tui/mod.rs:draw (lines 121-138) — called on every render frame (~30 fps).
// Current shape:
//   let chunks: Vec<_> = Layout::vertical([...]).split(area).to_vec();
// `Layout::split` returns `Rc<[Rect]>`.  `.to_vec()` heap-allocates a new
// Vec<Rect> (3 or 5 Copy elements) so that chunks can be indexed.  Since
// `Rc<[Rect]>` already implements `Deref<Target = [Rect]>`, the Rects can be
// indexed directly without materialising a Vec — saving one heap alloc per frame.
//
// Candidate: index the Rc<[Rect]> directly without .to_vec().
//
// NOTE: bench approximates the Vec<T> vs slice-index cost for a small number of
// Copy values (Rect is 4×u16 = 8 bytes).  We use u64 as a Rect stand-in (same
// Copy + 8 bytes) to avoid a ratatui dependency in the bench crate.
//
// Frequency: 1 allocation per frame (compact=3 rects, non-compact=5 rects) ×
// ~30 fps = ~30 avoidable Vec allocs/sec.

fn bench_layout_split_to_vec(c: &mut Criterion) {
    // Simulate the two layout arm sizes: compact (3 rects) and non-compact (5).
    let cases: &[(&str, &[u64])] = &[
        ("compact_3", &[10, 480, 10]),
        ("non_compact_5", &[10, 1, 480, 1, 10]),
    ];

    let mut group = c.benchmark_group("layout_split_to_vec");

    for &(label, rects) in cases {
        // Baseline: current production shape — clone slice into a Vec, then index.
        group.bench_with_input(
            BenchmarkId::new("to_vec_then_index", label),
            rects,
            |b, rects| {
                b.iter(|| {
                    // Simulates Rc<[Rect]>.to_vec() — allocates a Vec copy of rects.
                    let v: Vec<u64> = black_box(rects).to_vec();
                    // Access the same indices that draw() uses (first, mid, last).
                    let first = v[0];
                    let mid = v[v.len() / 2];
                    let last = v[v.len() - 1];
                    black_box((first, mid, last))
                })
            },
        );

        // Candidate: index the slice directly — no Vec allocation.
        group.bench_with_input(
            BenchmarkId::new("slice_index_direct", label),
            rects,
            |b, rects| {
                b.iter(|| {
                    // Simulates indexing Rc<[Rect]> directly: rects[0], rects[n].
                    let s: &[u64] = black_box(rects);
                    let first = s[0];
                    let mid = s[s.len() / 2];
                    let last = s[s.len() - 1];
                    black_box((first, mid, last))
                })
            },
        );
    }

    group.finish();
}

// ── active_download_item prefix_w chars().count() vs .len() ──────────────────
//
// src/tui/widgets.rs:active_download_item (line 337) — called per active
// download slot per render frame (~30 fps, up to 8 concurrent slots).
// Current shape:
//   let prefix_w = prefix.chars().count() as u16;
// `prefix` is built as "  #" (3 bytes) + id_s (1–7 ASCII digits) + padding
// spaces + " " — guaranteed all-ASCII, always exactly 11 chars.  For an
// all-ASCII string `chars().count()` iterates every byte to decode UTF-8;
// `.len()` returns the stored byte-count field in O(1).
//
// Candidate: `prefix.len() as u16` — O(1) field read instead of O(n) traversal.
//
// NOTE: bench uses a fixed 11-byte all-ASCII string matching the real prefix
// shape.  `chars().count()` iterates all 11 bytes; `.len()` is a single load.
//
// Frequency: concurrent_slots × ~30 fps = up to ~240 calls/sec.

fn bench_prefix_len_vs_chars_count(c: &mut Criterion) {
    // Representative prefix strings produced by active_download_item.
    // All are exactly 11 ASCII chars: "  #" + id (1-7 digits) + padding + " ".
    let cases: &[(&str, &str)] = &[
        ("id_1digit", "  #1       "),
        ("id_4digit", "  #1234    "),
        ("id_7digit", "  #1234567 "),
    ];

    let mut group = c.benchmark_group("prefix_len_vs_chars_count");

    for &(label, prefix) in cases {
        // Baseline: current production shape — chars().count() traverses every byte.
        group.bench_with_input(
            BenchmarkId::new("chars_count", label),
            prefix,
            |b, prefix| {
                b.iter(|| {
                    let w = black_box(prefix).chars().count() as u16;
                    black_box(w)
                })
            },
        );

        // Candidate: .len() — O(1) field read; valid because prefix is all-ASCII.
        group.bench_with_input(BenchmarkId::new("len_field", label), prefix, |b, prefix| {
            b.iter(|| {
                let w = black_box(prefix).len() as u16;
                black_box(w)
            })
        });
    }

    group.finish();
}

// ── truncate_to_width double char-count traversal ─────────────────────────────
//
// src/tui/widgets.rs:active_download_item (lines 345-346) — called per active
// download slot per render frame.  Current shape:
//   let message = truncate_to_width(&line.displayed_message(), budget);
//   let message_w = message.chars().count() as u16;
// `truncate_to_width` already calls `message.chars().count()` internally to
// decide whether to truncate.  On the fast path (no truncation, typical case)
// the string is traversed twice — once inside the function, once at the call
// site — to obtain the same char count.
//
// Candidate: return `(String, u16)` from `truncate_to_width`.
// - Fast path (fits in budget): char_width = count already computed inside fn.
// - Slow path (truncated): char_width = budget (exactly budget chars emitted).
// Call site uses the returned u16 directly, eliminating the second O(n) pass.
//
// NOTE: bench simulates the combined cost of truncate_to_width + post-call
// chars().count() using inline equivalents.  No ratatui dependency required.
//
// Frequency: concurrent_slots × ~30 fps = up to ~240 combined ops/sec.

fn bench_truncate_char_count_double(c: &mut Criterion) {
    // Representative displayed_message() strings: typical length ≤ budget
    // (fast path, no truncation) and one that exceeds (slow path).
    let budget: usize = 60;
    let cases: &[(&str, &str)] = &[
        ("short_fits", "downloading from Beatconnect"),
        ("typical_fits", "rate limited on all mirrors, waiting 3s"),
        (
            "long_fits",
            "retrying nerinyan after connection timeout (attempt 3/5)",
        ),
        (
            "truncated",
            "retrying a-very-long-mirror-hostname.example.com after connection reset by peer (attempt 2/5)",
        ),
    ];

    let mut group = c.benchmark_group("truncate_char_count_double");

    for &(label, msg) in cases {
        // Baseline: current production shape.
        // truncate_to_width calls chars().count() internally, then caller calls it again.
        group.bench_with_input(
            BenchmarkId::new("truncate_then_recount", label),
            msg,
            |b, msg| {
                b.iter(|| {
                    let msg = black_box(msg);
                    // Inline of current truncate_to_width body.
                    let text: String = if msg.chars().count() <= budget {
                        msg.to_string()
                    } else if budget == 1 {
                        "…".to_string()
                    } else {
                        let mut s: String = msg.chars().take(budget.saturating_sub(1)).collect();
                        s.push('…');
                        s
                    };
                    // Caller re-traverses to get width.
                    let w = text.chars().count() as u16;
                    black_box((text, w))
                })
            },
        );

        // Candidate: return (String, u16) — char_width computed once, returned alongside.
        group.bench_with_input(
            BenchmarkId::new("truncate_return_width", label),
            msg,
            |b, msg| {
                b.iter(|| {
                    let msg = black_box(msg);
                    // Inline of candidate truncate_to_width_with_width body.
                    let char_count = msg.chars().count();
                    let (text, char_width): (String, u16) = if char_count <= budget {
                        (msg.to_string(), char_count as u16)
                    } else if budget == 1 {
                        ("…".to_string(), 1)
                    } else {
                        let mut s: String = msg.chars().take(budget.saturating_sub(1)).collect();
                        s.push('…');
                        (s, budget as u16)
                    };
                    // No second traversal — char_width already known.
                    black_box((text, char_width))
                })
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_panel_block_title_format,
    bench_message_style_classify,
    bench_render_separator,
    bench_indeterminate_bar_spans,
    bench_tab_titles,
    bench_emit_status_format,
    bench_render_constraints_vec,
    bench_emit_status_retrying,
    bench_current_snapshots_beatmapset_clone,
    bench_summary_spans_format,
    bench_active_download_prefix,
    bench_render_gauge_titles,
    bench_active_download_message_clone,
    bench_progress_percent_format,
    bench_status_pill_format,
    bench_render_info_title_clone,
    bench_indeterminate_title_to_string,
    bench_layout_split_to_vec,
    bench_prefix_len_vs_chars_count,
    bench_truncate_char_count_double,
    bench_header_render_version_alloc,
    bench_header_tab_to_lowercase,
);
criterion_main!(benches);

// ── header::render version string + width allocs ──────────────────────────────
//
// src/tui/header.rs:18-20 — called on every TUI render frame (~30/s).
// Current production shape:
//   let version = format!(" v{} ", env!("CARGO_PKG_VERSION"));  // heap alloc
//   let version_width = version.chars().count() as u16;          // O(n) traversal
//   let brand_width = BRAND.chars().count() as u16;              // O(n) traversal
//
// env!("CARGO_PKG_VERSION") is a &'static str known at compile time.
// The padded version string " v1.2.3 " is also a compile-time constant —
// concat!(" v", env!("CARGO_PKG_VERSION"), " ") produces &'static str with zero
// runtime cost.  Both strings are pure ASCII, so .len() == .chars().count().
//
// Candidate: static VERSION string via concat! + .len() for widths.
//
// Frequency: ~30/s × every open tab = minimum 30 allocs/sec just for version.

fn bench_header_render_version_alloc(c: &mut Criterion) {
    // Simulate a realistic CARGO_PKG_VERSION value.
    let pkg_version = black_box("1.4.2");
    // The static candidate — no runtime work at all.
    const VERSION_STATIC: &str = concat!(" v", env!("CARGO_PKG_VERSION"), " ");
    const BRAND: &str = " osu-shelf ";

    let mut group = c.benchmark_group("header_render_version_alloc");

    // Baseline: current production shape — format! alloc + chars().count() twice.
    group.bench_function("format_and_chars_count", |b| {
        b.iter(|| {
            let version = format!(" v{} ", black_box(pkg_version));
            let version_width = version.chars().count() as u16;
            let brand_width = black_box(BRAND).chars().count() as u16;
            black_box((version, version_width, brand_width))
        })
    });

    // Candidate: const string (no alloc) + .len() for ASCII-only strings.
    group.bench_function("const_string_len", |b| {
        b.iter(|| {
            // VERSION_STATIC is &'static str — zero runtime allocation.
            let version_width = black_box(VERSION_STATIC).len() as u16;
            let brand_width = black_box(BRAND).len() as u16;
            black_box((VERSION_STATIC, version_width, brand_width))
        })
    });

    group.finish();
}

// ── header::render tab title to_lowercase per frame ───────────────────────────
//
// src/tui/header.rs:43 — inside tabs.iter().enumerate() loop, called every frame.
// Current production shape:
//   let title = title.to_lowercase();  // String heap alloc per tab
//
// Static tab titles ("Home", "Updates", "Config") are compile-time constants;
// their lowercased forms are also constants.  Only dynamic collection tabs
// (page.title.as_str()) actually vary at runtime — and collection names are
// typically already lowercase from the API.
//
// Candidate: pass pre-lowercased &'static str for static tabs; caller normalises
// once at construction (or passes already-lowercase strings from the API).
// For the bench we measure the per-frame cost: to_lowercase() vs. a direct &str.
//
// Frequency: tabs.len() × ~30/s = ≥90 allocs/sec for a typical 3-tab session.

fn bench_header_tab_to_lowercase(c: &mut Criterion) {
    // Tabs representative of a typical session: 3 static + 1-2 collection tabs.
    let static_tabs: &[&str] = &["Home", "Updates", "Config"];
    let dynamic_tabs: &[&str] = &[
        "Home",
        "Updates",
        "Config",
        "Summer 2023 Maps",
        "Loved Picks",
    ];
    // Pre-lowercased equivalents — what the candidate passes instead.
    let static_lower: &[&str] = &["home", "updates", "config"];
    let dynamic_lower: &[&str] = &[
        "home",
        "updates",
        "config",
        "summer 2023 maps",
        "loved picks",
    ];

    let mut group = c.benchmark_group("header_tab_to_lowercase");

    // Baseline: current production shape — to_lowercase() per tab per frame.
    group.bench_with_input(
        BenchmarkId::new("to_lowercase", "3_static"),
        static_tabs,
        |b, tabs| {
            b.iter(|| {
                let result: Vec<String> =
                    black_box(tabs).iter().map(|t| t.to_lowercase()).collect();
                black_box(result)
            })
        },
    );
    group.bench_with_input(
        BenchmarkId::new("to_lowercase", "5_with_dynamic"),
        dynamic_tabs,
        |b, tabs| {
            b.iter(|| {
                let result: Vec<String> =
                    black_box(tabs).iter().map(|t| t.to_lowercase()).collect();
                black_box(result)
            })
        },
    );

    // Candidate: pass pre-lowercased &str — zero per-frame allocation.
    group.bench_with_input(
        BenchmarkId::new("pre_lowercased", "3_static"),
        static_lower,
        |b, tabs| {
            b.iter(|| {
                // Simulates header receiving already-lowercase strings: zero work.
                let result = black_box(tabs).to_vec();
                black_box(result)
            })
        },
    );
    group.bench_with_input(
        BenchmarkId::new("pre_lowercased", "5_with_dynamic"),
        dynamic_lower,
        |b, tabs| {
            b.iter(|| {
                let result = black_box(tabs).to_vec();
                black_box(result)
            })
        },
    );

    group.finish();
}
