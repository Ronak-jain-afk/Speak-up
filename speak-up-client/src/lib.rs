pub mod audio;
pub mod backend;
pub mod context;
pub mod hotkeys;
pub mod injection;
pub mod overlay;
pub mod settings;

use std::time::Duration;

use crossbeam_channel::Sender;
use ringbuf::traits::{Consumer, Observer};
use speak_up_core::ipc::{BackendMessage, ClientMessage};
use speak_up_core::AppContext;

use crate::injection::TextInjector;

use hotkeys::HotkeyAction;
use overlay::OverlayState;

pub fn run() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .try_init();

    tracing::info!("Speak Up client v{} starting", env!("CARGO_PKG_VERSION"));

    let default_port: u16 = std::env::var("SPEAK_UP_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(9876);

    // Overlay state channel
    let (overlay_tx, overlay_rx) = crossbeam_channel::unbounded::<OverlayState>();

    // Spawn overlay thread
    let overlay_cfg = overlay::OverlayConfig::default();
    std::thread::spawn(move || {
        overlay::run_overlay_loop(overlay_cfg, overlay_rx);
    });

    // Connect to backend
    let backend = match backend::BackendClient::spawn_and_connect(default_port) {
        Ok(b) => b,
        Err(e) => {
            tracing::error!("Backend connection failed: {}", e);
            return;
        }
    };

    // Register hotkeys
    let mut hotkey_mgr = match hotkeys::HotkeyManager::new() {
        Ok(h) => h,
        Err(e) => {
            tracing::error!("Hotkey init failed: {:?}", e);
            return;
        }
    };
    if let Err(e) = hotkey_mgr.register("Ctrl+Shift+Space", HotkeyAction::ToggleRecording) {
        tracing::warn!("Failed to register toggle hotkey: {:?}", e);
    }
    if let Err(e) = hotkey_mgr.register("Ctrl+Shift+M", HotkeyAction::StopRecording) {
        tracing::warn!("Failed to register stop hotkey: {:?}", e);
    }
    if let Err(e) = hotkey_mgr.register("Ctrl+Shift+V", HotkeyAction::RetypeLast) {
        tracing::warn!("Failed to register retype hotkey: {:?}", e);
    }

    let mut audio = audio::AudioCapture::new();
    let mut injector = match injection::DefaultTextInjector::new() {
        Ok(i) => i,
        Err(e) => {
            tracing::error!("Text injector init failed: {:?}", e);
            return;
        }
    };

    let mut recording = false;
    let mut current_session_id = None;
    let mut overlay_state = OverlayState::default();

    fn update_overlay(
        tx: &Sender<OverlayState>,
        state: &mut OverlayState,
        new: OverlayState,
    ) {
        *state = new.clone();
        let _ = tx.send(new);
    }

    loop {
        // --- Hotkey events ---
        if let Some(action) = hotkey_mgr.poll_event() {
            match action {
                HotkeyAction::ToggleRecording | HotkeyAction::StartRecording => {
                    if !recording {
                        tracing::info!("Starting recording");
                        recording = true;

                        if let Err(e) = audio.start("") {
                            tracing::error!("Audio start failed: {}", e);
                            recording = false;
                            continue;
                        }

                        update_overlay(
                            &overlay_tx,
                            &mut overlay_state,
                            OverlayState {
                                is_visible: true,
                                is_recording: true,
                                audio_level: 0.0,
                                transcript: String::new(),
                                is_processing: false,
                            },
                        );

                        let msg = ClientMessage::StartSession {
                            app_context: AppContext {
                                window_title: String::new(),
                                executable_name: String::new(),
                                window_class: String::new(),
                                profile_name: None,
                            },
                        };
                        if let Ok(data) = bincode::serialize(&msg) {
                            let _ = backend.to_backend.send(data);
                        }
                    }
                }
                HotkeyAction::StopRecording => {
                    if recording {
                        stop_recording(
                            &mut audio,
                            &backend.to_backend,
                            current_session_id.take(),
                        );
                        let state_clone = overlay_state.clone();
                        update_overlay(
                            &overlay_tx,
                            &mut overlay_state,
                            OverlayState {
                                is_visible: true,
                                is_recording: false,
                                is_processing: true,
                                ..state_clone
                            },
                        );
                        recording = false;
                    }
                }
                HotkeyAction::RetypeLast => {
                    if let Err(e) = injector.retype_last() {
                        tracing::warn!("Retype failed: {:?}", e);
                    }
                }
            }
        }

        // --- Backend messages ---
        while let Ok(data) = backend.from_backend.try_recv() {
            if let Ok(msg) = bincode::deserialize::<BackendMessage>(&data) {
                handle_backend_message(
                    msg,
                    &mut current_session_id,
                    &mut audio,
                    &backend.audio_tx,
                    &mut injector,
                    &overlay_tx,
                    &mut overlay_state,
                );
            }
        }

        // --- Update audio level during recording ---
        if recording {
            let level = audio.current_level();
            if (level - overlay_state.audio_level).abs() > 0.01 {
                overlay_state.audio_level = level;
                let _ = overlay_tx.send(overlay_state.clone());
            }
        }

        std::thread::sleep(Duration::from_millis(16));
    }
}

