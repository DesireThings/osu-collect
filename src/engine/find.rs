//! Finding maps without a collection, for the GUI: the TUI's find form as plain
//! values, the route it takes (osu! API or nzbasic), one page of results, and
//! the download of the picked map sets.
//!
//! The form logic stays in [`FindSource`]: criteria are poured into one and its
//! own `build_plan` / `resolved_route` / `folder_tag` / `run_label` decide, so
//! both frontends search the same way and download into the same folders.

use super::{
    Downloads,
    download::{RunSettings, run_settings},
};
use crate::{
    app::find_source::{FindChoices, FindPlan, FindRoute, FindSource},
    config::{constants::USER_AGENT, load_config_or_default},
    core::search::{SearchService, shared_service},
    download::{DownloadId, IdsDownloadRequest, IdsRunSource},
    utils::{AppError, Result},
};
use futures_util::{StreamExt, stream};
use osu_downloader::{
    filter::{BeatmapDetails, BeatmapStatus, FilterClient},
    search::BeatmapSetMeta,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    sync::LazyLock,
};

/// Diff ids per nzbasic `beatmapDetails` request.
const DETAILS_PAGE: usize = 250;
/// Diff ids whose details a result page looks up; sets past them show by id.
const DETAILS_MAX: usize = 2_000;
/// `beatmapDetails` requests in flight at once.
const DETAILS_CONCURRENT: usize = 4;

static FILTER_CLIENT: LazyLock<FilterClient> =
    LazyLock::new(|| FilterClient::with_user_agent(USER_AGENT));

/// The find form as plain values. Choice fields are positions in the lists
/// [`options`] returns; text fields take the TUI's syntax (`7+`, `5.5..7`,
/// `2020..2024`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct FindCriteria {
    pub preset: usize,
    pub special: usize,
    pub mode: usize,
    pub status: usize,
    pub sort: usize,
    pub explicit: usize,
    pub genre: usize,
    pub language: usize,
    pub extra: Vec<usize>,
    pub query: String,
    pub stars: String,
    pub ar: String,
    pub cs: String,
    pub od: String,
    pub hp: String,
    pub bpm: String,
    pub length: String,
    pub keys: String,
    pub favourites: String,
    pub ranked: String,
    pub artist: String,
    pub creator: String,
    pub title: String,
    pub limit: String,
}

impl Default for FindCriteria {
    /// The TUI's untouched form (status "has leaderboard", newest ranked first).
    fn default() -> Self {
        Self::from_source(&FindSource::new())
    }
}

impl FindCriteria {
    fn from_source(source: &FindSource) -> Self {
        let FindChoices {
            preset,
            special,
            mode,
            status,
            sort,
            explicit,
            genre,
            language,
            extra,
        } = source.choices();
        let text = |field: &crate::app::InputField| field.value.clone();
        Self {
            preset,
            special,
            mode,
            status,
            sort,
            explicit,
            genre,
            language,
            extra,
            query: text(&source.query),
            stars: text(&source.stars),
            ar: text(&source.ar),
            cs: text(&source.cs),
            od: text(&source.od),
            hp: text(&source.hp),
            bpm: text(&source.bpm),
            length: text(&source.length),
            keys: text(&source.keys),
            favourites: text(&source.favourites),
            ranked: text(&source.ranked),
            artist: text(&source.artist),
            creator: text(&source.creator),
            title: text(&source.title),
            limit: text(&source.limit),
        }
    }

    fn to_source(&self) -> Result<FindSource> {
        let mut source = FindSource::new();
        source
            .set_choices(&FindChoices {
                preset: self.preset,
                special: self.special,
                mode: self.mode,
                status: self.status,
                sort: self.sort,
                explicit: self.explicit,
                genre: self.genre,
                language: self.language,
                extra: self.extra.clone(),
            })
            .map_err(AppError::other_dynamic)?;
        for (field, value) in [
            (&mut source.query, &self.query),
            (&mut source.stars, &self.stars),
            (&mut source.ar, &self.ar),
            (&mut source.cs, &self.cs),
            (&mut source.od, &self.od),
            (&mut source.hp, &self.hp),
            (&mut source.bpm, &self.bpm),
            (&mut source.length, &self.length),
            (&mut source.keys, &self.keys),
            (&mut source.favourites, &self.favourites),
            (&mut source.ranked, &self.ranked),
            (&mut source.artist, &self.artist),
            (&mut source.creator, &self.creator),
            (&mut source.title, &self.title),
            (&mut source.limit, &self.limit),
        ] {
            field.set_value(value.as_str());
        }
        Ok(source)
    }

    /// A fresh form with preset `idx` applied: every criterion reset, then the
    /// preset's own filled in.
    pub fn with_preset(idx: usize) -> Result<Self> {
        FindSource::with_preset(idx)
            .map(|source| Self::from_source(&source))
            .map_err(AppError::other_dynamic)
    }
}

