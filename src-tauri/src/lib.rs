mod about;
mod pins;
mod text;

use std::sync::Mutex;

use about::About;
use pins::{PinList, PinStore, PinView, UpsertPin};
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let directory = app.path().app_data_dir().expect("the app data directory");
            app.manage(Mutex::new(PinStore::open(directory.join("pins.json"))));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_pins,
            upsert_pin,
            delete_pin,
            set_pin_enabled,
            app_about,
            open_project_page
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
