use std::io::Write;
use std::path::{Path, PathBuf};

const MODELS_DIR: &str = "speak-up/models";

fn models_dir() -> PathBuf {
    dirs::data_dir()
        .map(|p| p.join(MODELS_DIR))
        .unwrap_or_else(|| {
            let home = std::env::var("HOME").unwrap_or_else(|_| "~".into());
            PathBuf::from(home).join(".local/share/speak-up/models")
        })
}

pub struct ModelSpec {
    pub name: &'static str,
    pub filename: &'static str,
    pub url: &'static str,
    pub sha256: &'static str,
    pub size_mb: u64,
}

pub static WHISPER_TINY: ModelSpec = ModelSpec {
    name: "whisper-tiny",
    filename: "ggml-tiny.bin",
    url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.bin",
    sha256: "be07e048e1e599ad46341c8d2a135645097a538221678b7acdd1b1919c6e1b21",
    size_mb: 75,
};

pub static WHISPER_SMALL: ModelSpec = ModelSpec {
    name: "whisper-small",
    filename: "ggml-small.bin",
    url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin",
    sha256: "1be3a9b2063867b937e64e2ec7483364a79917e157fa98c5d94b5c1fffea987b",
    size_mb: 150,
};

pub static MODELS: &[&ModelSpec] = &[&WHISPER_TINY, &WHISPER_SMALL];

#[derive(Debug, Clone)]
pub enum DownloadEvent {
    Progress { bytes_downloaded: u64, total_bytes: u64 },
    Completed { path: PathBuf },
    Error { message: String },
}

pub struct ModelDownloader;

impl ModelDownloader {
    pub fn ensure_models_dir() -> PathBuf {
        let dir = models_dir();
        std::fs::create_dir_all(&dir).ok();
        dir
    }

    pub fn model_path(spec: &ModelSpec) -> PathBuf {
        Self::ensure_models_dir().join(spec.filename)
    }

    pub fn is_downloaded(spec: &ModelSpec) -> bool {
        let path = Self::model_path(spec);
        if !path.exists() {
            return false;
        }
        let Ok(meta) = std::fs::metadata(&path) else {
            return false;
        };
        meta.len() > 0
    }

    pub fn verify_sha256(spec: &ModelSpec) -> bool {
        let path = Self::model_path(spec);
        if !path.exists() {
            return false;
        }
        match compute_sha256(&path) {
            Ok(hash) => {
                let expected = spec.sha256.to_lowercase();
                let actual = hash.to_lowercase();
                if actual != expected {
                    tracing::warn!(
                        "SHA256 mismatch for {}: expected {}, got {}",
                        spec.name,
                        expected,
                        actual
                    );
                }
                actual == expected
            }
            Err(e) => {
                tracing::error!("Failed to compute SHA256 for {}: {}", spec.name, e);
                false
            }
        }
    }

    pub async fn download(
        spec: &ModelSpec,
        progress_tx: tokio::sync::mpsc::Sender<DownloadEvent>,
    ) {
        let path = Self::model_path(spec);

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }

        let existing_size = std::fs::metadata(&path).ok().map(|m| m.len()).unwrap_or(0);

        let client = reqwest::Client::new();
        let request = client.get(spec.url);

        let request = if existing_size > 0 {
            tracing::info!(
                "Resuming download of {} from byte {}",
                spec.name,
                existing_size
            );
            request.header("Range", format!("bytes={}-", existing_size))
        } else {
            request
        };

        let response = match request.send().await {
            Ok(r) => r,
            Err(e) => {
                let _ = progress_tx
                    .send(DownloadEvent::Error {
                        message: format!("HTTP request failed: {}", e),
                    })
                    .await;
                return;
            }
        };

        let status = response.status();
        let total_size = response.content_length().unwrap_or(0) + existing_size;

        if status == reqwest::StatusCode::NOT_FOUND {
            let _ = progress_tx
                .send(DownloadEvent::Error {
                    message: format!("Model file not found at {}", spec.url),
                })
                .await;
            return;
        }

        let mut file = match std::fs::OpenOptions::new()
            .create(true)
            .append(existing_size > 0)
            .write(true)
            .open(&path)
        {
            Ok(f) => f,
            Err(e) => {
                let _ = progress_tx
                    .send(DownloadEvent::Error {
                        message: format!("Failed to open file: {}", e),
                    })
                    .await;
                return;
            }
        };

        let mut downloaded = existing_size;
        let mut stream = response.bytes_stream();

        use futures_util::StreamExt;
        while let Some(chunk_result) = stream.next().await {
            let bytes = match chunk_result {
                Ok(b) => b,
                Err(e) => {
                    let _ = progress_tx
                        .send(DownloadEvent::Error {
                            message: format!("Download stream error: {}", e),
                        })
                        .await;
                    return;
                }
            };

            if let Err(e) = file.write_all(&bytes) {
                let _ = progress_tx
                    .send(DownloadEvent::Error {
                        message: format!("File write error: {}", e),
                    })
                    .await;
                return;
            }

            downloaded += bytes.len() as u64;

            let _ = progress_tx
                .send(DownloadEvent::Progress {
                    bytes_downloaded: downloaded,
                    total_bytes: total_size,
                })
                .await;
        }

        if let Err(e) = file.sync_all() {
            tracing::warn!("Failed to sync file: {}", e);
        }

        if Self::verify_sha256(spec) {
            tracing::info!("SHA256 verified for {}", spec.name);
            let _ = progress_tx
                .send(DownloadEvent::Completed { path })
                .await;
        } else {
            tracing::error!("SHA256 verification failed for {}", spec.name);
            let _ = progress_tx
                .send(DownloadEvent::Error {
                    message: format!("SHA256 verification failed for {}", spec.name),
                })
                .await;
        }
    }
}

fn compute_sha256(path: &Path) -> Result<String, Box<dyn std::error::Error>> {
    use sha2::Digest;
    let data = std::fs::read(path)?;
    let hash = sha2::Sha256::digest(&data);
    Ok(format!("{:x}", hash))
}
