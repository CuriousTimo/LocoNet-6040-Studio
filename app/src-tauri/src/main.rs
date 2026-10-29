// Prevents an additional console window on Windows in release builds
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod nano;

use std::fs;
use std::sync::atomic::{AtomicBool, Ordering};

use l6040_core::backup::{self, Parsed};
use l6040_core::keymap;
use l6040_core::Layout;
use serde::Serialize;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{
    AppHandle, Emitter, Manager, Runtime, Theme, WebviewUrl, WebviewWindowBuilder, WindowEvent,
};

/// Whether the layout has changes that are not saved; the window tells us.
static DIRTY: AtomicBool = AtomicBool::new(false);

#[tauri::command]
fn set_dirty(dirty: bool) {
    DIRTY.store(dirty, Ordering::Relaxed);
}

/// Closes the window after the user chose to discard the changes.
#[tauri::command]
fn close_window(app: AppHandle) {
    DIRTY.store(false, Ordering::Relaxed);
    app.exit(0);
}

/// Close (or quit) unless there are unsaved changes; then the window asks and calls
/// `close_window`.
fn request_close<R: Runtime>(app: &AppHandle<R>) -> bool {
    if DIRTY.load(Ordering::Relaxed) {
        let _ = app.emit("close-requested", ());
        false
    } else {
        true
    }
}

fn read_file(path: &str) -> Result<String, String> {
    fs::read_to_string(path).map_err(|error| format!("Cannot open {path}: {error}"))
}

fn write_file(path: &str, text: &str) -> Result<(), String> {
    fs::write(path, text).map_err(|error| format!("Cannot save {path}: {error}"))
}

/// A layout file given when the app was started ("Open with", or on the command line).
#[tauri::command]
fn startup_file() -> Option<String> {
    std::env::args()
        .skip(1)
        .find(|arg| !arg.starts_with('-') && arg.ends_with(".json"))
}

/// How the window starts, from the command line, for documentation screenshots:
/// `--keyboard 2 --pair 9 --settings` (and `--about` for the About window).
#[derive(Serialize, Default)]
struct StartupView {
    keyboard: Option<u8>,
    /// Key pair 1 to 16.
    pair: Option<u8>,
    settings: bool,
}

fn arg_value(name: &str) -> Option<u8> {
    let args: Vec<String> = std::env::args().collect();
    args.iter()
        .position(|arg| arg == name)
        .and_then(|index| args.get(index + 1))
        .and_then(|value| value.parse().ok())
}

#[tauri::command]
fn startup_view() -> StartupView {
    StartupView {
        keyboard: arg_value("--keyboard"),
        pair: arg_value("--pair"),
        settings: std::env::args().any(|arg| arg == "--settings"),
    }
}

#[tauri::command]
fn new_layout() -> Layout {
    let mut layout = Layout::default();
    layout.keyboard_mut(1);
    layout
}

#[tauri::command]
fn open_layout(path: String) -> Result<Layout, String> {
    Layout::from_json(&read_file(&path)?).map_err(|error| error.to_string())
}

#[tauri::command]
fn save_layout(path: String, layout: Layout) -> Result<(), String> {
    write_file(&path, &layout.to_json())
}

/// A backup file: what the Nano's `backup` wrote, or what Export backup saved.
#[tauri::command]
fn import_backup(path: String) -> Result<Parsed, String> {
    Ok(backup::parse(&read_file(&path)?))
}

#[tauri::command]
fn export_backup(path: String, layout: Layout) -> Result<(), String> {
    write_file(&path, &backup::to_text(&layout))
}

#[tauri::command]
fn export_keymap(path: String, layout: Layout) -> Result<(), String> {
    write_file(&path, &keymap::to_keymap_h(&layout))
}

#[tauri::command]
fn problems(layout: Layout) -> Vec<String> {
    layout.problems()
}

/// A turnout number as Rocrail's MADA, PADA and FADA write it.
#[tauri::command]
fn rocrail_addresses(turnout: u16) -> l6040_core::rocrail::RocrailAddresses {
    l6040_core::rocrail::addresses(turnout)
}

#[tauri::command]
fn list_ports() -> Vec<nano::PortInfo> {
    nano::ports()
}

#[derive(Serialize)]
struct FromNano {
    parsed: Parsed,
    firmware: Option<String>,
}

#[tauri::command]
async fn read_from_nano(port: String) -> Result<FromNano, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut nano = nano::Nano::connect(&port)?;
        let parsed = backup::parse(&nano.read_backup()?);
        Ok(FromNano {
            parsed,
            firmware: nano.firmware,
        })
    })
    .await
    .map_err(|error| error.to_string())?
}

#[derive(Serialize, Clone)]
struct Progress {
    done: usize,
    total: usize,
}

#[derive(Serialize)]
struct Sent {
    firmware: Option<String>,
    /// Where the Nano differs from the layout after sending, read back; empty if equal.
    differences: Vec<String>,
}

/// Sends the layout to the Nano one command at a time, then reads it back to check.
#[tauri::command]
async fn send_to_nano(app: AppHandle, port: String, layout: Layout) -> Result<Sent, String> {
    let problems = layout.problems();
    if !problems.is_empty() {
        return Err(format!(
            "Please correct this first:\n{}",
            problems.join("\n")
        ));
    }
    tauri::async_runtime::spawn_blocking(move || {
        let mut nano = nano::Nano::connect(&port)?;
        let commands = backup::to_commands(&layout);
        let total = commands.len();
        for (done, command) in commands.iter().enumerate() {
            let _ = app.emit("send-progress", Progress { done, total });
            nano.exchange(command)?;
        }
        let _ = app.emit("send-progress", Progress { done: total, total });
        let on_the_nano = backup::parse(&nano.read_backup()?);
        Ok(Sent {
            firmware: nano.firmware,
            differences: backup::differences(&layout, &on_the_nano.layout),
        })
    })
    .await
    .map_err(|error| error.to_string())?
}

