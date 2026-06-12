use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite;

use speak_up_core::ipc::{BackendMessage, ClientMessage};

use crate::session::SessionManager;

pub async fn start_server(port: u16, session_manager: Arc<SessionManager>) {
    let addr = format!("127.0.0.1:{}", port);
    let listener = TcpListener::bind(&addr).await.expect("Failed to bind");

    tracing::info!("Backend WebSocket server listening on ws://{}", addr);

    while let Ok((stream, peer)) = listener.accept().await {
        tracing::debug!("TCP connection from {}", peer);
        let sm = session_manager.clone();
        tokio::spawn(handle_connection(stream, sm));
    }
}

async fn handle_connection(
    stream: tokio::net::TcpStream,
    session_manager: Arc<SessionManager>,
) {
    let ws_stream = match accept_async(stream).await {
        Ok(ws) => {
            tracing::debug!("WebSocket connection established");
            ws
        }
        Err(e) => {
            tracing::error!("WebSocket handshake failed: {}", e);
            return;
        }
    };

    let (mut ws_write, mut ws_read) = ws_stream.split();
    let (msg_tx, mut msg_rx) = mpsc::channel::<Vec<u8>>(256);

    let write_task = tokio::spawn(async move {
        while let Some(data) = msg_rx.recv().await {
            if let Err(e) = ws_write
                .send(tungstenite::Message::Binary(data))
                .await
            {
                tracing::error!("Failed to send message: {:?}", e);
                break;
            }
        }
    });

    while let Some(msg_result) = ws_read.next().await {
        let msg = match msg_result {
            Ok(m) => m,
            Err(e) => {
                tracing::error!("WebSocket read error: {:?}", e);
                break;
            }
        };

        if !msg.is_binary() {
            if msg.is_close() {
                break;
            }
            continue;
        }

        let client_msg: ClientMessage = match bincode::deserialize(&msg.into_data()) {
            Ok(m) => m,
            Err(e) => {
                tracing::error!("Failed to deserialize message: {}", e);
                let err = BackendMessage::Error {
                    code: speak_up_core::ipc::ErrorCode::InvalidConfig,
                    message: format!("Deserialization error: {}", e),
                };
                if let Ok(data) = bincode::serialize(&err) {
                    let _ = msg_tx.send(data).await;
                }
                continue;
            }
        };

        match client_msg {
            ClientMessage::StartSession { app_context } => {
                let session_id = session_manager.create_session(app_context).await;
                tracing::info!("Session started: {}", session_id);
                let response = BackendMessage::SessionStarted { session_id };
                send_message(&msg_tx, &response).await;
            }

            ClientMessage::AudioChunk { session_id, chunk } => {
                let ok = session_manager.append_audio(session_id, chunk).await;
                if !ok {
                    let err = BackendMessage::Error {
                        code: speak_up_core::ipc::ErrorCode::SessionNotFound,
                        message: format!("Session {} not found", session_id),
                    };
                    send_message(&msg_tx, &err).await;
                }
            }

            ClientMessage::EndSession { session_id } => {
                let processing = BackendMessage::ProcessingStatus {
                    session_id,
                    stage: speak_up_core::ipc::ProcessingStage::Transcribing,
                };
                send_message(&msg_tx, &processing).await;

                let tx = msg_tx.clone();
                match session_manager.finalize_session(session_id).await {
                    Some((mut events_rx, task)) => {
                        tokio::spawn(async move {
                            while let Some(event) = events_rx.recv().await {
                                if event.is_final {
                                    let msg = BackendMessage::InterimTranscript {
                                        session_id,
                                        segment: event.segment,
                                    };
                                    if let Ok(data) = bincode::serialize(&msg) {
                                        let _ = tx.send(data).await;
                                    }
                                }
                            }

                            let result = task.await.unwrap_or_else(|_| {
                                tracing::error!("Transcription task failed for {}", session_id);
                                crate::asr::TranscriptResult {
                                    segments: Vec::new(),
                                    full_text: String::new(),
                                }
                            });

                            let transcript = BackendMessage::FinalTranscript {
                                session_id,
                                raw_text: result.full_text.clone(),
                                cleaned_text: result.full_text,
                            };
                            if let Ok(data) = bincode::serialize(&transcript) {
                                let _ = tx.send(data).await;
                            }

                            let done = BackendMessage::ProcessingStatus {
                                session_id,
                                stage: speak_up_core::ipc::ProcessingStage::Done,
                            };
                            if let Ok(data) = bincode::serialize(&done) {
                                let _ = tx.send(data).await;
                            }
                        });
                    }
                    None => {
                        let err = BackendMessage::Error {
                            code: speak_up_core::ipc::ErrorCode::SessionNotFound,
                            message: format!("Session {} not found", session_id),
                        };
                        send_message(&msg_tx, &err).await;
                    }
                }
            }

            ClientMessage::ReconfigureProvider {
                provider_type,
                config: _,
            } => {
                tracing::warn!(
                    "Provider reconfiguration not implemented in MVP: {:?}",
                    provider_type
                );
                let response = BackendMessage::ProviderSwitched {
                    provider_type,
                    success: false,
                    error: Some("Not implemented in MVP".into()),
                };
                send_message(&msg_tx, &response).await;
            }

            ClientMessage::ReloadSettings => {
                tracing::info!("Settings reload requested (no-op in MVP)");
            }

            ClientMessage::QueryHistory { .. } => {
                let response = BackendMessage::HistoryResult {
                    entries: Vec::new(),
                    total_count: 0,
                };
                send_message(&msg_tx, &response).await;
            }

            ClientMessage::QueryLastDictation => {
                let response = BackendMessage::LastDictationResult { entry: None };
                send_message(&msg_tx, &response).await;
            }
        }
    }

    tracing::debug!("WebSocket connection closed");
    drop(msg_tx);
    let _ = write_task.await;
}

async fn send_message(tx: &mpsc::Sender<Vec<u8>>, msg: &BackendMessage) {
    let data = bincode::serialize(msg).expect("Failed to serialize response");
    if let Err(e) = tx.send(data).await {
        tracing::error!("Failed to enqueue message: {:?}", e);
    }
}
