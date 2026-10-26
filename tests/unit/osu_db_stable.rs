use super::StableReader;
use crate::osu_db::{BeatmapReader, checksum};
use std::{fs, path::Path};

/// osu! stable's current `osu!.db` version (a 2026 client).
const CURRENT: u32 = 20260924;
/// Before 20250107, when star ratings were stored as doubles.
const BEFORE_FLOAT_RATINGS: u32 = 20241001;

/// An `osu!.db` written by hand in stable's on-disk layout (version 20191106 or
/// later: no per-beatmap size prefix), so the test checks the real format
/// rather than the parser against its own writer.
struct OsuDb {
    version: u32,
    beatmaps: Vec<u8>,
    count: u32,
}

impl OsuDb {
    fn new(version: u32) -> Self {
        Self {
            version,
            beatmaps: Vec::new(),
            count: 0,
        }
    }

    fn beatmap(mut self, beatmapset_id: i32, beatmap_id: i32, md5: &str) -> Self {
        let out = &mut self.beatmaps;
        for text in ["Artist", "Artist", "Title", "Title", "Mapper", "Insane"] {
            string(out, text);
        }
        string(out, "audio.mp3");
        string(out, md5);
        string(out, "map.osu");
        out.push(4); // ranked
        out.extend_from_slice(&[0; 6]); // circle, slider, spinner counts
        out.extend_from_slice(&0u64.to_le_bytes()); // last modified
        for _ in 0..4 {
            out.extend_from_slice(&5.0f32.to_le_bytes()); // AR, CS, HP, OD
        }
        out.extend_from_slice(&1.4f64.to_le_bytes()); // slider velocity
        // osu!, taiko, catch, mania star ratings: one nomod entry for osu!.
        star_ratings(out, self.version, &[(0, 5.25)]);
        for _ in 0..3 {
            star_ratings(out, self.version, &[]);
        }
        out.extend_from_slice(&[0; 12]); // drain, total, preview time
        out.extend_from_slice(&1u32.to_le_bytes()); // one timing point
        out.extend_from_slice(&300.0f64.to_le_bytes());
        out.extend_from_slice(&0.0f64.to_le_bytes());
        out.push(1);
        out.extend_from_slice(&beatmap_id.to_le_bytes());
        out.extend_from_slice(&beatmapset_id.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes()); // thread id
        out.extend_from_slice(&[9; 4]); // grades: unplayed
        out.extend_from_slice(&0u16.to_le_bytes()); // local offset
        out.extend_from_slice(&0.7f32.to_le_bytes()); // stack leniency
        out.push(0); // osu! mode
        out.push(0); // song source
        out.push(0); // tags
        out.extend_from_slice(&0u16.to_le_bytes()); // online offset
        out.push(0); // title font
        out.push(1); // unplayed
        out.extend_from_slice(&0u64.to_le_bytes()); // last played
        out.push(0); // osz2
        string(out, "folder");
        out.extend_from_slice(&0u64.to_le_bytes()); // last online check
        out.extend_from_slice(&[0; 5]); // ignore sounds/skin, storyboard, video, visual override
        out.extend_from_slice(&0u32.to_le_bytes()); // last modification time
        out.push(0); // mania scroll speed
        self.count += 1;
        self
    }

    fn write(&self, dir: &Path) {
        let mut out = Vec::new();
        out.extend_from_slice(&self.version.to_le_bytes());
        out.extend_from_slice(&1u32.to_le_bytes()); // folder count
        out.push(1); // account unlocked
        out.extend_from_slice(&0u64.to_le_bytes()); // unlock date
        string(&mut out, "player");
        out.extend_from_slice(&self.count.to_le_bytes());
        out.extend_from_slice(&self.beatmaps);
        out.extend_from_slice(&0u32.to_le_bytes()); // user permissions
        fs::write(dir.join("osu!.db"), out).unwrap();
    }
}

fn string(out: &mut Vec<u8>, text: &str) {
    assert!(text.len() < 0x80, "one-byte ULEB128 length only");
    out.push(0x0b);
    out.push(text.len() as u8);
    out.extend_from_slice(text.as_bytes());
}

