// App-wide state that outlives a screen: the collections the user added, the
// last update scan and what's picked from it, the last search and its picks,
// and the downloads the backend is running. Switching pages in the sidebar keeps all of it.
import {
  cancelDownload,
  downloadFound,
  downloadUpdates,
  findMaps,
  listDownloads,
  onDownloadStatus,
  onScanProgress,
  previewCollection,
  scanUpdates,
  startDownload,
  type CollectionPreview,
  type DownloadStatus,
  type FindCriteria,
  type FindRoute,
  type FindRow,
  type ScanProgress,
  type UpdateReport,
} from "./api";

export type Entry =
  | { key: number; input: string; state: "loading" }
  | { key: number; input: string; state: "ready"; preview: CollectionPreview }
  | { key: number; input: string; state: "error"; message: string };

export const store = $state({
  entries: [] as Entry[],
  /** Latest status per download id. */
  downloads: {} as Record<number, DownloadStatus>,
  /** Errors from asking the backend to start a download, per entry key. */
  startErrors: {} as Record<number, string>,
  updates: {
    scanning: false,
    progress: null as ScanProgress | null,
    report: null as UpdateReport | null,
    error: null as string | null,
    /** Collections ticked for download. */
    picked: [] as number[],
    /** Previously deleted sets picked again, as `${collection}:${set}`. */
    reincluded: [] as string[],
    downloadError: null as string | null,
    /** A download was started from this scan; check again before the next. */
    downloaded: false,
  },
  find: {
    /** The form as edited; null until the options load. */
    criteria: null as FindCriteria | null,
    /** The user picked a sort; until then it follows the search text. */
    sortChosen: false,
    searching: false,
    loadingMore: false,
    error: null as string | null,
    /** The form as it was when the shown results were searched. */
    searched: null as FindCriteria | null,
    route: null as FindRoute | null,
    rows: [] as FindRow[],
    total: 0,
    nextCursor: null as string | null,
    picked: [] as number[],
    downloadError: null as string | null,
  },
});

let nextKey = 0;
let listening = false;
let listeningScan = false;

/** Pick up downloads already running and follow their updates. Idempotent. */
export async function initDownloads() {
  if (listening) return;
  listening = true;
  await onDownloadStatus((status) => {
    store.downloads[status.id] = status;
  });
  for (const status of await listDownloads()) {
    store.downloads[status.id] = status;
  }
}

export async function addCollection(input: string) {
  const key = nextKey++;
  store.entries.push({ key, input, state: "loading" });
  let updated: Entry;
  try {
    const preview = await previewCollection(input);
    const duplicate = store.entries.some(
      (e) => e.state === "ready" && e.preview.id === preview.id,
    );
    updated = duplicate
      ? { key, input, state: "error", message: `"${preview.name}" is already in the list.` }
      : { key, input, state: "ready", preview };
  } catch (e) {
    updated = { key, input, state: "error", message: String(e) };
  }
  const index = store.entries.findIndex((e) => e.key === key);
  if (index !== -1) store.entries[index] = updated;
}

export function removeEntry(key: number) {
  store.entries = store.entries.filter((e) => e.key !== key);
  delete store.startErrors[key];
}

/** The newest download of a collection, if any. */
export function downloadFor(collectionId: number): DownloadStatus | undefined {
  let latest: DownloadStatus | undefined;
  for (const status of Object.values(store.downloads)) {
    if (
      status.kind === "collection" &&
      status.collection_id === collectionId &&
      (!latest || status.id > latest.id)
    ) {
      latest = status;
    }
  }
  return latest;
}

export const isActive = (status: DownloadStatus | undefined) =>
  !!status && !["done", "failed", "cancelled"].includes(status.phase);

export async function download(entry: Entry) {
  delete store.startErrors[entry.key];
  try {
    await startDownload(entry.input);
  } catch (e) {
    store.startErrors[entry.key] = String(e);
  }
}

export const cancel = (status: DownloadStatus) => cancelDownload(status.id);

/** The newest update download, if any. */
export function latestUpdateRun(): DownloadStatus | undefined {
  let latest: DownloadStatus | undefined;
  for (const status of Object.values(store.downloads)) {
    if (status.kind === "update" && (!latest || status.id > latest.id)) latest = status;
  }
  return latest;
}