fn stop_recording(
    audio: &mut audio::AudioCapture,
    to_backend: &crossbeam_channel::Sender<Vec<u8>>,
    session_id: Option<uuid::Uuid>,
) {
    audio.stop();
    if let Some(sid) = session_id {
        let msg = ClientMessage::EndSession { session_id: sid };
        if let Ok(data) = bincode::serialize(&msg) {
            let _ = to_backend.send(data);
        }
    }
}

fn handle_backend_message(
    msg: BackendMessage,
    current_session_id: &mut Option<uuid::Uuid>,
    audio: &mut audio::AudioCapture,
    audio_tx: &crossbeam_channel::Sender<Vec<u8>>,
    injector: &mut injection::DefaultTextInjector,
    overlay_tx: &crossbeam_channel::Sender<OverlayState>,
    overlay_state: &mut OverlayState,
) {
    match msg {
        BackendMessage::SessionStarted { session_id } => {
            tracing::info!("Session started: {}", session_id);
            *current_session_id = Some(session_id);

            if let Some(consumer) = audio.take_consumer() {
                let audio_tx = audio_tx.clone();
                let sid = session_id;
                let sample_rate = audio.sample_rate();
                let channels = audio.channels();
                std::thread::spawn(move || {
                    stream_audio(consumer, audio_tx, sid, sample_rate, channels);
                });
            }
        }
        BackendMessage::InterimTranscript { session_id: _, segment } => {
            update_overlay_fn(
                overlay_tx,
                overlay_state,
                OverlayState {
                    is_visible: true,
                    is_recording: true,
                    transcript: segment.text,
                    audio_level: overlay_state.audio_level,
                    is_processing: false,
                },
            );
        }
        BackendMessage::FinalTranscript {
            session_id: _,
            raw_text: _,
            cleaned_text,
        } => {
            tracing::info!("Final transcript: {}", cleaned_text);

            update_overlay_fn(
                overlay_tx,
                overlay_state,
                OverlayState {
                    is_visible: true,
                    is_recording: false,
                    is_processing: false,
                    transcript: cleaned_text.clone(),
                    audio_level: 0.0,
                },
            );

            injector.save_clipboard();
            if let Err(e) = injector.inject_text(&cleaned_text) {
                tracing::error!("Text injection failed: {:?}", e);
            }
            injector.restore_clipboard();

            let state = overlay_state.clone();
            let overlay_tx_clone = overlay_tx.clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(1500));
                let _ = overlay_tx_clone.send(OverlayState {
                    is_visible: false,
                    ..state
                });
            });
        }
        BackendMessage::ProcessingStatus {
            session_id: _,
            stage: _,
        } => {
            // Status updates during processing
        }
        BackendMessage::Error {
            code: _,
            message,
        } => {
            tracing::error!("Backend error: {}", message);
        }
        BackendMessage::ProviderSwitched { .. }
        | BackendMessage::HistoryResult { .. }
        | BackendMessage::LastDictationResult { .. } => {}
    }
}

fn stream_audio(
    mut consumer: ringbuf::HeapCons<f32>,
    audio_tx: crossbeam_channel::Sender<Vec<u8>>,
    session_id: uuid::Uuid,
    sample_rate: u32,
    channels: u16,
) {
    let chunk_size = (sample_rate as usize / 50).max(160); // 20ms chunks
    let mut buf = Vec::with_capacity(chunk_size);

    loop {
        while let Some(sample) = consumer.try_pop() {
            buf.push(sample);
            if buf.len() >= chunk_size {
                let chunk = buf.drain(..chunk_size).collect::<Vec<_>>();
                let i16_samples: Vec<i16> = chunk
                    .iter()
                    .map(|&s| (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)
                    .collect();

                let audio_chunk = speak_up_core::AudioChunk {
                    data: i16_samples
                        .iter()
                        .flat_map(|&s| s.to_le_bytes())
                        .collect(),
                    sample_rate,
                    channels,
                };

                let msg = ClientMessage::AudioChunk {
                    session_id,
                    chunk: audio_chunk,
                };
                if let Ok(data) = bincode::serialize(&msg) {
                    if audio_tx.send(data).is_err() {
                        break;
                    }
                }
            }
        }

        if buf.is_empty() && consumer.is_empty() {
            // Check if the session ended (consumer will be dropped by AudioCapture::stop)
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}

fn update_overlay_fn(
    tx: &crossbeam_channel::Sender<OverlayState>,
    state: &mut OverlayState,
    new: OverlayState,
) {
    *state = new.clone();
    let _ = tx.send(new);
}