/// Int-Double pairs before 20250107, Int-Float pairs from then on.
fn star_ratings(out: &mut Vec<u8>, version: u32, ratings: &[(u32, f64)]) {
    out.extend_from_slice(&(ratings.len() as u32).to_le_bytes());
    for &(mods, stars) in ratings {
        out.push(0x08);
        out.extend_from_slice(&mods.to_le_bytes());
        if version < 20250107 {
            out.push(0x0d);
            out.extend_from_slice(&stars.to_le_bytes());
        } else {
            out.push(0x0c);
            out.extend_from_slice(&(stars as f32).to_le_bytes());
        }
    }
}

const MD5_A: &str = "0123456789abcdef0123456789abcdef";
const MD5_B: &str = "fedcba9876543210fedcba9876543210";
const MD5_C: &str = "00112233445566778899aabbccddeeff";

fn read_sets(version: u32) -> Vec<(u32, Vec<checksum::Md5>)> {
    let dir = tempfile::tempdir().unwrap();
    OsuDb::new(version)
        .beatmap(100, 1000, MD5_A)
        .beatmap(100, 1001, MD5_B)
        .beatmap(200, 2000, MD5_C)
        // Unsubmitted maps have no online ids and are left out.
        .beatmap(-1, -1, MD5_C)
        .write(dir.path());

    let mut sets: Vec<_> = StableReader::new(dir.path().to_path_buf())
        .list_beatmapsets()
        .expect("osu!.db reads")
        .into_iter()
        .map(|set| {
            let mut sums: Vec<_> = set.beatmaps.iter().map(|b| b.checksum).collect();
            sums.sort();
            (set.id, sums)
        })
        .collect();
    sets.sort();
    sets
}

fn expected_sets() -> Vec<(u32, Vec<checksum::Md5>)> {
    let md5 = |hex| checksum::parse_hex(hex).unwrap();
    let mut first = vec![md5(MD5_A), md5(MD5_B)];
    first.sort();
    vec![(100, first), (200, vec![md5(MD5_C)])]
}

#[test]
fn reads_an_osu_db_from_a_current_stable_client() {
    assert_eq!(read_sets(CURRENT), expected_sets());
}

#[test]
fn still_reads_an_osu_db_from_before_float_star_ratings() {
    assert_eq!(read_sets(BEFORE_FLOAT_RATINGS), expected_sets());
}

#[test]
fn an_unreadable_osu_db_is_an_error_not_an_empty_library() {
    let dir = tempfile::tempdir().unwrap();
    // A version number, then bytes no parser accepts.
    let mut bytes = CURRENT.to_le_bytes().to_vec();
    bytes.extend_from_slice(&[0xff; 64]);
    fs::write(dir.path().join("osu!.db"), bytes).unwrap();

    let err = StableReader::new(dir.path().to_path_buf())
        .list_beatmapsets()
        .expect_err("garbage must not read as an empty library");
    assert!(err.starts_with("Couldn't read osu!.db at "), "{err}");
    assert!(err.contains("newer osu! version"), "{err}");
}

#[test]
fn an_unreadable_collection_db_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let mut bytes = CURRENT.to_le_bytes().to_vec();
    bytes.extend_from_slice(&1u32.to_le_bytes()); // one collection
    bytes.extend_from_slice(&[0xff; 16]); // not a string marker
    fs::write(dir.path().join("collection.db"), bytes).unwrap();

    let err = StableReader::new(dir.path().to_path_buf())
        .list_collections()
        .expect_err("garbage must not read as no collections");
    assert!(err.starts_with("Couldn't read collection.db at "), "{err}");
}

#[test]
fn a_new_empty_osu_db_is_an_empty_library() {
    let dir = tempfile::tempdir().unwrap();
    OsuDb::new(CURRENT).write(dir.path());
    let sets = StableReader::new(dir.path().to_path_buf())
        .list_beatmapsets()
        .unwrap();
    assert!(sets.is_empty());
}
