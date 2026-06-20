pub mod audio;
pub mod backend;
pub mod context;
pub mod hotkeys;
pub mod injection;
pub mod keychain;
pub mod overlay;
pub mod settings;
pub mod tray;

use std::sync::OnceLock;
use std::time::Duration;

use crossbeam_channel::{Receiver, Sender};
use ringbuf::traits::{Consumer, Observer};
use speak_up_core::ipc::{BackendMessage, ClientMessage, DictationEntry};
use speak_up_core::AppContext;

type HistoryResp = crossbeam_channel::Sender<Result<(Vec<DictationEntry>, usize), String>>;
type LastDictResp = crossbeam_channel::Sender<Result<Option<DictationEntry>, String>>;

type ModelsResp = crossbeam_channel::Sender<Result<Vec<speak_up_core::ipc::ModelInfo>, String>>;

pub enum BackendRequest {
    QueryHistory {
        limit: usize,
        offset: usize,
        search_term: Option<String>,
        response_tx: HistoryResp,
    },
    QueryLastDictation {
        response_tx: LastDictResp,
    },
    InjectText {
        text: String,
    },
    ListModels {
        response_tx: ModelsResp,
    },
    DownloadModel {
        model_name: String,
    },
}

use crate::context::ContextDetector;
use crate::injection::TextInjector;
use crate::overlay::OverlayState;
use crate::tray::{AppState, TrayCommand};

use hotkeys::HotkeyAction;

static RELOAD_SETTINGS_TX: OnceLock<Sender<()>> = OnceLock::new();
static BACKEND_REQ_TX: OnceLock<Sender<BackendRequest>> = OnceLock::new();

pub fn notify_settings_changed() {
    if let Some(tx) = RELOAD_SETTINGS_TX.get() {
        let _ = tx.send(());
    }
}

pub fn get_backend_request_tx() -> Option<Sender<BackendRequest>> {
    BACKEND_REQ_TX.get().cloned()
}

pub fn setup_tauri(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
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

    let (overlay_tx, overlay_rx) = crossbeam_channel::unbounded::<OverlayState>();
    let (tray_cmd_tx, tray_cmd_rx) = crossbeam_channel::unbounded::<TrayCommand>();
    let (state_tx, state_rx) = crossbeam_channel::unbounded::<AppState>();

    if crate::settings::is_first_run() {
        let handle = app.handle().clone();
        let handle2 = handle.clone();
        // Defer window creation to after event loop starts so IPC bridge is available
        let _ = handle.run_on_main_thread(move || {
            let _ = tauri::WebviewWindowBuilder::new(
                &handle2,
                "wizard",
                tauri::WebviewUrl::App("wizard.html".into()),
            )
            .title("Welcome to Speak Up")
            .inner_size(580.0, 520.0)
            .center()
            .resizable(false)
            .build();
        });
    }

    let overlay_cfg = overlay::OverlayConfig::default();
    std::thread::spawn(move || {
        overlay::run_overlay_loop(overlay_cfg, overlay_rx);
    });

    std::thread::spawn(move || {
        run_main_loop(MainLoopConfig {
            overlay_tx,
            tray_cmd_rx,
            state_tx,
            backend_port: default_port,
        });
    });

    let app_handle = app.handle().clone();
    let tray_ctx = match tray::build_tray(&app_handle, tray_cmd_tx) {
        Ok(t) => Some(t),
        Err(e) => {
            tracing::error!("Failed to build tray: {:?}", e);
            None
        }
    };

    if let Some(tray_ctx) = tray_ctx {
        std::thread::spawn(move || {
            let mut current_state = AppState::Idle;
            while let Ok(new_state) = state_rx.recv() {
                if new_state != current_state {
                    current_state = new_state;
                    tray::update_tray_label(&tray_ctx, current_state);
                }
            }
        });
    }

    Ok(())
}

pub struct MainLoopConfig {
    pub overlay_tx: Sender<OverlayState>,
    pub tray_cmd_rx: Receiver<TrayCommand>,
    pub state_tx: Sender<AppState>,
    pub backend_port: u16,
}

