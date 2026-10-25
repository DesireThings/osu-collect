// Typed wrappers over the Tauri commands in src-tauri/src/lib.rs. Errors come
// back as plain sentences, ready to show.
import { invoke } from "@tauri-apps/api/core";

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