/// The choice lists, in the order [`FindCriteria`]'s positions index.
#[derive(Debug, Clone, Serialize)]
pub struct FindOptions {
    pub presets: Vec<&'static str>,
    pub specials: Vec<&'static str>,
    pub modes: Vec<&'static str>,
    pub statuses: Vec<&'static str>,
    pub sorts: Vec<&'static str>,
    pub explicit: Vec<&'static str>,
    pub genres: Vec<&'static str>,
    pub languages: Vec<&'static str>,
    pub extras: Vec<&'static str>,
    pub defaults: FindCriteria,
}

pub fn options() -> FindOptions {
    let source = FindSource::new();
    FindOptions {
        presets: source.preset_labels().to_vec(),
        specials: source.special_labels().to_vec(),
        modes: source.mode_labels().to_vec(),
        statuses: source.status_labels().to_vec(),
        sorts: source.sort_labels(),
        explicit: source.explicit_labels().to_vec(),
        genres: source.genre_labels().to_vec(),
        languages: source.language_labels().to_vec(),
        extras: source.extra_labels().to_vec(),
        defaults: FindCriteria::from_source(&source),
    }
}

/// Which service a search goes to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Route {
    /// osu!'s own search; needs the user's osu! API app.
    Osu,
    /// nzbasic's map database; needs nothing.
    Nzbasic,
}

/// What the form would do right now, for the live line under it.
#[derive(Debug, Clone, Serialize)]
pub struct FindCheck {
    /// `None` when two criteria need different services.
    pub route: Option<Route>,
    /// Why the search can't run as is: the clash, or a field that doesn't parse.
    pub problem: Option<String>,
}

pub fn check(criteria: &FindCriteria) -> FindCheck {
    let source = match criteria.to_source() {
        Ok(source) => source,
        Err(e) => {
            return FindCheck {
                route: None,
                problem: Some(e.to_string()),
            };
        }
    };
    match source.resolved_route() {
        FindRoute::Conflict { nzbasic, osu } => FindCheck {
            route: None,
            problem: Some(format!(
                "{nzbasic} only works with nzbasic, and {osu} only with the osu! API. Change one of them."
            )),
        },
        route => FindCheck {
            route: Some(if route == FindRoute::Nzbasic {
                Route::Nzbasic
            } else {
                Route::Osu
            }),
            problem: source.build_plan(None).err(),
        },
    }
}

/// One map set in the results.
#[derive(Debug, Clone, Serialize)]
pub struct FindRow {
    pub id: u32,
    /// Empty when nothing could be looked up for the set.
    pub title: String,
    pub artist: String,
    pub creator: String,
    pub status: String,
    /// Lowest and highest star rating among its difficulties.
    pub stars: Option<(f64, f64)>,
    pub difficulties: usize,
    pub favourites: Option<u32>,
    pub plays: Option<u32>,
    /// Download size in bytes, when the service says.
    pub size: Option<u64>,
}

/// One page of results.
#[derive(Debug, Clone, Serialize)]
pub struct FindPage {
    pub route: Route,
    pub rows: Vec<FindRow>,
    /// Matches in all: osu!'s count across pages, or nzbasic's set count.
    pub total: u64,
    /// Pass back to [`find`] for the next page (osu! API only).
    pub next_cursor: Option<String>,
}

/// Run the search, or fetch the page after `cursor`.
pub async fn find(criteria: &FindCriteria, cursor: Option<String>) -> Result<FindPage> {
    let source = criteria.to_source()?;
    let plan = source.build_plan(cursor).map_err(AppError::other_dynamic)?;
    match plan {
        FindPlan::Osu(query) => {
            let results = shared_service().search(&query).await?;
            Ok(FindPage {
                route: Route::Osu,
                rows: results.beatmapsets.iter().map(osu_row).collect(),
                total: results.total,
                next_cursor: results.cursor_string,
            })
        }
        FindPlan::Nzbasic(query) => {
            let results = FILTER_CLIENT
                .fetch(&query)
                .await
                .map_err(|e| AppError::other_dynamic(format!("nzbasic: {e}")))?;
            let details = details_for(&results.ids).await;
            let rows = nzbasic_rows(&results.set_ids, &details, &results.size_map);
            Ok(FindPage {
                route: Route::Nzbasic,
                total: rows.len() as u64,
                rows,
                next_cursor: None,
            })
        }
    }
}

fn osu_row(set: &BeatmapSetMeta) -> FindRow {
    FindRow {
        id: set.id,
        title: set.title.clone(),
        artist: set.artist.clone(),
        creator: set.creator.clone(),
        status: set.status.clone(),
        stars: star_span(set.beatmaps.iter().map(|b| b.difficulty_rating)),
        difficulties: set.beatmaps.len(),
        favourites: Some(set.favourite_count),
        plays: Some(set.play_count),
        size: None,
    }
}

