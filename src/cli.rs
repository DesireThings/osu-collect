use crate::{
    app::update_source::MissingBeatmapset,
    download::lazer_import::{IMPORT_FOLDER_NAME, build_import_folder},
    engine::updates,
    osu_db::{BeatmapReader, LazerReader, OsuClient, StableReader},
};
use std::{path::PathBuf, time::Instant};
use tracing::{info, warn};

#[derive(Debug)]
pub struct UpdateCollectionsArgs {
    pub db: Option<PathBuf>,
    pub client: OsuClient,
    pub dry_run: bool,
    pub verbose: bool,
}

#[derive(Debug)]
pub struct LazerImportArgs {
    /// Download directory holding the run folders; `None` uses the last one
    /// typed into the home form.
    pub dir: Option<PathBuf>,
    /// Import folder to build; `None` puts it inside the download directory.
    pub out: Option<PathBuf>,
}

#[derive(Debug)]
pub enum CliCommand {
    UpdateCollections(UpdateCollectionsArgs),
    LazerImport(LazerImportArgs),
}

/// Parse CLI arguments. Returns `None` when no subcommand is given (TUI mode).
pub fn parse_args() -> Result<Option<CliCommand>, String> {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.is_empty() {
        return Ok(None);
    }

    match args[0].as_str() {
        "update-collections" => {
            let cmd = parse_update_collections(&args[1..])?;
            Ok(Some(CliCommand::UpdateCollections(cmd)))
        }
        "lazer-import" => {
            let cmd = parse_lazer_import(&args[1..])?;
            Ok(Some(CliCommand::LazerImport(cmd)))
        }
        "--help" | "-h" => {
            print_help();
            std::process::exit(0);
        }
        "--version" | "-V" => {
            println!("osu-collect {}", env!("CARGO_PKG_VERSION"));
            std::process::exit(0);
        }
        other => Err(format!("unknown subcommand: {other}")),
    }
}

fn parse_update_collections(args: &[String]) -> Result<UpdateCollectionsArgs, String> {
    let mut db: Option<PathBuf> = None;
    let mut client = OsuClient::Stable;
    let mut dry_run = false;
    let mut verbose = false;
    let mut i = 0;

    while i < args.len() {
        match args[i].as_str() {
            "--db" => {
                i += 1;
                let path = args.get(i).ok_or("--db requires a path argument")?;
                db = Some(PathBuf::from(path));
            }
            "--lazer" => client = OsuClient::Lazer,
            "--stable" => client = OsuClient::Stable,
            "--dry-run" => dry_run = true,
            "--verbose" => verbose = true,
            other => return Err(format!("unknown argument: {other}")),
        }
        i += 1;
    }

    Ok(UpdateCollectionsArgs {
        db,
        client,
        dry_run,
        verbose,
    })
}

fn parse_lazer_import(args: &[String]) -> Result<LazerImportArgs, String> {
    let mut dir: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut i = 0;

    while i < args.len() {
        match args[i].as_str() {
            "--dir" => {
                i += 1;
                let path = args.get(i).ok_or("--dir requires a path argument")?;
                dir = Some(PathBuf::from(path));
            }
            "--out" => {
                i += 1;
                let path = args.get(i).ok_or("--out requires a path argument")?;
                out = Some(PathBuf::from(path));
            }
            other => return Err(format!("unknown argument: {other}")),
        }
        i += 1;
    }

    Ok(LazerImportArgs { dir, out })
}

fn print_help() {
    println!(concat!(
        "osu-collect ",
        env!("CARGO_PKG_VERSION"),
        "\n\n",
        "usage:\n",
        "  osu-collect                          launch TUI\n",
        "  osu-collect update-collections [opts] check and report missing beatmaps\n",
        "  osu-collect lazer-import [opts]       build one folder lazer imports all downloads from\n\n",
        "update-collections options:\n",
        "  --db <path>    override osu! database path\n",
        "  --stable       use osu! stable database (default)\n",
        "  --lazer        use osu! lazer database\n",
        "  --dry-run      run without writing state\n",
        "  --verbose      raise log level to debug\n\n",
        "lazer-import options:\n",
        "  --dir <path>   download directory (default: the last one used)\n",
        "  --out <path>   folder to build (default: <dir>/lazer-import)",
    ));
}

