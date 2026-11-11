mod about;
mod pins;
mod runtime;
mod text;

use std::sync::Mutex;

use about::About;
use pins::{PinList, PinStore, PinView, UpsertPin};
use runtime::RuntimeSettings;
use tauri::{Manager, State};

#[tauri::command]
fn list_pins(store: State<'_, Mutex<PinStore>>) -> PinList {
    store.lock().unwrap_or_else(|err| err.into_inner()).list()
}

#[tauri::command]
fn upsert_pin(store: State<'_, Mutex<PinStore>>, pin: UpsertPin) -> Result<PinView, String> {
    store
        .lock()
        .unwrap_or_else(|err| err.into_inner())
        .upsert(pin)
}

#[tauri::command]
fn delete_pin(store: State<'_, Mutex<PinStore>>, id: String) -> Result<(), String> {
    store
        .lock()
        .unwrap_or_else(|err| err.into_inner())
        .delete(&id)
}

#[tauri::command]
fn set_pin_enabled(
    store: State<'_, Mutex<PinStore>>,
    id: String,
    enabled: bool,
) -> Result<PinView, String> {
    store
        .lock()
        .unwrap_or_else(|err| err.into_inner())
        .set_enabled(&id, enabled)
}

#[tauri::command]
fn app_about(app: tauri::AppHandle) -> About {
    about::about(&app.package_info().version.to_string())
}

#[tauri::command]
fn open_project_page(url: String) -> Result<(), String> {
    about::open_project_page(&url)
}

#[tauri::command]
fn runtime_settings(
    app: tauri::AppHandle,
    state: State<'_, Mutex<runtime::RuntimeState>>,
) -> RuntimeSettings {
    runtime::current(&state, &app)
}

#[tauri::command]
fn set_language(
    app: tauri::AppHandle,
    state: State<'_, Mutex<runtime::RuntimeState>>,
    language: String,
) -> Result<RuntimeSettings, String> {
    runtime::set_language(&state, &app, &language)
}

#[tauri::command]
fn set_run_in_background(
    state: State<'_, Mutex<runtime::RuntimeState>>,
    enabled: bool,
) -> Result<RuntimeSettings, String> {
    runtime::set_run_in_background(&state, enabled)
}

#[tauri::command]
fn set_open_at_login(
    app: tauri::AppHandle,
    state: State<'_, Mutex<runtime::RuntimeState>>,
    enabled: bool,
) -> Result<RuntimeSettings, String> {
    runtime::set_open_at_login(&state, &app, enabled)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .setup(|app| {
            let directory = app.path().app_data_dir().expect("the app data directory");
            app.manage(Mutex::new(PinStore::open(directory.join("pins.json"))));
            app.manage(Mutex::new(runtime::RuntimeState::open(
                directory.join("runtime.json"),
            )));
            runtime::install(app.handle())?;
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_title("Namevane by RumenX");
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_pins,
            upsert_pin,
            delete_pin,
            set_pin_enabled,
            app_about,
            open_project_page,
            runtime_settings,
            set_language,
            set_run_in_background,
            set_open_at_login
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| runtime::on_run_event(app, &event));
}