fn build_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let item = |id: &str, text: &str, accelerator: Option<&str>| {
        MenuItem::with_id(app, id, text, true, accelerator)
    };
    let sep = || PredefinedMenuItem::separator(app);
    let quit = item("quit", "Quit LocoNet 6040 Studio", Some("CmdOrCtrl+Q"))?;
    let mut file_items = vec![
        item("new", "New", Some("CmdOrCtrl+N"))?,
        item("open", "Open…", Some("CmdOrCtrl+O"))?,
        item("save", "Save", Some("CmdOrCtrl+S"))?,
        item("save-as", "Save As…", Some("CmdOrCtrl+Shift+S"))?,
        item("import-backup", "Import Backup…", None)?,
        item("export-backup", "Export Backup…", None)?,
        item("export-keymap", "Export KeyMap.h…", None)?,
    ];
    let file = Submenu::with_id(app, "file", "File", true)?;
    for (index, entry) in file_items.drain(..).enumerate() {
        if index == 4 {
            file.append(&sep()?)?;
        }
        file.append(&entry)?;
    }
    if !cfg!(target_os = "macos") {
        file.append(&sep()?)?;
        file.append(&quit)?;
    }
    let edit = Submenu::with_items(
        app,
        "Edit",
        true,
        &[
            &PredefinedMenuItem::undo(app, None)?,
            &PredefinedMenuItem::redo(app, None)?,
            &sep()?,
            &PredefinedMenuItem::cut(app, None)?,
            &PredefinedMenuItem::copy(app, None)?,
            &PredefinedMenuItem::paste(app, None)?,
            &PredefinedMenuItem::select_all(app, None)?,
        ],
    )?;
    let nano = Submenu::with_items(
        app,
        "Nano",
        true,
        &[
            &item("read", "Read from Nano", Some("CmdOrCtrl+R"))?,
            &item("send", "Send to Nano", Some("CmdOrCtrl+U"))?,
        ],
    )?;
    let window = Submenu::with_items(
        app,
        "Window",
        true,
        &[
            &PredefinedMenuItem::minimize(app, None)?,
            &PredefinedMenuItem::maximize(app, None)?,
            &PredefinedMenuItem::fullscreen(app, None)?,
            &sep()?,
            &PredefinedMenuItem::close_window(app, None)?,
        ],
    )?;
    let about = item("about", "About LocoNet 6040 Studio", None)?;
    if cfg!(target_os = "macos") {
        let app_menu = Submenu::with_items(
            app,
            "LocoNet 6040 Studio",
            true,
            &[
                &about,
                &sep()?,
                &PredefinedMenuItem::services(app, None)?,
                &sep()?,
                &PredefinedMenuItem::hide(app, None)?,
                &PredefinedMenuItem::hide_others(app, None)?,
                &PredefinedMenuItem::show_all(app, None)?,
                &sep()?,
                &quit,
            ],
        )?;
        Menu::with_items(app, &[&app_menu, &file, &edit, &nano, &window])
    } else {
        let help = Submenu::with_items(app, "Help", true, &[&about])?;
        Menu::with_items(app, &[&file, &edit, &nano, &window, &help])
    }
}

#[derive(Serialize)]
struct AboutInfo {
    version: &'static str,
}

#[tauri::command]
fn about_info() -> AboutInfo {
    AboutInfo {
        version: env!("CARGO_PKG_VERSION"),
    }
}

/// Our own about window (about.html): the native about panels can't show links.
fn show_about<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window("about") {
        return window.set_focus();
    }
    WebviewWindowBuilder::new(app, "about", WebviewUrl::App("about.html".into()))
        .title("About LocoNet 6040 Studio")
        .inner_size(360.0, 470.0)
        .resizable(false)
        .minimizable(false)
        .maximizable(false)
        .center()
        .theme(Some(Theme::Dark))
        .background_color(tauri::window::Color(0x0E, 0x10, 0x14, 0xFF))
        .build()?;
    Ok(())
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .menu(build_menu)
        .setup(|app| {
            if std::env::args().any(|arg| arg == "--about") {
                show_about(app.handle())?;
            }
            Ok(())
        })
        .on_menu_event(|app, event| {
            let id = event.id().as_ref();
            if id == "quit" {
                if request_close(app) {
                    app.exit(0);
                }
            } else if id == "about" {
                let _ = show_about(app);
            } else {
                let _ = app.emit("menu", id);
            }
        })
        .on_window_event(|window, event| {
            if window.label() != "main" {
                return;
            }
            match event {
                WindowEvent::CloseRequested { api, .. } if !request_close(window.app_handle()) => {
                    api.prevent_close()
                }
                // Don't keep running with only the about window left
                WindowEvent::Destroyed => window.app_handle().exit(0),
                _ => {}
            }
        })
        .invoke_handler(tauri::generate_handler![
            startup_file,
            startup_view,
            new_layout,
            open_layout,
            save_layout,
            import_backup,
            export_backup,
            export_keymap,
            problems,
            rocrail_addresses,
            list_ports,
            read_from_nano,
            send_to_nano,
            set_dirty,
            close_window,
            about_info
        ])
        .run(tauri::generate_context!())
        .expect("LocoNet 6040 Studio could not start");
}