/// Build the lazer import folder from every run in the download directory and
/// print what to do with it.
pub async fn run_lazer_import(
    args: LazerImportArgs,
    last_directory: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let dir = args
        .dir
        .or_else(|| last_directory.map(PathBuf::from))
        .ok_or("no download directory known yet; use --dir")?;
    if !dir.is_dir() {
        return Err(format!("download directory not found: {}", dir.display()).into());
    }
    let out = args.out.unwrap_or_else(|| dir.join(IMPORT_FOLDER_NAME));

    let report = tokio::task::spawn_blocking({
        let (dir, out) = (dir.clone(), out.clone());
        move || build_import_folder(&dir, &out)
    })
    .await
    .map_err(|e| format!("task panicked: {e}"))??;

    println!(
        "{} run folder(s): {} set(s) unpacked, {} already there, {} collection(s) with {} map(s).",
        report.runs,
        report.unpacked,
        report.already_unpacked,
        report.collections,
        report.beatmap_hashes,
    );
    for (archive, reason) in &report.failed {
        println!("  could not unpack {}: {reason}", archive.display());
    }
    println!();
    println!("Import into osu!lazer (one pass for everything):");
    println!("  1. Settings -> General -> Run setup wizard, then Next until the import screen.");
    println!("  2. Set \"previous osu! install\" to: {}", out.display());
    println!("  3. Leave only Beatmaps and Collections ticked (untick Scores and Skins),");
    println!("     then click \"Import content from previous version\".");
    Ok(())
}

/// Run the update-collections flow without the TUI.
pub async fn run_update_collections(
    args: UpdateCollectionsArgs,
) -> Result<(), Box<dyn std::error::Error>> {
    let t_total = Instant::now();

    // Resolve osu! database path
    let db_path =
        match args.db {
            Some(p) => p,
            None => match args.client {
                OsuClient::Stable => StableReader::default_path()
                    .ok_or("could not detect osu! stable path; use --db")?,
                OsuClient::Lazer => LazerReader::default_path()
                    .ok_or("could not detect osu! lazer path; use --db")?,
            },
        };

    info!(path = %db_path.display(), "reading local database");
    let scan = updates::scan(args.client, db_path, |progress| {
        if let updates::ScanProgress::Checking {
            local_sets,
            collections,
        } = progress
        {
            info!(
                beatmapsets = local_sets,
                collections, "fetching collections from API"
            );
        }
    })
    .await?;

    let db_ms = scan.db_read.as_millis();
    let fetch_ms = scan.fetch.as_millis();
    info!(
        collections = scan.local_collections.len(),
        beatmapsets = scan.local_beatmapsets.len(),
        elapsed_ms = db_ms,
        "phase: read database"
    );

    if scan.collection_ids.is_empty() {
        warn!("no collections with recognizable osu!collector IDs found");
        println!("no updatable collections found.");
        return Ok(());
    }

    // Save updated state (unless dry-run)
    let scan = if args.dry_run {
        info!("dry-run: skipping state write");
        scan
    } else {
        tokio::task::spawn_blocking(move || {
            updates::persist(&scan);
            scan
        })
        .await
        .map_err(|e| format!("task panicked: {e}"))?
    };

    let missing = &scan.fetched.missing;
    let added_count = scan.added_since_last_scan;
    let hidden_failed_count = scan.fetched.hidden_failed_count;
    let (fetchable_count, held_back_count) = report_counts(missing);
    info!(
        missing = fetchable_count,
        held_back = held_back_count,
        elapsed_ms = fetch_ms,
        "phase: fetch + compare"
    );

    let total_ms = t_total.elapsed().as_millis();
    println!("update-collections complete: {fetchable_count} missing mapsets");
    println!("  phase db-read:       {db_ms}ms");
    println!("  phase fetch+compare: {fetch_ms}ms");
    println!("  total:               {total_ms}ms");

    if held_back_count > 0 {
        eprintln!(
            "info: {held_back_count} previously-deleted mapsets held back; re-include them from the update browse"
        );
    }
    if added_count > 0 {
        eprintln!("info: {added_count} mapsets added manually since last scan; they will remain");
    }
    if hidden_failed_count > 0 {
        eprintln!(
            "info: {hidden_failed_count} failed mapsets hidden; recheck from the TUI to include them"
        );
    }

    if !missing.is_empty() {
        println!("missing mapsets:");
        for m in missing {
            let tag = row_tag(m);
            println!(
                "  mapset {} in collection {} ({}){tag}",
                m.id, m.collection_id, m.collection_name
            );
        }
    }

    Ok(())
}

/// The `update-collections` report's headline figures: how many sets an update
/// run would enqueue, and how many the scan held back as previously deleted.
/// Held-back sets still print in the listing (tagged), so the two always sum to
/// the scanned total.
fn report_counts(missing: &[MissingBeatmapset]) -> (usize, usize) {
    let fetchable = missing.iter().filter(|m| m.included).count();
    (fetchable, missing.len() - fetchable)
}

/// The listing's per-row marker. Reads the same field [`report_counts`] counts,
/// so the number of tagged rows and the reported held-back figure cannot
/// disagree.
fn row_tag(set: &MissingBeatmapset) -> &'static str {
    if set.included { "" } else { " [held back]" }
}

#[cfg(test)]
#[path = "../tests/unit/cli.rs"]
mod tests;
