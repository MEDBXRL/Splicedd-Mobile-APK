use std::fs::{self, File};
use std::io::{Read, Write, BufReader};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use flume::{Sender, Receiver};
use futures::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, Runtime};
use tauri_plugin_notification::NotificationExt;
use tauri_plugin_opener::OpenerExt;
use tokio::sync::Mutex;
use zip::write::FileOptions;

use crate::files::{write_sample_file, file_exists, dir_exists};

/// Progress event for downloads
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgress {
    pub task_id: String,
    pub pack_name: Option<String>,
    pub sample_name: String,
    pub current: usize,
    pub total: usize,
    pub bytes_downloaded: u64,
    pub bytes_total: u64,
    pub status: DownloadStatus,
    pub error: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DownloadStatus {
    Starting,
    Downloading,
    Writing,
    Completed,
    Failed,
    Cancelled,
}

/// Pack download request
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadPackRequest {
    pub pack_uuid: String,
    pub pack_name: String,
    pub sample_uuids: Vec<String>,
    pub base_url: String,
}

/// Single sample download request
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadSampleRequest {
    pub sample_uuid: String,
    pub sample_name: String,
    pub pack_name: Option<String>,
    pub url: String,
    pub pack_uuid: Option<String>,
}

/// Get platform-appropriate download directory
#[tauri::command]
pub async fn get_download_dir<R: Runtime>(app: AppHandle<R>) -> Result<String, String> {
    // Try to get the Downloads directory
    if let Ok(downloads) = app.path().download_dir() {
        return Ok(downloads.to_string_lossy().to_string());
    }

    // Fallback to app data dir
    if let Ok(app_data) = app.path().app_data_dir() {
        let downloads = app_data.join("Downloads");
        if !downloads.exists() {
            fs::create_dir_all(&downloads).map_err(|e| e.to_string())?;
        }
        return Ok(downloads.to_string_lossy().to_string());
    }

    Err("Could not determine download directory".into())
}

/// Download a single sample file
#[tauri::command]
pub async fn download_sample<R: Runtime>(
    app: AppHandle<R>,
    request: DownloadSampleRequest,
) -> Result<String, String> {
    let task_id = uuid::Uuid::new_v4().to_string();

    // Emit starting event
    emit_progress(&app, DownloadProgress {
        task_id: task_id.clone(),
        pack_name: request.pack_name.clone(),
        sample_name: request.sample_name.clone(),
        current: 0,
        total: 1,
        bytes_downloaded: 0,
        bytes_total: 0,
        status: DownloadStatus::Starting,
        error: None,
    });

    let download_dir = get_download_dir(app.clone()).await?;
    let pack_folder = request.pack_name.unwrap_or_else(|| "Splicedd".to_string());
    let safe_pack_name = sanitize_filename(&pack_folder);
    let safe_sample_name = sanitize_filename(&request.sample_name);

    // Ensure extension is .wav
    let sample_name = if safe_sample_name.ends_with(".wav") {
        safe_sample_name
    } else {
        format!("{}.wav", safe_sample_name)
    };

    let pack_dir = Path::new(&download_dir).join(&safe_pack_name);
    fs::create_dir_all(&pack_dir).map_err(|e| e.to_string())?;

    let file_path = pack_dir.join(&sample_name);

    // Check if already exists
    if file_path.exists() {
        emit_progress(&app, DownloadProgress {
            task_id: task_id.clone(),
            pack_name: request.pack_name.clone(),
            sample_name: request.sample_name.clone(),
            current: 1,
            total: 1,
            bytes_downloaded: fs::metadata(&file_path).map(|m| m.len()).unwrap_or(0),
            bytes_total: fs::metadata(&file_path).map(|m| m.len()).unwrap_or(0),
            status: DownloadStatus::Completed,
            error: None,
        });
        return Ok(file_path.to_string_lossy().to_string());
    }

    // Download the file
    let client = Client::new();
    let mut resp = client.get(&request.url)
        .send()
        .await
        .map_err(|e| format!("Failed to start download: {}", e))?;

    let total_size = resp.content_length().unwrap_or(0);
    let mut downloaded = 0u64;
    let mut file = File::create(&file_path).map_err(|e| e.to_string())?;

    emit_progress(&app, DownloadProgress {
        task_id: task_id.clone(),
        pack_name: request.pack_name.clone(),
        sample_name: request.sample_name.clone(),
        current: 0,
        total: 1,
        bytes_downloaded: 0,
        bytes_total: total_size,
        status: DownloadStatus::Downloading,
        error: None,
    });

    while let Some(chunk) = resp.chunk().await.map_err(|e| e.to_string())? {
        file.write_all(&chunk).map_err(|e| e.to_string())?;
        downloaded += chunk.len() as u64;

        emit_progress(&app, DownloadProgress {
            task_id: task_id.clone(),
            pack_name: request.pack_name.clone(),
            sample_name: request.sample_name.clone(),
            current: 0,
            total: 1,
            bytes_downloaded: downloaded,
            bytes_total: total_size,
            status: DownloadStatus::Downloading,
            error: None,
        });
    }

    file.flush().map_err(|e| e.to_string())?;
    drop(file);

    emit_progress(&app, DownloadProgress {
        task_id: task_id.clone(),
        pack_name: request.pack_name.clone(),
        sample_name: request.sample_name.clone(),
        current: 1,
        total: 1,
        bytes_downloaded: downloaded,
        bytes_total: total_size,
        status: DownloadStatus::Completed,
        error: None,
    });

    // Show notification
    if let Err(e) = app.notification().builder()
        .title("Download Complete")
        .body(&format!("Downloaded {}", request.sample_name))
        .show() {
        eprintln!("Failed to show notification: {}", e);
    }

    Ok(file_path.to_string_lossy().to_string())
}

/// Download all samples in a pack
#[tauri::command]
pub async fn download_pack<R: Runtime>(
    app: AppHandle<R>,
    request: DownloadPackRequest,
) -> Result<String, String> {
    let task_id = uuid::Uuid::new_v4().to_string();
    let total_samples = request.sample_uuids.len();
    let download_dir = get_download_dir(app.clone()).await?;
    let safe_pack_name = sanitize_filename(&request.pack_name);
    let pack_dir = Path::new(&download_dir).join(&safe_pack_name);
    fs::create_dir_all(&pack_dir).map_err(|e| e.to_string())?;

    let client = Client::new();
    let mut completed = 0;
    let mut failed = 0;

    emit_progress(&app, DownloadProgress {
        task_id: task_id.clone(),
        pack_name: Some(request.pack_name.clone()),
        sample_name: String::new(),
        current: 0,
        total: total_samples,
        bytes_downloaded: 0,
        bytes_total: 0,
        status: DownloadStatus::Starting,
        error: None,
    });

    for (idx, sample_uuid) in request.sample_uuids.iter().enumerate() {
        // Fetch sample details from Splice API
        let sample_url = format!("{}/assets/{}", request.base_url.trim_end_matches('/'), sample_uuid);

        let sample_resp = match client.get(&sample_url).send().await {
            Ok(r) => r,
            Err(e) => {
                failed += 1;
                emit_progress(&app, DownloadProgress {
                    task_id: task_id.clone(),
                    pack_name: Some(request.pack_name.clone()),
                    sample_name: sample_uuid.clone(),
                    current: idx,
                    total: total_samples,
                    bytes_downloaded: 0,
                    bytes_total: 0,
                    status: DownloadStatus::Failed,
                    error: Some(format!("Failed to fetch sample info: {}", e)),
                });
                continue;
            }
        };

        let sample_json: serde_json::Value = match sample_resp.json().await {
            Ok(j) => j,
            Err(e) => {
                failed += 1;
                emit_progress(&app, DownloadProgress {
                    task_id: task_id.clone(),
                    pack_name: Some(request.pack_name.clone()),
                    sample_name: sample_uuid.clone(),
                    current: idx,
                    total: total_samples,
                    bytes_downloaded: 0,
                    bytes_total: 0,
                    status: DownloadStatus::Failed,
                    error: Some(format!("Failed to parse sample info: {}", e)),
                });
                continue;
            }
        };

        // Extract preview URL and name
        let sample_name = sample_json["data"]["asset"]["name"].as_str().unwrap_or(sample_uuid);
        let preview_url = sample_json["data"]["asset"]["files"]
            .as_array()
            .and_then(|arr| arr.iter().find(|f| f["asset_file_type_slug"] == "preview_mp3"))
            .and_then(|f| f["url"].as_str());

        let Some(preview_url) = preview_url else {
            failed += 1;
            emit_progress(&app, DownloadProgress {
                task_id: task_id.clone(),
                pack_name: Some(request.pack_name.clone()),
                sample_name: sample_name.to_string(),
                current: idx,
                total: total_samples,
                bytes_downloaded: 0,
                bytes_total: 0,
                status: DownloadStatus::Failed,
                error: Some("No preview URL found".into()),
            });
            continue;
        };

        let safe_sample_name = sanitize_filename(sample_name);
        let sample_name_wav = if safe_sample_name.ends_with(".wav") {
            safe_sample_name.clone()
        } else {
            format!("{}.wav", safe_sample_name)
        };
        let file_path = pack_dir.join(&sample_name_wav);

        // Skip if already exists
        if file_path.exists() {
            completed += 1;
            emit_progress(&app, DownloadProgress {
                task_id: task_id.clone(),
                pack_name: Some(request.pack_name.clone()),
                sample_name: sample_name.to_string(),
                current: idx + 1,
                total: total_samples,
                bytes_downloaded: fs::metadata(&file_path).map(|m| m.len()).unwrap_or(0),
                bytes_total: fs::metadata(&file_path).map(|m| m.len()).unwrap_or(0),
                status: DownloadStatus::Completed,
                error: None,
            });
            continue;
        }

        emit_progress(&app, DownloadProgress {
            task_id: task_id.clone(),
            pack_name: Some(request.pack_name.clone()),
            sample_name: sample_name.to_string(),
            current: idx,
            total: total_samples,
            bytes_downloaded: 0,
            bytes_total: 0,
            status: DownloadStatus::Downloading,
            error: None,
        });

        // Download sample
        let mut resp = match client.get(preview_url).send().await {
            Ok(r) => r,
            Err(e) => {
                failed += 1;
                emit_progress(&app, DownloadProgress {
                    task_id: task_id.clone(),
                    pack_name: Some(request.pack_name.clone()),
                    sample_name: sample_name.to_string(),
                    current: idx,
                    total: total_samples,
                    bytes_downloaded: 0,
                    bytes_total: 0,
                    status: DownloadStatus::Failed,
                    error: Some(format!("Download failed: {}", e)),
                });
                continue;
            }
        };

        let total_size = resp.content_length().unwrap_or(0);
        let mut downloaded = 0u64;
        let mut file = match File::create(&file_path) {
            Ok(f) => f,
            Err(e) => {
                failed += 1;
                emit_progress(&app, DownloadProgress {
                    task_id: task_id.clone(),
                    pack_name: Some(request.pack_name.clone()),
                    sample_name: sample_name.to_string(),
                    current: idx,
                    total: total_samples,
                    bytes_downloaded: 0,
                    bytes_total: 0,
                    status: DownloadStatus::Failed,
                    error: Some(format!("File create failed: {}", e)),
                });
                continue;
            }
        };

        while let Some(chunk) = resp.chunk().await.map_err(|e| e.to_string())? {
            file.write_all(&chunk).map_err(|e| e.to_string())?;
            downloaded += chunk.len() as u64;

            emit_progress(&app, DownloadProgress {
                task_id: task_id.clone(),
                pack_name: Some(request.pack_name.clone()),
                sample_name: sample_name.to_string(),
                current: idx,
                total: total_samples,
                bytes_downloaded: downloaded,
                bytes_total: total_size,
                status: DownloadStatus::Downloading,
                error: None,
            });
        }

        file.flush().map_err(|e| e.to_string())?;
        completed += 1;

        emit_progress(&app, DownloadProgress {
            task_id: task_id.clone(),
            pack_name: Some(request.pack_name.clone()),
            sample_name: sample_name.to_string(),
            current: idx + 1,
            total: total_samples,
            bytes_downloaded: downloaded,
            bytes_total: total_size,
            status: DownloadStatus::Completed,
            error: None,
        });

        // Small delay to avoid rate limiting
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    // Final summary notification
    let message = if failed == 0 {
        format!("Pack '{}' downloaded successfully! ({} samples)", request.pack_name, completed)
    } else {
        format!("Pack '{}' completed with {} failed ({} succeeded)", request.pack_name, failed, completed)
    };

    if let Err(e) = app.notification().builder()
        .title("Pack Download Complete")
        .body(&message)
        .show() {
        eprintln!("Failed to show notification: {}", e);
    }

    emit_progress(&app, DownloadProgress {
        task_id: task_id.clone(),
        pack_name: Some(request.pack_name.clone()),
        sample_name: String::new(),
        current: total_samples,
        total: total_samples,
        bytes_downloaded: 0,
        bytes_total: 0,
        status: if failed == 0 { DownloadStatus::Completed } else { DownloadStatus::Failed },
        error: if failed > 0 { Some(format!("{} samples failed", failed)) } else { None },
    });

    Ok(pack_dir.to_string_lossy().to_string())
}

/// Open the download folder in file manager
#[tauri::command]
pub async fn open_download_folder<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    let download_dir = get_download_dir(app.clone()).await?;
    app.opener().open_path(download_dir, None::<&str>).map_err(|e| e.to_string())?;
    Ok(())
}

/// Sanitize filename for filesystem compatibility
fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect::<String>()
        .trim()
        .to_string()
}

/// Emit progress event to frontend
fn emit_progress<R: Runtime>(app: &AppHandle<R>, progress: DownloadProgress) {
    if let Err(e) = app.emit("download-progress", &progress) {
        eprintln!("Failed to emit download progress: {}", e);
    }
}