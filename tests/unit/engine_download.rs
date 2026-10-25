use super::{ABORTED, DownloadStatus, Downloads, Phase, mirrors_from_config};
use crate::app::{HomeTab, collection::FailureReason};
use crate::config::Config;
use crate::download::{BeatmapStage, DownloadEvent, DownloadStage, DownloadSummary, FailedMap};
use std::sync::Arc;

fn mirror_names(mirrors: &[crate::mirrors::Mirror]) -> Vec<String> {
    mirrors.iter().map(|m| format!("{m:?}")).collect()
}

#[test]
fn mirrors_match_what_the_tui_builds() {
    for video in [false, true] {
        let mut config = Config::default();
        config.download.video = video;
        config.mirror.hinamizawa = true;
        config.mirror.nekoha = false;

        let tui = HomeTab::new(&config).build_mirror_list(false);

        assert_eq!(
            mirror_names(&mirrors_from_config(&config)),
            mirror_names(&tui)
        );
    }
}

#[test]
fn osu_official_is_never_used_without_the_tui_login() {
    let mut config = Config::default();
    config.mirror.osu_official = true;

    let names = mirror_names(&mirrors_from_config(&config)).join(" ");

    assert!(!names.contains("OsuApi"), "{names}");
}

fn status() -> DownloadStatus {
    DownloadStatus::queued(1, 18951, "https://osucollector.com/collections/18951")
}

fn beatmap(stage: BeatmapStage, set: u32) -> DownloadEvent {
    DownloadEvent::BeatmapStatus {
        id: 1,
        beatmapset_id: set,
        stage,
        message: String::new(),
        rate_limited: false,
        cooldown_until: None,
    }
}

#[test]
fn a_run_reports_name_progress_and_bytes() {
    let mut s = status();

    assert!(s.apply(&DownloadEvent::CollectionReady {
        id: 1,
        collection_name: "Tiebreakers".into(),
        uploader: "Sour Dough".into(),
        total_maps: 947,
        output_dir: "/dl/Tiebreakers-18951".into(),
    }));
    s.apply(&DownloadEvent::StageChanged {
        id: 1,
        stage: DownloadStage::Downloading,
    });
    s.apply(&beatmap(BeatmapStage::Downloading, 10));
    s.apply(&DownloadEvent::BeatmapProgress {
        id: 1,
        beatmapset_id: 10,
        downloaded: 400,
        total: 1000,
    });

    assert_eq!(s.name, "Tiebreakers");
    assert_eq!(s.total, 947);
    assert_eq!(s.phase, Phase::Downloading);
    assert_eq!(s.active, 1);
    assert_eq!(s.bytes_done, 400);

    s.apply(&beatmap(BeatmapStage::Success, 10));
    s.apply(&DownloadEvent::OverallProgress {
        id: 1,
        downloaded: 1,
        skipped: 5,
        failed: 0,
        unverified: 0,
    });

    assert_eq!(s.active, 0);
    assert_eq!(s.bytes_done, 1000, "a finished set counts at its full size");
    assert_eq!((s.downloaded, s.skipped), (1, 5));
}

#[test]
fn a_failed_set_does_not_count_its_partial_bytes() {
    let mut s = status();
    s.apply(&beatmap(BeatmapStage::Downloading, 10));
    s.apply(&DownloadEvent::BeatmapProgress {
        id: 1,
        beatmapset_id: 10,
        downloaded: 400,
        total: 1000,
    });

    s.apply(&beatmap(BeatmapStage::Failed, 10));

    assert_eq!(s.bytes_done, 0);
}

#[test]
fn finishing_records_failures_and_the_summary() {
    let mut s = status();
    s.apply(&DownloadEvent::FailedMaps {
        id: 1,
        failures: vec![FailedMap {
            beatmapset_id: 7,
            title: Some("Some Song".into()),
            reason: FailureReason::NotFound,
        }],
    });
    s.apply(&DownloadEvent::Finished {
        id: 1,
        summary: DownloadSummary {
            downloaded: 900,
            skipped: 46,
            failed: 1,
            unverified: 0,
        },
    });

    assert_eq!(s.phase, Phase::Done);
    assert_eq!((s.downloaded, s.skipped, s.failed), (900, 46, 1));
    assert_eq!(s.failures[0].reason, "not on any mirror");
}

#[test]
fn a_cancelled_run_stays_cancelled() {
    let mut s = status();
    s.apply(&DownloadEvent::Failed {
        id: 1,
        message: ABORTED.into(),
    });
    s.apply(&DownloadEvent::StageChanged {
        id: 1,
        stage: DownloadStage::Completed,
    });

    assert_eq!(s.phase, Phase::Cancelled);
    assert_eq!(s.error, None);
}

#[test]
fn a_pipeline_error_is_kept_for_the_user() {
    let mut s = status();
    s.apply(&DownloadEvent::Failed {
        id: 1,
        message: "collection not found (404)".into(),
    });

    assert_eq!(s.phase, Phase::Failed);
    assert_eq!(s.error.as_deref(), Some("collection not found (404)"));
}

#[test]
fn queueing_rejects_something_that_is_not_a_collection_link() {
    let downloads = Downloads::new(Arc::new(|_| {}));

    let err = downloads.enqueue("hello").unwrap_err();

    assert_eq!(err.to_string(), super::NOT_A_COLLECTION_LINK);
    assert!(downloads.list().is_empty());
}