export const reincludeKey = (collectionId: number, setId: number) => `${collectionId}:${setId}`;

/** Scan for updates; afterwards every collection with something new is ticked. */
export async function scanForUpdates() {
  const u = store.updates;
  if (u.scanning) return;
  if (!listeningScan) {
    listeningScan = true;
    await onScanProgress((progress) => {
      if (store.updates.scanning) store.updates.progress = progress;
    });
  }
  u.scanning = true;
  u.progress = null;
  u.error = null;
  u.downloadError = null;
  u.downloaded = false;
  try {
    const report = await scanUpdates();
    u.report = report;
    u.picked = report.collections
      .filter((c) => c.missing.some((m) => !m.previously_deleted))
      .map((c) => c.id);
    u.reincluded = [];
  } catch (e) {
    u.error = String(e);
  } finally {
    u.scanning = false;
    u.progress = null;
  }
}

/** Map sets the current pick downloads: each once, however many collections miss it. */
export function pickedSetIds(): Set<number> {
  const u = store.updates;
  const ids = new Set<number>();
  for (const collection of u.report?.collections ?? []) {
    if (!u.picked.includes(collection.id)) continue;
    for (const set of collection.missing) {
      if (
        !set.previously_deleted ||
        u.reincluded.includes(reincludeKey(collection.id, set.beatmapset_id))
      ) {
        ids.add(set.beatmapset_id);
      }
    }
  }
  return ids;
}

export async function downloadPickedUpdates() {
  const u = store.updates;
  u.downloadError = null;
  try {
    await downloadUpdates({
      collection_ids: [...u.picked],
      reincluded: u.reincluded.map((key) => {
        const [collection_id, beatmapset_id] = key.split(":").map(Number);
        return { collection_id, beatmapset_id };
      }),
    });
    u.downloaded = true;
  } catch (e) {
    u.downloadError = String(e);
  }
}

/** The newest find download, if any. */
export function latestFindRun(): DownloadStatus | undefined {
  let latest: DownloadStatus | undefined;
  for (const status of Object.values(store.downloads)) {
    if (status.kind === "find" && (!latest || status.id > latest.id)) latest = status;
  }
  return latest;
}

/** Search with the form as it is. Results start unticked: a search can match
 * thousands of map sets, and the user picks what they want. */
export async function search() {
  const f = store.find;
  if (!f.criteria || f.searching) return;
  const criteria = $state.snapshot(f.criteria);
  f.searching = true;
  f.error = null;
  f.downloadError = null;
  try {
    const page = await findMaps(criteria);
    f.searched = criteria;
    f.route = page.route;
    f.rows = page.rows;
    f.total = page.total;
    f.nextCursor = page.next_cursor;
    f.picked = [];
  } catch (e) {
    f.error = String(e);
  } finally {
    f.searching = false;
  }
}

/** The next page of the shown results (osu! API only). */
export async function loadMore() {
  const f = store.find;
  if (!f.searched || !f.nextCursor || f.loadingMore) return;
  f.loadingMore = true;
  f.error = null;
  try {
    const page = await findMaps(f.searched, f.nextCursor);
    const known = new Set(f.rows.map((r) => r.id));
    const fresh = page.rows.filter((r) => !known.has(r.id));
    f.rows = [...f.rows, ...fresh];
    f.nextCursor = page.next_cursor;
  } catch (e) {
    f.error = String(e);
  } finally {
    f.loadingMore = false;
  }
}

/** Queue the ticked sets; resolves to the new run's id, or null when it failed. */
export async function downloadPicked(): Promise<number | null> {
  const f = store.find;
  if (!f.searched || !f.route) return null;
  f.downloadError = null;
  const sizes: Record<number, number> = {};
  for (const row of f.rows) {
    if (row.size !== null && f.picked.includes(row.id)) sizes[row.id] = row.size;
  }
  try {
    return await downloadFound({
      criteria: f.searched,
      route: f.route,
      beatmapset_ids: f.rows.filter((r) => f.picked.includes(r.id)).map((r) => r.id),
      sizes,
    });
  } catch (e) {
    f.downloadError = String(e);
    return null;
  }
}
