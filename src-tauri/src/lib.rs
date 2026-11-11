mod about;
mod pins;
mod runtime;
mod text;

use std::sync::Mutex;

use about::About;
use pins::{PinList, PinStore, PinView, UpsertPin};
use runtime::RuntimeSettings;
use tauri::{Manager, State};
use tauri_plugin_dialog::DialogExt;

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
fn export_pins(app: tauri::AppHandle, store: State<'_, Mutex<PinStore>>) -> Result<bool, String> {
    let text = store
        .lock()
        .unwrap_or_else(|err| err.into_inner())
        .export_text()?;
    let chosen = app
        .dialog()
        .file()
        .add_filter("JSON", &["json"])
        .set_file_name("namevane-addresses.json")
        .set_title("Export addresses")
        .blocking_save_file();
    let Some(chosen) = chosen else {
        return Ok(false);
    };
    let path = chosen
        .into_path()
        .map_err(|_| "error.location".to_string())?;
    std::fs::write(path, text).map_err(|_| "error.listSave".to_string())?;
    Ok(true)
}

#[tauri::command]
fn import_pins(
    app: tauri::AppHandle,
    store: State<'_, Mutex<PinStore>>,
) -> Result<pins::ImportReport, String> {
    let chosen = app
        .dialog()
        .file()
        .add_filter("JSON", &["json"])
        .set_title("Import addresses")
        .blocking_pick_file();
    let Some(chosen) = chosen else {
        return Ok(pins::ImportReport {
            added: 0,
            skipped: 0,
        });
    };
    let path = chosen
        .into_path()
        .map_err(|_| "error.fileRead".to_string())?;
    let text =
        std::fs::read_to_string(path).map_err(|_| "error.fileRead".to_string())?;
    store
        .lock()
        .unwrap_or_else(|err| err.into_inner())
        .import_text(&text)
}

#[tauri::command]
fn install_update(app: tauri::AppHandle) -> Result<(), String> {
    let version = app.package_info().version.to_string();
    about::install_update(&version)?;
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(400));
        runtime::request_quit(&app);
    });
    Ok(())
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
        .plugin(tauri_plugin_dialog::init())
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
            set_open_at_login,
            export_pins,
            import_pins,
            install_update
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| runtime::on_run_event(app, &event));
}
