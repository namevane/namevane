mod about;
mod hosts;
mod lists;
mod pins;
mod runtime;
mod text;

pub fn apply_hosts_file(path: &str) -> i32 {
    hosts::apply_privileged(std::path::Path::new(path))
}

use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use about::About;
use lists::{Download, ListStore, SourceList};
use pins::{PinList, PinStore, PinView, UpsertPin};
use runtime::RuntimeSettings;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;

static LISTS_OP: Mutex<()> = Mutex::new(());

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
    let text = std::fs::read_to_string(path).map_err(|_| "error.fileRead".to_string())?;
    store
        .lock()
        .unwrap_or_else(|err| err.into_inner())
        .import_text(&text)
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct HostsStatus {
    applied: bool,
    block_present: bool,
}

fn read_hosts_status(pins: &PinStore, lists: &ListStore) -> Result<HostsStatus, String> {
    let text = hosts::read_hosts(std::path::Path::new("/etc/hosts"))?;
    status_for(&text, &lists.merge_hosts_lines(&pins.hosts_lines()))
}

fn status_for(text: &str, lines: &[String]) -> Result<HostsStatus, String> {
    Ok(HostsStatus {
        applied: hosts::in_sync(text, lines)?,
        block_present: hosts::has_block(text)?,
    })
}

#[tauri::command]
fn hosts_status(
    pins: State<'_, Mutex<PinStore>>,
    lists: State<'_, Mutex<ListStore>>,
) -> Result<HostsStatus, String> {
    let pins = pins.lock().unwrap_or_else(|err| err.into_inner());
    let lists = lists.lock().unwrap_or_else(|err| err.into_inner());
    read_hosts_status(&pins, &lists)
}

#[tauri::command]
fn apply_pins(
    pins: State<'_, Mutex<PinStore>>,
    lists: State<'_, Mutex<ListStore>>,
) -> Result<HostsStatus, String> {
    change_hosts(&pins, &lists, false)
}

#[tauri::command]
fn turn_off_pins(
    pins: State<'_, Mutex<PinStore>>,
    lists: State<'_, Mutex<ListStore>>,
) -> Result<HostsStatus, String> {
    change_hosts(&pins, &lists, true)
}

fn change_hosts(
    pins: &State<'_, Mutex<PinStore>>,
    lists: &State<'_, Mutex<ListStore>>,
    turn_off: bool,
) -> Result<HostsStatus, String> {
    let (lines, directory, current) = {
        let pins = pins.lock().unwrap_or_else(|err| err.into_inner());
        let lists = lists.lock().unwrap_or_else(|err| err.into_inner());
        let current = hosts::read_hosts(std::path::Path::new("/etc/hosts"))?;
        if turn_off && !hosts::has_block(&current)? {
            return read_hosts_status(&pins, &lists);
        }
        let lines = if turn_off {
            Vec::new()
        } else {
            lists.merge_hosts_lines(&pins.hosts_lines())
        };
        if lines.is_empty() && !hosts::has_block(&current)? {
            return status_for(&current, &lines);
        }
        let _ = hosts::render(&current, &lines)?;
        (lines, pins.directory(), current)
    };
    hosts::backup_hosts(&current, &directory)?;
    hosts::prompt_apply(&lines, &directory.join("apply-command.json"))?;
    {
        let mut pins = pins.lock().unwrap_or_else(|err| err.into_inner());
        let lists = lists.lock().unwrap_or_else(|err| err.into_inner());
        pins.mark_applied()?;
        read_hosts_status(&pins, &lists)
    }
}

#[derive(Debug, Clone, Deserialize)]
struct AddSource {
    title: String,
    url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CheckReport {
    changed: u32,
    unchanged: u32,
    failed: u32,
    sources: SourceList,
}

#[derive(Debug, Clone, Serialize)]
struct ListsUpdated {
    changed: u32,
    failed: u32,
    automatic: bool,
}

fn lists_op() -> MutexGuard<'static, ()> {
    LISTS_OP.lock().unwrap_or_else(|err| err.into_inner())
}

fn source_list(
    pins: &State<'_, Mutex<PinStore>>,
    lists: &State<'_, Mutex<ListStore>>,
) -> SourceList {
    let pin_lines = pins
        .lock()
        .unwrap_or_else(|err| err.into_inner())
        .hosts_lines();
    lists
        .lock()
        .unwrap_or_else(|err| err.into_inner())
        .view(&pin_lines)
}

#[tauri::command]
fn list_sources(
    pins: State<'_, Mutex<PinStore>>,
    lists: State<'_, Mutex<ListStore>>,
) -> SourceList {
    source_list(&pins, &lists)
}

