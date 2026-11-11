use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, RunEvent, State, WindowEvent};
use tauri_plugin_autostart::ManagerExt;

const VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeSettings {
    pub run_in_background: bool,
    pub open_at_login: bool,
    pub language: String,
}

#[derive(Debug)]
pub struct RuntimeState {
    path: PathBuf,
    pub run_in_background: bool,
    pub open_at_login: bool,
    pub language: String,
    pub quitting: bool,
}

impl RuntimeState {
    pub fn open(path: PathBuf) -> Self {
        let stored = load_file(&path).unwrap_or_else(|err| {
            eprintln!("read runtime settings: {err}");
            StoredSettings::default()
        });
        Self {
            path,
            run_in_background: stored.run_in_background,
            open_at_login: stored.open_at_login,
            language: normalize_language(&stored.language),
            quitting: false,
        }
    }

    pub fn view(&self) -> RuntimeSettings {
        RuntimeSettings {
            run_in_background: self.run_in_background,
            open_at_login: self.open_at_login,
            language: self.language.clone(),
        }
    }

    fn save(&self) -> Result<(), String> {
        save_file(
            &self.path,
            &StoredSettings {
                version: VERSION,
                run_in_background: self.run_in_background,
                open_at_login: self.open_at_login,
                language: self.language.clone(),
            },
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct StoredSettings {
    version: u32,
    run_in_background: bool,
    open_at_login: bool,
    #[serde(default = "english")]
    language: String,
}

fn english() -> String {
    "en".to_string()
}

pub fn normalize_language(value: &str) -> String {
    if value == "bg" { "bg" } else { "en" }.to_string()
}

impl Default for StoredSettings {
    fn default() -> Self {
        Self {
            version: VERSION,
            run_in_background: true,
            open_at_login: false,
            language: english(),
        }
    }
}

pub fn current(state: &State<'_, Mutex<RuntimeState>>, app: &AppHandle) -> RuntimeSettings {
    let mut settings = state.lock().unwrap_or_else(|err| err.into_inner()).view();
    if let Ok(enabled) = app.autolaunch().is_enabled() {
        settings.open_at_login = enabled;
    }
    settings
}

pub fn set_run_in_background(
    state: &State<'_, Mutex<RuntimeState>>,
    enabled: bool,
) -> Result<RuntimeSettings, String> {
    let mut state = state.lock().unwrap_or_else(|err| err.into_inner());
    state.run_in_background = enabled;
    state.save()?;
    Ok(state.view())
}

pub fn set_open_at_login(
    state: &State<'_, Mutex<RuntimeState>>,
    app: &AppHandle,
    enabled: bool,
) -> Result<RuntimeSettings, String> {
    let launch = app.autolaunch();
    if enabled {
        launch.enable().map_err(|err| {
            eprintln!("open at login: {err}");
            "error.loginOn"
                .to_string()
        })?;
    } else {
        launch.disable().map_err(|err| {
            eprintln!("open at login: {err}");
            "error.loginOff".to_string()
        })?;
    }
    let mut state = state.lock().unwrap_or_else(|err| err.into_inner());
    state.open_at_login = enabled;
    state.save()?;
    Ok(state.view())
}

pub fn set_language(
    state: &State<'_, Mutex<RuntimeState>>,
    app: &AppHandle,
    language: &str,
) -> Result<RuntimeSettings, String> {
    let language = normalize_language(language);
    {
        let mut state = state.lock().unwrap_or_else(|err| err.into_inner());
        state.language = language.clone();
        state.save()?;
    }
    apply_tray_language(app, &language);
    Ok(current(state, app))
}

pub fn install(app: &AppHandle) -> Result<(), String> {
    let language = app
        .state::<Mutex<RuntimeState>>()
        .lock()
        .map(|state| state.language.clone())
        .unwrap_or_else(|_| english());
    let open = MenuItem::with_id(app, "open", crate::text::get(&language, "tray.open"), true, None::<&str>)
        .map_err(|_| "error.tray".to_string())?;
    let quit = MenuItem::with_id(app, "quit", crate::text::get(&language, "tray.quit"), true, None::<&str>)
        .map_err(|_| "error.tray".to_string())?;
    app.manage(ShellMenu {
        open: open.clone(),
        quit: quit.clone(),
    });
    let menu = Menu::with_items(app, &[&open, &quit])
        .map_err(|_| "error.tray".to_string())?;
    let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/128x128.png"))
        .map_err(|_| "error.tray".to_string())?;
    let _tray = TrayIconBuilder::with_id("namevane")
        .icon(icon)
        .tooltip("Namevane")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => show_main(app),
            "quit" => quit_app(app),
            _ => {}
        })
        .build(app)
        .map_err(|_| "error.tray".to_string())?;

    let handle = app.clone();
    if let Some(window) = app.get_webview_window("main") {
        window.on_window_event(move |event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let stay = handle
                    .state::<Mutex<RuntimeState>>()
                    .lock()
                    .map(|state| state.run_in_background && !state.quitting)
                    .unwrap_or(false);
                if stay {
                    api.prevent_close();
                    if let Some(window) = handle.get_webview_window("main") {
                        let _ = window.hide();
                    }
                } else {
                    quit_app(&handle);
                }
            }
        });
    }
    Ok(())
}

