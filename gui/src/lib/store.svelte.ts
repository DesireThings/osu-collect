// App-wide state that outlives a screen: the collections the user added, the
// last update scan and what's picked from it, and the downloads the backend is
// running. Switching pages in the sidebar keeps all of it.
import {
  cancelDownload,
  downloadUpdates,
  listDownloads,
  onDownloadStatus,
  onScanProgress,
  previewCollection,
  scanUpdates,
  startDownload,
  type CollectionPreview,
  type DownloadStatus,
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