#[tauri::command]
fn add_source(
    pins: State<'_, Mutex<PinStore>>,
    lists: State<'_, Mutex<ListStore>>,
    source: AddSource,
) -> Result<SourceList, String> {
    let _operation = lists_op();
    let url = {
        let store = lists.lock().unwrap_or_else(|err| err.into_inner());
        store.ensure_can_add(&source.url)?
    };
    let (text, etag) = match lists::download_list(&url, "") {
        Download::Updated { text, etag } => (text, etag),
        Download::NotModified => return Err("error.listDownload".to_string()),
        Download::Failed(error) => return Err(error),
    };
    {
        let mut store = lists.lock().unwrap_or_else(|err| err.into_inner());
        store.insert(&source.title, &url, &text, &etag)?;
    }
    Ok(source_list(&pins, &lists))
}

#[tauri::command]
fn remove_source(
    pins: State<'_, Mutex<PinStore>>,
    lists: State<'_, Mutex<ListStore>>,
    id: String,
) -> Result<SourceList, String> {
    let _operation = lists_op();
    lists
        .lock()
        .unwrap_or_else(|err| err.into_inner())
        .remove(&id)?;
    Ok(source_list(&pins, &lists))
}

#[tauri::command]
fn set_source_enabled(
    pins: State<'_, Mutex<PinStore>>,
    lists: State<'_, Mutex<ListStore>>,
    id: String,
    enabled: bool,
) -> Result<SourceList, String> {
    let _operation = lists_op();
    lists
        .lock()
        .unwrap_or_else(|err| err.into_inner())
        .set_enabled(&id, enabled)?;
    Ok(source_list(&pins, &lists))
}

#[tauri::command]
fn set_list_interval(
    pins: State<'_, Mutex<PinStore>>,
    lists: State<'_, Mutex<ListStore>>,
    minutes: u32,
) -> Result<SourceList, String> {
    let _operation = lists_op();
    lists
        .lock()
        .unwrap_or_else(|err| err.into_inner())
        .set_interval(minutes)?;
    Ok(source_list(&pins, &lists))
}

#[tauri::command]
fn check_sources(
    app: AppHandle,
    pins: State<'_, Mutex<PinStore>>,
    lists: State<'_, Mutex<ListStore>>,
    id: Option<String>,
) -> Result<CheckReport, String> {
    run_list_check(&app, &pins, &lists, id.as_deref(), false)
}

fn run_list_check(
    app: &AppHandle,
    pins: &State<'_, Mutex<PinStore>>,
    lists: &State<'_, Mutex<ListStore>>,
    id: Option<&str>,
    automatic: bool,
) -> Result<CheckReport, String> {
    let _operation = lists_op();
    let jobs = lists
        .lock()
        .unwrap_or_else(|err| err.into_inner())
        .jobs(id)?;
    let mut downloads = Vec::with_capacity(jobs.len());
    for job in jobs {
        downloads.push((job.id, lists::download_list(&job.url, &job.etag)));
    }
    let mut changed = 0;
    let mut unchanged = 0;
    let mut failed = 0;
    {
        let mut store = lists.lock().unwrap_or_else(|err| err.into_inner());
        for (list_id, download) in downloads {
            match store.apply_download(&list_id, download)? {
                Some(lists::CheckKind::Changed) => changed += 1,
                Some(lists::CheckKind::Unchanged) => unchanged += 1,
                Some(lists::CheckKind::Failed) => failed += 1,
                None => {}
            }
        }
        if id.is_none() {
            store.mark_full_check();
        }
        store.save()?;
    }
    if automatic && (changed > 0 || failed > 0) {
        let _ = app.emit(
            "lists-updated",
            ListsUpdated {
                changed,
                failed,
                automatic: true,
            },
        );
    }
    Ok(CheckReport {
        changed,
        unchanged,
        failed,
        sources: source_list(pins, lists),
    })
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn lists_check_loop(app: AppHandle) {
    loop {
        std::thread::sleep(Duration::from_secs(30));
        let due = app
            .state::<Mutex<ListStore>>()
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .check_due(now_secs());
        if !due {
            continue;
        }
        let pins = app.state::<Mutex<PinStore>>();
        let lists = app.state::<Mutex<ListStore>>();
        if let Err(error) = run_list_check(&app, &pins, &lists, None, true) {
            eprintln!("list check: {error}");
        }
    }
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
            app.manage(Mutex::new(ListStore::open(directory.join("lists.json"))));
            app.manage(Mutex::new(runtime::RuntimeState::open(
                directory.join("runtime.json"),
            )));
            let handle = app.handle().clone();
            std::thread::spawn(move || lists_check_loop(handle));
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
            install_update,
            hosts_status,
            apply_pins,
            turn_off_pins,
            list_sources,
            add_source,
            remove_source,
            set_source_enabled,
            set_list_interval,
            check_sources
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| runtime::on_run_event(app, &event));
}
