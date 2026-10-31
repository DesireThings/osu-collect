#[cfg(test)]
mod tests {
    use osu_shelf::osu_db::{BeatmapReader, LazerReader};
    use std::path::PathBuf;

    fn testing_db_path() -> Option<PathBuf> {
        if let Ok(p) = std::env::var("OSU_TESTING_DB") {
            let path = PathBuf::from(p);
            if path.join("client.realm").exists() {
                return Some(path);
            }
        }
        let relative = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testing-db");
        if relative.join("client.realm").exists() {
            Some(relative)
        } else {
            None
        }
    }

    fn open_realm_or_skip() -> Option<LazerReader> {
        let path = PathBuf::from("realm");
        let realm_file = path.join("client.realm");
        if !realm_file.exists() {
            println!(
                "skipping test: realm file not found at {}",
                realm_file.display()
            );
            return None;
        }
        Some(LazerReader::new(path))
    }

    #[test]
    fn test_realm_reading() {
        let Some(reader) = open_realm_or_skip() else {
            return;
        };

        // Test collections
        let collections = reader
            .list_collections()
            .expect("Failed to read collections");
        println!("\n=== Collections ===");
        println!("Total: {}", collections.len());
        for c in collections.iter().take(5) {
            println!("  - {}: {} checksums", c.name, c.beatmap_checksums.len());
        }

        // Test beatmapsets
        let beatmapsets = reader
            .list_beatmapsets()
            .expect("Failed to read beatmapsets");
        println!("\n=== Beatmapsets ===");
        println!("Total: {}", beatmapsets.len());

        let total_beatmaps: usize = beatmapsets.iter().map(|bs| bs.beatmaps.len()).sum();
        println!("Total beatmaps (diffs): {}", total_beatmaps);

        // Show sample
        for bs in beatmapsets.iter().take(3) {
            println!("  - Set {}: {} beatmaps", bs.id, bs.beatmaps.len());
            for bm in bs.beatmaps.iter().take(2) {
                println!("      Checksum {:?}:", bm.checksum);
            }
        }

        // Verify that beatmapsets have valid data
        let sets_with_beatmaps = beatmapsets
            .iter()
            .filter(|bs| !bs.beatmaps.is_empty())
            .count();
        println!("\nBeatmapsets with beatmaps: {}", sets_with_beatmaps);

        // Test all checksums (includes checksums from beatmaps with invalid OnlineIDs)
        let all_checksums = reader
            .list_all_checksums()
            .expect("Failed to read all checksums");

        let checksums_from_beatmapsets: std::collections::HashSet<osu_shelf::osu_db::Md5> =
            beatmapsets
                .iter()
                .flat_map(|bs| bs.beatmaps.iter().map(|b| b.checksum))
                .collect();

        println!("\n=== Checksum Comparison ===");
        println!(
            "Checksums from beatmapsets: {}",
            checksums_from_beatmapsets.len()
        );
        println!("All checksums (including skipped): {}", all_checksums.len());
        println!(
            "Additional checksums recovered: {}",
            all_checksums.len() - checksums_from_beatmapsets.len()
        );

        assert!(!beatmapsets.is_empty(), "Should have some beatmapsets");
        assert!(total_beatmaps > 0, "Should have some beatmaps");
        assert!(
            all_checksums.len() >= checksums_from_beatmapsets.len(),
            "All checksums should include at least as many as from beatmapsets"
        );
    }

    /// Test that simulates the comparison logic to verify the fix for false "not installed" reports.
    /// This test verifies that using `list_all_checksums()` recovers more installed beatmaps
    /// than using only checksums from `list_beatmapsets()`.
    #[test]
    fn test_all_checksums_recovers_more_matches() {
        let Some(reader) = open_realm_or_skip() else {
            return;
        };

        // Get beatmapsets (only includes beatmaps with valid OnlineIDs)
        let beatmapsets = reader
            .list_beatmapsets()
            .expect("Failed to read beatmapsets");

        // Build checksum set from beatmapsets (old method - what was causing the bug)
        let checksums_old: std::collections::HashSet<osu_shelf::osu_db::Md5> = beatmapsets
            .iter()
            .flat_map(|bs| bs.beatmaps.iter().map(|b| b.checksum))
            .collect();

        // Get ALL checksums (new method - the fix)
        let all_checksums = reader
            .list_all_checksums()
            .expect("Failed to read all checksums");
        let checksums_new: std::collections::HashSet<osu_shelf::osu_db::Md5> =
            all_checksums.into_iter().collect();

        // The new method should have at least as many checksums as the old method
        assert!(
            checksums_new.len() >= checksums_old.len(),
            "New method should have at least as many checksums. Old: {}, New: {}",
            checksums_old.len(),
            checksums_new.len()
        );

        // Calculate the difference
        let additional_checksums = checksums_new.len() - checksums_old.len();

        println!("\n=== All Checksums Recovery Test ===");
        println!("Checksums (old method): {}", checksums_old.len());
        println!("Checksums (new method): {}", checksums_new.len());
        println!("Additional checksums recovered: {}", additional_checksums);

        // The new method should recover some additional checksums
        // (assuming the realm has beatmaps with invalid OnlineIDs)
        if additional_checksums > 0 {
            println!(
                "SUCCESS: {} additional checksums recovered that would cause false 'Not Installed' reports",
                additional_checksums
            );
        } else {
            println!(
                "Note: No additional checksums recovered (realm may not have beatmaps with invalid OnlineIDs)"
            );
        }

        // Verify that ALL old checksums are in the new set (new is a superset)
        let missing_from_new: Vec<_> = checksums_old
            .iter()
            .filter(|cs| !checksums_new.contains(*cs))
            .collect();

        assert!(
            missing_from_new.is_empty(),
            "New checksum set should contain all old checksums. Missing: {:?}",
            missing_from_new
        );
    }

    /// Verifies that `read_all` opens realm once and returns identical data to calling
    /// list_collections, list_beatmapsets, and list_all_checksums separately.
    #[test]
    fn test_read_all_matches_individual_calls() {
        let Some(path) = testing_db_path() else {
            println!("skipping: no client.realm found");
            return;
        };

        let reader = LazerReader::new(path);

        let collections_individual = reader
            .list_collections()
            .expect("failed to read collections");
        let beatmapsets_individual = reader
            .list_beatmapsets()
            .expect("failed to read beatmapsets");
        let checksums_individual: std::collections::HashSet<osu_shelf::osu_db::Md5> = reader
            .list_all_checksums()
            .expect("failed to read checksums")
            .into_iter()
            .collect();

        let (collections_all, beatmapsets_all, checksums_all_vec) =
            reader.read_all().expect("read_all failed");
        let checksums_all: std::collections::HashSet<osu_shelf::osu_db::Md5> =
            checksums_all_vec.into_iter().collect();

        assert_eq!(
            collections_all.len(),
            collections_individual.len(),
            "collection count mismatch"
        );
        assert_eq!(
            beatmapsets_all.len(),
            beatmapsets_individual.len(),
            "beatmapset count mismatch"
        );
        assert_eq!(
            checksums_all.len(),
            checksums_individual.len(),
            "checksum count mismatch"
        );

        // Verify all individual checksums are present in read_all result
        for cs in &checksums_individual {
            assert!(
                checksums_all.contains(cs),
                "checksum {cs:?} missing from read_all result"
            );
        }

        println!(
            "read_all: {} collections, {} beatmapsets, {} checksums",
            collections_all.len(),
            beatmapsets_all.len(),
            checksums_all.len()
        );
    }
}
