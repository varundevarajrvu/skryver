//! whispr tray app (M2): system tray + paperback settings UI over whispr-core.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod pipeline;

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex, RwLock};

use serde::{Deserialize, Serialize};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct Settings {
    pub engine: String,   // "parakeet" | "moonshine"
    pub hotkey_vk: i32,   // 0x70..=0x7B => F1..F12
    pub llm_mode: String, // "off" | "auto" | "always"
    pub theme: String,    // "auto" | "light" | "dark"
    pub dict: Vec<(String, String)>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            engine: "parakeet".into(),
            hotkey_vk: 0x78, // F9
            llm_mode: "auto".into(),
            theme: "light".into(),
            dict: Vec::new(),
        }
    }
}

#[derive(Serialize, Clone)]
pub struct HistoryEntry {
    pub time: String, // HH:MM:SS
    pub path: String, // "fast" | "ai"
    pub text: String,
    pub ms: u64,
}

pub struct Shared {
    pub settings: RwLock<Settings>,
    pub history: Mutex<VecDeque<HistoryEntry>>,
    pub enabled: AtomicBool,
    pub status: Mutex<String>,
}

struct Ctl(mpsc::Sender<()>); // reload signal to the pipeline thread

fn settings_path(app: &AppHandle) -> std::path::PathBuf {
    let dir = app
        .path()
        .app_config_dir()
        .expect("app config dir");
    let _ = std::fs::create_dir_all(&dir);
    dir.join("settings.json")
}

fn load_settings(app: &AppHandle) -> Settings {
    let path = settings_path(app);
    if let Ok(text) = std::fs::read_to_string(&path) {
        if let Ok(s) = serde_json::from_str::<Settings>(&text) {
            return s;
        }
    }
    // First run: seed the dictionary from the repo's whispr.dict.txt if present.
    let mut s = Settings::default();
    if let Ok(root) = whispr_core::asr::default_models_root() {
        let seed = root.join("../../whispr.dict.txt");
        if let Ok(text) = std::fs::read_to_string(seed) {
            s.dict = whispr_core::postproc::parse_dict_str(&text);
        }
    }
    s
}

fn save_settings(app: &AppHandle, s: &Settings) {
    if let Ok(json) = serde_json::to_string_pretty(s) {
        let _ = std::fs::write(settings_path(app), json);
    }
}

#[tauri::command]
fn get_state(shared: tauri::State<Arc<Shared>>) -> serde_json::Value {
    serde_json::json!({
        "settings": &*shared.settings.read().unwrap(),
        "enabled": shared.enabled.load(Ordering::Relaxed),
        "status": &*shared.status.lock().unwrap(),
    })
}

#[tauri::command]
fn get_history(shared: tauri::State<Arc<Shared>>) -> Vec<HistoryEntry> {
    shared.history.lock().unwrap().iter().cloned().collect()
}

#[tauri::command]
fn set_enabled(enabled: bool, shared: tauri::State<Arc<Shared>>) {
    shared.enabled.store(enabled, Ordering::Relaxed);
}

#[tauri::command]
fn set_settings(
    settings: Settings,
    app: AppHandle,
    shared: tauri::State<Arc<Shared>>,
    ctl: tauri::State<Ctl>,
) {
    let needs_reload = {
        let cur = shared.settings.read().unwrap();
        cur.engine != settings.engine || cur.llm_mode != settings.llm_mode
    };
    *shared.settings.write().unwrap() = settings.clone();
    save_settings(&app, &settings);
    if needs_reload {
        let _ = ctl.0.send(());
    }
}

fn open_settings_window(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.set_focus();
        return;
    }
    let _ = WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
        .title("Xenon")
        .inner_size(1120.0, 780.0)
        .min_inner_size(640.0, 480.0)
        .resizable(true)
        .maximizable(true)
        .build();
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let handle = app.handle().clone();
            let shared = Arc::new(Shared {
                settings: RwLock::new(load_settings(&handle)),
                history: Mutex::new(VecDeque::new()),
                enabled: AtomicBool::new(true),
                status: Mutex::new("starting…".into()),
            });
            let (ctl_tx, ctl_rx) = mpsc::channel::<()>();
            app.manage(shared.clone());
            app.manage(Ctl(ctl_tx));

            // Tray
            let toggle = MenuItem::with_id(app, "toggle", "Pause dictation", true, None::<&str>)?;
            let settings_item = MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit Xenon", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&toggle, &settings_item, &quit])?;
            let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/32x32.png"))?;
            let shared_for_menu = shared.clone();
            TrayIconBuilder::with_id("whispr-tray")
                .icon(icon)
                .tooltip("Xenon — hold your hotkey to dictate")
                .menu(&menu)
                .show_menu_on_left_click(true)
                .on_menu_event(move |app, event| match event.id.as_ref() {
                    "toggle" => {
                        let now = !shared_for_menu.enabled.load(Ordering::Relaxed);
                        shared_for_menu.enabled.store(now, Ordering::Relaxed);
                        if let Some(item) = app
                            .menu()
                            .and_then(|m| m.get("toggle"))
                            .and_then(|i| i.as_menuitem().cloned())
                        {
                            let _ = item.set_text(if now { "Pause dictation" } else { "Resume dictation" });
                        }
                    }
                    "settings" => open_settings_window(app),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;

            // Pipeline thread
            let pipe_shared = shared.clone();
            let pipe_handle = handle.clone();
            std::thread::spawn(move || pipeline::run(pipe_handle, pipe_shared, ctl_rx));

            // Open settings on first launch so the app isn't invisible.
            open_settings_window(&handle);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_state,
            get_history,
            set_enabled,
            set_settings
        ])
        .build(tauri::generate_context!())
        .expect("error building whispr")
        .run(|_app, event| {
            // Keep running in the tray when the settings window closes.
            if let tauri::RunEvent::ExitRequested { api, code, .. } = event {
                if code.is_none() {
                    api.prevent_exit();
                }
            }
        });
}