fn register_hotkeys(hotkey_mgr: &mut hotkeys::HotkeyManager, settings: &speak_up_core::Settings) {
    tracing::info!("Registering hotkeys: record='{}', stop='{}', retype='{}'", settings.hotkeys.hold_to_record, settings.hotkeys.toggle_mic, settings.hotkeys.retype_last);
    hotkey_mgr.unregister_all();
    if let Err(e) = hotkey_mgr.register(&settings.hotkeys.hold_to_record, HotkeyAction::ToggleRecording) {
        tracing::warn!("Failed to register record hotkey '{}': {:?}", settings.hotkeys.hold_to_record, e);
    }
    if let Err(e) = hotkey_mgr.register(&settings.hotkeys.toggle_mic, HotkeyAction::StopRecording) {
        tracing::warn!("Failed to register stop hotkey '{}': {:?}", settings.hotkeys.toggle_mic, e);
    }
    if let Err(e) = hotkey_mgr.register(&settings.hotkeys.retype_last, HotkeyAction::RetypeLast) {
        tracing::warn!("Failed to register retype hotkey '{}': {:?}", settings.hotkeys.retype_last, e);
    }
}

fn run_main_loop(cfg: MainLoopConfig) {
    let backend = match backend::BackendClient::spawn_and_connect(cfg.backend_port) {
        Ok(b) => b,
        Err(e) => {
            tracing::error!("Backend connection failed: {}", e);
            return;
        }
    };

    let mut hotkey_mgr = match hotkeys::HotkeyManager::new() {
        Ok(h) => h,
        Err(e) => {
            tracing::error!("Hotkey init failed: {:?}", e);
            return;
        }
    };
    register_hotkeys(&mut hotkey_mgr, &crate::settings::load_settings_from_disk());

    let mut context_detector = context::DefaultContextDetector::new();
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
    let mut last_cleaned_text: Option<String> = None;

    let _state_tx = cfg.state_tx;

    let (reload_tx, reload_rx) = crossbeam_channel::unbounded::<()>();
    let _ = RELOAD_SETTINGS_TX.set(reload_tx);

    let (backend_req_tx, backend_req_rx) = crossbeam_channel::unbounded::<BackendRequest>();
    let _ = BACKEND_REQ_TX.set(backend_req_tx);

    let mut pending_history_resp: Option<HistoryResp> = None;
    let mut pending_last_dict_resp: Option<LastDictResp> = None;
    let mut pending_models_resp: Option<ModelsResp> = None;

    let mut was_muted: Option<bool> = None;

    loop {
        if let Some(action) = hotkey_mgr.poll_event() {
            match action {
                HotkeyAction::ToggleRecording | HotkeyAction::StartRecording => {
                    if !recording {
                        tracing::info!("Starting recording");
                        recording = true;
                        let _ = _state_tx.send(AppState::Recording);

                        if let Err(e) = audio.start("") {
                            tracing::error!("Audio start failed: {}", e);
                            recording = false;
                            continue;
                        }

                        let settings = crate::settings::load_settings_from_disk();
                        if settings.general.auto_mute {
                            was_muted = crate::audio::get_sink_mute_state().ok();
                            if was_muted != Some(true) {
                                if let Err(e) = crate::audio::set_system_audio_mute(true) {
                                    tracing::warn!("Auto-mute failed: {}", e);
                                }
                            }
                        }

                        update_overlay_fn(
                            &cfg.overlay_tx,
                            &mut overlay_state,
                            OverlayState {
                                is_visible: true,
                                is_recording: true,
                                audio_level: 0.0,
                                transcript: String::new(),
                                is_processing: false,
                            },
                        );

                        context_detector.poll();
                        let msg = ClientMessage::StartSession {
                            app_context: context_detector.last_context().unwrap_or_else(|| AppContext {
                                window_title: String::new(),
                                executable_name: String::new(),
                                window_class: String::new(),
                                profile_name: None,
                            }),
                        };
                        if let Ok(data) = bincode::serialize(&msg) {
                            let _ = backend.to_backend.send(data);
                        }
                    } else {
                        do_stop_recording(
                            &mut audio,
                            &backend.to_backend,
                            current_session_id.take(),
                            &cfg.overlay_tx,
                            &mut overlay_state,
                            &mut was_muted,
                        );
                        let _ = _state_tx.send(AppState::Processing);
                        recording = false;
                    }
                }
                HotkeyAction::StopRecording => {
                    if recording {
                        do_stop_recording(
                            &mut audio,
                            &backend.to_backend,
                            current_session_id.take(),
                            &cfg.overlay_tx,
                            &mut overlay_state,
                            &mut was_muted,
                        );
                        let _ = _state_tx.send(AppState::Processing);
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

        if let Ok(cmd) = cfg.tray_cmd_rx.try_recv() {
            match cmd {
                TrayCommand::ToggleRecording => {
                    if !recording {
                        tracing::info!("Starting recording (tray)");
                        recording = true;
                        let _ = _state_tx.send(AppState::Recording);

                        if let Err(e) = audio.start("") {
                            tracing::error!("Audio start failed: {}", e);
                            recording = false;
                            continue;
                        }

                        let settings = crate::settings::load_settings_from_disk();
                        if settings.general.auto_mute {
                            was_muted = crate::audio::get_sink_mute_state().ok();
                            if was_muted != Some(true) {
                                if let Err(e) = crate::audio::set_system_audio_mute(true) {
                                    tracing::warn!("Auto-mute failed: {}", e);
                                }
                            }
                        }

                        update_overlay_fn(
                            &cfg.overlay_tx,
                            &mut overlay_state,
                            OverlayState {
                                is_visible: true,
                                is_recording: true,
                                audio_level: 0.0,
                                transcript: String::new(),
                                is_processing: false,
                            },
                        );

                        context_detector.poll();
                        let msg = ClientMessage::StartSession {
                            app_context: context_detector.last_context().unwrap_or_else(|| AppContext {
                                window_title: String::new(),
                                executable_name: String::new(),
                                window_class: String::new(),
                                profile_name: None,
                            }),
                        };
                        if let Ok(data) = bincode::serialize(&msg) {
                            let _ = backend.to_backend.send(data);
                        }
                    } else {
                        do_stop_recording(
                            &mut audio,
                            &backend.to_backend,
                            current_session_id.take(),
                            &cfg.overlay_tx,
                            &mut overlay_state,
                            &mut was_muted,
                        );
                        let _ = _state_tx.send(AppState::Processing);
                        recording = false;
                    }
                }
                TrayCommand::StopRecording => {
                    if recording {
                        do_stop_recording(
                            &mut audio,
                            &backend.to_backend,
                            current_session_id.take(),
                            &cfg.overlay_tx,
                            &mut overlay_state,
                            &mut was_muted,
                        );
                        let _ = _state_tx.send(AppState::Processing);
                        recording = false;
                    }
                }
                TrayCommand::RetypeLast => {
                    if let Err(e) = injector.retype_last() {
                        tracing::warn!("Retype failed: {:?}", e);
                    }
                }
                TrayCommand::OpenSettings => {}
                TrayCommand::Quit => {
                    tracing::info!("Quit requested via tray");
                    break;
                }
            }
        }

        if reload_rx.try_recv().is_ok() {
            tracing::info!("Settings changed, reloading on backend and re-registering hotkeys");
            let msg = ClientMessage::ReloadSettings;
            if let Ok(data) = bincode::serialize(&msg) {
                let _ = backend.to_backend.send(data);
            }
            let settings = crate::settings::load_settings_from_disk();
            register_hotkeys(&mut hotkey_mgr, &settings);
        }

        while let Ok(req) = backend_req_rx.try_recv() {
            match req {
                BackendRequest::QueryHistory { limit, offset, search_term, response_tx } => {
                    let msg = ClientMessage::QueryHistory { limit, offset, search_term };
                    if let Ok(data) = bincode::serialize(&msg) {
                        let _ = backend.to_backend.send(data);
                    }
                    pending_history_resp = Some(response_tx);
                }
                BackendRequest::QueryLastDictation { response_tx } => {
                    let msg = ClientMessage::QueryLastDictation;
                    if let Ok(data) = bincode::serialize(&msg) {
                        let _ = backend.to_backend.send(data);
                    }
                    pending_last_dict_resp = Some(response_tx);
                }
                BackendRequest::InjectText { text } => {
                    if let Err(e) = injector.inject_text(&text) {
                        tracing::error!("Inject text from history failed: {:?}", e);
                    }
                }
                BackendRequest::ListModels { response_tx } => {
                    let msg = ClientMessage::ListModels;
                    if let Ok(data) = bincode::serialize(&msg) {
                        let _ = backend.to_backend.send(data);
                    }
                    pending_models_resp = Some(response_tx);
                }
                BackendRequest::DownloadModel { model_name } => {
                    let msg = ClientMessage::DownloadModel { model_name };
                    if let Ok(data) = bincode::serialize(&msg) {
                        let _ = backend.to_backend.send(data);
                    }
                }
            }
        }

        while let Ok(data) = backend.from_backend.try_recv() {
            if let Ok(msg) = bincode::deserialize::<BackendMessage>(&data) {
                handle_backend_message(
                    msg,
                    &mut current_session_id,
                    &mut audio,
                    &backend.audio_tx,
                    &mut injector,
                    &cfg.overlay_tx,
                    &mut overlay_state,
                    &_state_tx,
                    &mut last_cleaned_text,
                    &mut pending_history_resp,
                    &mut pending_last_dict_resp,
                    &mut pending_models_resp,
                );
            }
        }

        context_detector.poll();

        if recording {
            let level = audio.current_level();
            if (level - overlay_state.audio_level).abs() > 0.01 {
                overlay_state.audio_level = level;
                let _ = cfg.overlay_tx.send(overlay_state.clone());
            }
        }

        std::thread::sleep(Duration::from_millis(16));
    }
}

fn do_stop_recording(
    audio: &mut audio::AudioCapture,
    to_backend: &crossbeam_channel::Sender<Vec<u8>>,
    session_id: Option<uuid::Uuid>,
    overlay_tx: &Sender<OverlayState>,
    overlay_state: &mut OverlayState,
    was_muted: &mut Option<bool>,
) {
    audio.stop();

    if let Some(muted) = was_muted.take() {
        if !muted {
            if let Err(e) = crate::audio::set_system_audio_mute(false) {
                tracing::warn!("Auto-unmute failed: {}", e);
            }
        }
    }
    if let Some(sid) = session_id {
        let msg = ClientMessage::EndSession { session_id: sid };
        if let Ok(data) = bincode::serialize(&msg) {
            let _ = to_backend.send(data);
        }
    }
    let state_clone = overlay_state.clone();
    update_overlay_fn(
        overlay_tx,
        overlay_state,
        OverlayState {
            is_visible: true,
            is_recording: false,
            is_processing: true,
            ..state_clone
        },
    );
}

#[allow(clippy::too_many_arguments)]
fn handle_backend_message(
    msg: BackendMessage,
    current_session_id: &mut Option<uuid::Uuid>,
    audio: &mut audio::AudioCapture,
    audio_tx: &crossbeam_channel::Sender<Vec<u8>>,
    injector: &mut injection::DefaultTextInjector,
    overlay_tx: &crossbeam_channel::Sender<OverlayState>,
    overlay_state: &mut OverlayState,
    state_tx: &crossbeam_channel::Sender<AppState>,
    last_cleaned_text: &mut Option<String>,
    pending_history_resp: &mut Option<HistoryResp>,
    pending_last_dict_resp: &mut Option<LastDictResp>,
    pending_models_resp: &mut Option<ModelsResp>,
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
            *last_cleaned_text = Some(cleaned_text.clone());

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

            let _ = state_tx.send(AppState::Idle);

            let s = overlay_state.clone();
            let tx = overlay_tx.clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(1500));
                let _ = tx.send(OverlayState {
                    is_visible: false,
                    ..s
                });
            });
        }
        BackendMessage::ProcessingStatus {
            session_id: _,
            stage: _,
        } => {}
        BackendMessage::Error {
            code: _,
            message,
        } => {
            tracing::error!("Backend error: {}", message);
        }
        BackendMessage::ProviderSwitched { .. } => {}
        BackendMessage::HistoryResult {
            entries,
            total_count,
        } => {
            tracing::debug!("History query: {} entries of {}", entries.len(), total_count);
            if let Some(tx) = pending_history_resp.take() {
                let _ = tx.send(Ok((entries, total_count)));
            }
        }
        BackendMessage::LastDictationResult { entry } => {
            if let Some(ref e) = entry {
                tracing::debug!("Last dictation: {}", e.cleaned_text);
                if last_cleaned_text.is_none() {
                    *last_cleaned_text = Some(e.cleaned_text.clone());
                }
            }
            if let Some(tx) = pending_last_dict_resp.take() {
                let _ = tx.send(Ok(entry));
            }
        }

        BackendMessage::ModelList { models } => {
            tracing::debug!("Received model list ({} models)", models.len());
            if let Some(tx) = pending_models_resp.take() {
                let _ = tx.send(Ok(models));
            }
        }

        BackendMessage::ModelDownloadProgress { model_name, bytes_downloaded, total_bytes } => {
            tracing::info!(
                "Model '{}' download: {}/{} bytes ({:.0}%)",
                model_name,
                bytes_downloaded,
                total_bytes,
                if total_bytes > 0 { bytes_downloaded as f64 / total_bytes as f64 * 100.0 } else { 0.0 }
            );
        }

        BackendMessage::ModelDownloaded { model_name, success, error } => {
            if success {
                tracing::info!("Model '{}' downloaded successfully", model_name);
            } else {
                tracing::error!("Failed to download model '{}': {:?}", model_name, error);
            }
        }
    }
}

fn stream_audio(
    mut consumer: ringbuf::HeapCons<f32>,
    audio_tx: crossbeam_channel::Sender<Vec<u8>>,
    session_id: uuid::Uuid,
    sample_rate: u32,
    channels: u16,
) {
    let chunk_size = (sample_rate as usize / 50).max(160);
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