pub fn on_run_event(app: &AppHandle, event: &RunEvent) {
    match event {
        RunEvent::ExitRequested { api, .. } => {
            let stay = app
                .state::<Mutex<RuntimeState>>()
                .lock()
                .map(|state| state.run_in_background && !state.quitting)
                .unwrap_or(false);
            if stay {
                api.prevent_exit();
            }
        }
        #[cfg(target_os = "macos")]
        RunEvent::Reopen {
            has_visible_windows,
            ..
        } if !has_visible_windows => show_main(app),
        _ => {}
    }
}

fn apply_tray_language(app: &AppHandle, language: &str) {
    let Some(menu) = app.try_state::<ShellMenu>() else {
        return;
    };
    let _ = menu.open.set_text(crate::text::get(language, "tray.open"));
    let _ = menu.quit.set_text(crate::text::get(language, "tray.quit"));
}

struct ShellMenu {
    open: MenuItem<tauri::Wry>,
    quit: MenuItem<tauri::Wry>,
}

fn show_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

pub fn request_quit(app: &AppHandle) {
    quit_app(app);
}

fn quit_app(app: &AppHandle) {
    if let Ok(mut state) = app.state::<Mutex<RuntimeState>>().lock() {
        state.quitting = true;
    }
    app.exit(0);
}

fn load_file(path: &Path) -> Result<StoredSettings, String> {
    if !path.exists() {
        return Ok(StoredSettings::default());
    }
    let text = fs::read_to_string(path).map_err(|err| err.to_string())?;
    let settings: StoredSettings = serde_json::from_str(&text).map_err(|err| err.to_string())?;
    if settings.version != VERSION {
        return Err("newer settings".to_string());
    }
    Ok(settings)
}

fn save_file(path: &Path, settings: &StoredSettings) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .map_err(|_| "error.settingSave".to_string())?;
        }
    }
    let mut bytes = serde_json::to_vec_pretty(settings)
        .map_err(|_| "error.settingSave".to_string())?;
    bytes.push(b'\n');
    let temporary = {
        let mut name = path.as_os_str().to_owned();
        name.push(".tmp");
        PathBuf::from(name)
    };
    fs::write(&temporary, bytes).map_err(|_| "error.settingSave".to_string())?;
    fs::rename(&temporary, path).map_err(|_| "error.settingSave".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_file_keeps_namevane_running_and_does_not_open_at_login() {
        let path = std::env::temp_dir().join(format!("namevane-runtime-{}", uuid::Uuid::new_v4()));
        let state = RuntimeState::open(path);
        assert!(state.run_in_background);
        assert!(!state.open_at_login);
    }

    #[test]
    fn the_background_choice_survives_a_restart() {
        let dir = std::env::temp_dir().join(format!("namevane-runtime-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("runtime.json");
        let mut state = RuntimeState::open(path.clone());
        state.run_in_background = false;
        state.save().unwrap();
        let loaded = RuntimeState::open(path);
        assert!(!loaded.run_in_background);
        assert!(!loaded.open_at_login);
    }
}
