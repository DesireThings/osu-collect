// Typed wrappers over the Tauri commands in src-tauri/src/lib.rs. Errors come
// back as plain sentences, ready to show.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type OsuClient = "lazer" | "stable";

export interface Installs {
  lazer: string | null;
  stable: string | null;
}

export interface Settings {
  client: OsuClient;
  osu_path: string | null;
  download_dir: string;
}

export interface CollectionPreview {
  id: number;
  name: string;
  uploader: string;
  beatmapsets: number;
  beatmaps: number;
}

export interface ImportFolderReport {
  runs: number;
  unpacked: number;
  already_unpacked: number;
  failed: [string, string][];
  collections: number;
  beatmap_hashes: number;
}

export interface LazerImportResult {
  folder: string;
  report: ImportFolderReport;
}

export const detectInstalls = () => invoke<Installs>("detect_installs");
export const loadSettings = () => invoke<Settings>("load_settings");
export const saveSettings = (settings: Settings) =>
  invoke<void>("save_settings", { settings });
export const previewCollection = (input: string) =>
  invoke<CollectionPreview>("preview_collection", { input });
export const buildLazerImport = (downloadDir: string) =>
  invoke<LazerImportResult>("build_lazer_import", { downloadDir });

export type Phase = "queued" | "preparing" | "downloading" | "done" | "failed" | "cancelled";

export interface Failure {
  beatmapset_id: number;
  title: string | null;
  reason: string;
}

export interface DownloadStatus {
  id: number;
  collection_id: number;
  name: string;
  phase: Phase;
  total: number;
  downloaded: number;
  skipped: number;
  failed: number;
  already_in_osu: number;
  bytes_done: number;
  bytes_total: number | null;
  active: number;
  /** Sets waiting because every mirror that has them is rate-limiting. */
  waiting: number;
  /** Seconds until the first waiting set retries, as of this update. */
  retry_in_secs: number | null;
  failures: Failure[];
  error: string | null;
  warning: string | null;
  folder: string | null;
}

export const startDownload = (input: string) => invoke<number>("start_download", { input });
export const cancelDownload = (id: number) => invoke<void>("cancel_download", { id });
export const listDownloads = () => invoke<DownloadStatus[]>("list_downloads");
export const onDownloadStatus = (handler: (status: DownloadStatus) => void) =>
  listen<DownloadStatus>("download-status", (event) => handler(event.payload));
