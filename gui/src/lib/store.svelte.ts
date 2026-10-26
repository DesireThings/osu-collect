// App-wide state that outlives a screen: the collections the user added and
// the downloads the backend is running. Switching steps in the sidebar keeps
// both.
import {
  cancelDownload,
  listDownloads,
  onDownloadStatus,
  previewCollection,
  startDownload,
  type CollectionPreview,
  type DownloadStatus,
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
});

let nextKey = 0;
let listening = false;

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
