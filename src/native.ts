import { invoke } from '@tauri-apps/api/core';

/**
 * Whether the app is running inside the Tauri shell. When `false` (e.g. when
 * running `yarn dev` in a plain browser for UI work), native functionality is
 * unavailable and the app falls back to in-memory config and mock search data.
 */
export const IN_TAURI = "__TAURI_INTERNALS__" in window;

/** Whether the app is running on Windows. */
export const IS_WINDOWS = navigator.userAgent.includes("Windows");

/**
 * The longest path (in UTF-16 code units, excluding the null terminator) that the
 * Windows shell accepts. Dragging files with longer paths isn't possible.
 */
export const WINDOWS_MAX_PATH = 259;

/** Whether the app is running on mobile (Android/iOS). */
export const IS_MOBILE = /Android|iPhone|iPad|iPod/i.test(navigator.userAgent);

/**
 * Writes a file to the path retriveved by combining `baseDir` and `relativePath`.
 * The path is required to end with ".wav".
 */
// /src-tauri/src/files.rs
export async function writeSampleFile(baseDir: string, relativePath: string, buffer: Buffer) {
  await invoke("write_sample_file", {
    baseDir,
    relativePath,
    buffer: Array.from(buffer)
  });
}

/**
 * Checks if a file exists on the path retriveved by combining `baseDir` and `relativePath`.
 */
// /src-tauri/src/files.rs
export async function checkFileExists(baseDir: string, relativePath: string) {
  return await invoke<boolean>("file_exists", {
    baseDir,
    relativePath
  });
}

/**
 * Checks if `path` points to an existing directory. Always `true` outside of Tauri.
 */
// /src-tauri/src/files.rs
export async function checkDirExists(path: string) {
  if (!IN_TAURI)
    return true;

  return await invoke<boolean>("dir_exists", { path });
}

/**
 * Creates an empty placeholder file on the path retriveved by combining "baseDir" and "relativePath".
 */
// /src-tauri/src/files.rs
export async function createPlaceholder(baseDir: string, relativePath: string) {
  await invoke("create_placeholder_file", {
    baseDir,
    relativePath
  });
}

/**
 * Gets the platform-appropriate download directory.
 */
// /src-tauri/src/mobile.rs
export async function getDownloadDir(): Promise<string> {
  return await invoke<string>("get_download_dir");
}

/**
 * Downloads a single sample to the device.
 */
// /src-tauri/src/mobile.rs
export interface DownloadSampleRequest {
  sample_uuid: string;
  sample_name: string;
  pack_name?: string;
  url: string;
  pack_uuid?: string;
}

export async function downloadSample(request: DownloadSampleRequest): Promise<string> {
  return await invoke<string>("download_sample", { request });
}

/**
 * Downloads all samples in a pack.
 */
// /src-tauri/src/mobile.rs
export interface DownloadPackRequest {
  pack_uuid: string;
  pack_name: string;
  sample_uuids: string[];
  base_url: string;
}

export async function downloadPack(request: DownloadPackRequest): Promise<string> {
  return await invoke<string>("download_pack", { request });
}

/**
 * Opens the download folder in the system file manager.
 */
// /src-tauri/src/mobile.rs
export async function openDownloadFolder(): Promise<void> {
  return await invoke<void>("open_download_folder");
}

/**
 * Download progress event from backend.
 */
export interface DownloadProgress {
  task_id: string;
  pack_name?: string;
  sample_name: string;
  current: number;
  total: number;
  bytes_downloaded: number;
  bytes_total: number;
  status: 'starting' | 'downloading' | 'writing' | 'completed' | 'failed' | 'cancelled';
  error?: string;
}