fn star_span(stars: impl Iterator<Item = f64>) -> Option<(f64, f64)> {
    stars.fold(None, |span, s| match span {
        None => Some((s, s)),
        Some((lo, hi)) => Some((f64::min(lo, s), f64::max(hi, s))),
    })
}

/// Look up the difficulties behind nzbasic's ids, a few pages at a time. A page
/// that fails is skipped: its sets show by id rather than failing the search.
async fn details_for(diff_ids: &[u32]) -> Vec<BeatmapDetails> {
    let pages: Vec<Vec<u32>> = diff_ids
        .iter()
        .take(DETAILS_MAX)
        .copied()
        .collect::<Vec<_>>()
        .chunks(DETAILS_PAGE)
        .map(<[u32]>::to_vec)
        .collect();
    stream::iter(pages)
        .map(|page| async move { FILTER_CLIENT.details(&page).await.unwrap_or_default() })
        .buffered(DETAILS_CONCURRENT)
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .flatten()
        .collect()
}

/// One row per set, in nzbasic's order, filled from whatever details came back.
fn nzbasic_rows(
    set_ids: &[u32],
    details: &[BeatmapDetails],
    sizes: &HashMap<u32, u64>,
) -> Vec<FindRow> {
    let mut by_set: HashMap<u32, Vec<&BeatmapDetails>> = HashMap::new();
    for diff in details {
        by_set.entry(diff.set_id).or_default().push(diff);
    }
    let mut seen = HashSet::new();
    set_ids
        .iter()
        .filter(|id| seen.insert(**id))
        .map(|&id| {
            let diffs = by_set.get(&id).map(Vec::as_slice).unwrap_or_default();
            let first = diffs.first();
            FindRow {
                id,
                title: first.map(|d| d.title.clone()).unwrap_or_default(),
                artist: first.map(|d| d.artist.clone()).unwrap_or_default(),
                creator: first.map(|d| d.creator.clone()).unwrap_or_default(),
                status: first
                    .and_then(|d| d.status)
                    .map(status_name)
                    .unwrap_or_default()
                    .to_string(),
                stars: star_span(diffs.iter().map(|d| d.stars)),
                difficulties: diffs.len(),
                favourites: None,
                plays: None,
                size: sizes.get(&id).copied(),
            }
        })
        .collect()
}

fn status_name(status: BeatmapStatus) -> &'static str {
    match status {
        BeatmapStatus::Ranked => "ranked",
        BeatmapStatus::Loved => "loved",
        BeatmapStatus::Approved => "approved",
        BeatmapStatus::Pending => "pending",
        BeatmapStatus::Wip => "wip",
        BeatmapStatus::Graveyard => "graveyard",
    }
}

/// What to download from the results.
#[derive(Debug, Clone, Deserialize)]
pub struct FindPick {
    /// The criteria that produced the results; they name the run and its folder.
    pub criteria: FindCriteria,
    /// The service the results came from.
    pub route: Route,
    pub beatmapset_ids: Vec<u32>,
    /// Sizes the results carried, by set id.
    #[serde(default)]
    pub sizes: HashMap<u32, u64>,
}

/// Queue the download of `pick`, with the mirrors and folders in the config.
pub fn download(downloads: &Downloads, pick: &FindPick) -> Result<DownloadId> {
    let request = build_find_request(pick, run_settings(&load_config_or_default())?)?;
    downloads.enqueue_find(request)
}

/// The run for `pick`: the TUI's find download, folder and name included.
fn build_find_request(pick: &FindPick, run: RunSettings) -> Result<IdsDownloadRequest> {
    let source = pick.criteria.to_source()?;
    let mut seen = HashSet::new();
    let beatmapset_ids: Vec<u32> = pick
        .beatmapset_ids
        .iter()
        .copied()
        .filter(|id| seen.insert(*id))
        .collect();
    Ok(IdsDownloadRequest {
        known_sizes: pick
            .sizes
            .iter()
            .filter(|(id, _)| seen.contains(id))
            .map(|(&id, &size)| (id, size))
            .collect(),
        beatmapset_ids,
        label: source.run_label(),
        folder_tag: source.folder_tag(),
        source: match pick.route {
            Route::Osu => IdsRunSource::Search,
            Route::Nzbasic => IdsRunSource::Filter,
        },
        config: run.config,
        auto_overwrite: false,
        skip_already_imported: run.skip_already_imported,
        osu_client: run.osu_client,
        osu_path: run.osu_path,
    })
}

#[cfg(test)]
#[path = "../../tests/unit/engine_find.rs"]
mod tests;
