use crossbeam_channel::Sender;
use tauri::{
    menu::{MenuBuilder, MenuItemBuilder, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, Runtime,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayCommand {
    ToggleRecording,
    StopRecording,
    RetypeLast,
    OpenSettings,
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppState {
    Idle,
    Recording,
    Processing,
}

pub struct TrayContext<R: Runtime> {
    pub _tray: TrayIcon<R>,
    pub record_item: MenuItem<R>,
}

pub fn build_tray<R: Runtime>(
    app: &AppHandle<R>,
    cmd_tx: Sender<TrayCommand>,
) -> tauri::Result<TrayContext<R>> {
    let record = MenuItemBuilder::with_id("record", "Start Recording").build(app)?;
    let settings = MenuItemBuilder::with_id("settings", "Settings...").build(app)?;
    let retype = MenuItemBuilder::with_id("retype", "Re-type Last").build(app)?;
    let quit = MenuItemBuilder::with_id("quit", "Quit").build(app)?;

    let menu = MenuBuilder::new(app)
        .item(&record)
        .item(&settings)
        .item(&retype)
        .separator()
        .item(&quit)
        .build()?;

    let tray = TrayIconBuilder::new()
        .menu(&menu)
        .tooltip("Speak Up")
        .on_menu_event(move |app, event| {
            match event.id.as_ref() {
                "record" => {
                    let _ = cmd_tx.send(TrayCommand::ToggleRecording);
                }
                "settings" => {
                    open_settings_window(app);
                }
                "retype" => {
                    let _ = cmd_tx.send(TrayCommand::RetypeLast);
                }
                "quit" => {
                    app.exit(0);
                }
                _ => {}
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("settings") {
                    let _ = window.set_focus();
                }
            }
        })
        .build(app)?;

    Ok(TrayContext { _tray: tray, record_item: record })
}

pub fn update_tray_label<R: Runtime>(ctx: &TrayContext<R>, state: AppState) {
    let label = match state {
        AppState::Idle => "Start Recording",
        AppState::Recording => "Stop Recording",
        AppState::Processing => "Processing...",
    };
    let _ = ctx.record_item.set_text(label);
}

fn open_settings_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.set_focus();
        return;
    }
    let _ = tauri::WebviewWindowBuilder::new(
        app,
        "settings",
        tauri::WebviewUrl::App("index.html".into()),
    )
    .title("Speak Up \u{2014} Settings")
    .inner_size(720.0, 640.0)
    .center()
    .resizable(true)
    .build();
}
