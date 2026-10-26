#[cfg(test)]
mod tests {
    use osu_collect::osu_db::{BeatmapReader, StableReader};
    use std::path::PathBuf;

    fn testing_db_path() -> Option<PathBuf> {
        // Prefer env var, fall back to testing-db/ relative to workspace root
        if let Ok(p) = std::env::var("OSU_TESTING_DB") {
            let path = PathBuf::from(p);
            if path.join("osu!.db").exists() {
                return Some(path);
            }
        }
        let relative = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testing-db");
        if relative.join("osu!.db").exists() {
            Some(relative)
        } else {
            None
        }
    }

    #[test]
    fn test_stable_beatmapsets_read() {
        let Some(path) = testing_db_path() else {
            println!("skipping: no testing-db/osu!.db found");
            return;
        };

        let reader = StableReader::new(path);
        let beatmapsets = reader
            .list_beatmapsets()
            .expect("failed to read beatmapsets");

        let total_beatmaps: usize = beatmapsets.iter().map(|bs| bs.beatmaps.len()).sum();
        println!(
            "stable: {} beatmapsets, {} total beatmaps",
            beatmapsets.len(),
            total_beatmaps
        );
        // An unsupported osu!.db version fails the `expect` above.
        // All beatmapset IDs must be non-zero when entries exist.
        for bs in &beatmapsets {
            assert!(bs.id > 0, "beatmapset id must be non-zero");
        }
    }
}
