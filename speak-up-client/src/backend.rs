use std::process::{Child, Command};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crossbeam_channel::{Receiver, Sender};
use futures_util::{SinkExt, StreamExt};
use tokio::runtime::Runtime;

pub struct BackendClient {
    pub process: Option<Child>,
    pub to_backend: Sender<Vec<u8>>,
    pub audio_tx: Sender<Vec<u8>>,
    pub from_backend: Receiver<Vec<u8>>,
    pub runtime: Arc<Runtime>,
    running: Arc<AtomicBool>,
}

impl BackendClient {
    pub fn spawn_and_connect(port: u16) -> Result<Self, String> {
        let binary = std::env::current_exe()
            .ok()
            .and_then(|p| {
                let dir = p.parent()?;
                let b = dir.join("speak-up-backend");
                if b.exists() { Some(b) } else { None }
            })
            .unwrap_or_else(|| std::path::PathBuf::from("speak-up-backend"));

        let process = match Command::new(&binary).arg(port.to_string()).spawn() {
            Ok(c) => {
                tracing::info!("Backend spawned (pid {})", c.id());
                Some(c)
            }
            Err(e) => {
                tracing::warn!("Could not spawn backend ({}), assume already running", e);
                None
            }
        };

        std::thread::sleep(Duration::from_millis(500));

        let rt = Runtime::new().map_err(|e| format!("tokio runtime: {}", e))?;
        let (to_backend_tx, to_backend_rx) = crossbeam_channel::unbounded::<Vec<u8>>();
        let (audio_tx, audio_cb_rx) = crossbeam_channel::unbounded::<Vec<u8>>();
        let (from_backend_tx, from_backend) = crossbeam_channel::unbounded::<Vec<u8>>();
        let running = Arc::new(AtomicBool::new(true));
        let r = running.clone();

        let url = format!("ws://127.0.0.1:{}", port);
        rt.spawn(async move {
            loop {
                match tokio_tungstenite::connect_async(&url).await {
                    Ok((ws, _)) => {
                        tracing::info!("Connected to backend at {}", url);
                        let (mut write, mut read) = ws.split();
                        let (to_tx, mut to_rx) = tokio::sync::mpsc::unbounded_channel();
                        let (audio_async_tx, mut audio_async_rx) =
                            tokio::sync::mpsc::unbounded_channel();

                        let to_rx_clone = to_backend_rx.clone();
                        std::thread::spawn(move || {
                            while let Ok(msg) = to_rx_clone.recv() {
                                if to_tx.send(msg).is_err() {
                                    break;
                                }
                            }
                        });

                        let audio_rx_clone = audio_cb_rx.clone();
                        std::thread::spawn(move || {
                            while let Ok(msg) = audio_rx_clone.recv() {
                                if audio_async_tx.send(msg).is_err() {
                                    break;
                                }
                            }
                        });

                        'connected: loop {
                            tokio::select! {
                                msg = to_rx.recv() => {
                                    let Some(msg) = msg else { break 'connected; };
                                    if write
                                        .send(tokio_tungstenite::tungstenite::Message::Binary(msg))
                                        .await
                                        .is_err()
                                    {
                                        break 'connected;
                                    }
                                }
                                msg = audio_async_rx.recv() => {
                                    let Some(msg) = msg else { break 'connected; };
                                    if write
                                        .send(tokio_tungstenite::tungstenite::Message::Binary(msg))
                                        .await
                                        .is_err()
                                    {
                                        break 'connected;
                                    }
                                }
                                msg = read.next() => {
                                    match msg {
                                        Some(Ok(m)) if m.is_binary() => {
                                            let _ = from_backend_tx.send(m.into_data());
                                        }
                                        Some(Ok(m)) if m.is_close() => break 'connected,
                                        Some(Err(e)) => {
                                            tracing::error!("WS error: {:?}", e);
                                            break 'connected;
                                        }
                                        None => break 'connected,
                                        _ => {}
                                    }
                                }
                            }
                        }

                        tracing::warn!("WebSocket disconnected, reconnecting...");
                    }
                    Err(e) => {
                        tracing::warn!("WS connect failed: {:?}, retrying...", e);
                        tokio::time::sleep(Duration::from_secs(2)).await;
                    }
                }

                if !r.load(Ordering::Relaxed) {
                    break;
                }
            }
        });

        Ok(Self {
            process,
            to_backend: to_backend_tx,
            audio_tx,
            from_backend,
            runtime: Arc::new(rt),
            running,
        })
    }
}

impl Drop for BackendClient {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(mut child) = self.process.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
