//! App shell: tray icon, the popover and main windows, and the commands and
//! events the UI talks to. Command and event names match
//! `src/lib/tauriBackend.ts`.

mod collector;
mod demo;
mod log;
mod model;
mod twitch;
mod updater;

use std::{
    fs,
    path::PathBuf,
    sync::Mutex,
    thread,
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, PhysicalPosition, Rect, State, WindowEvent,
};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};
use tauri_plugin_opener::OpenerExt;

use collector::Collector;
use model::{ActivityEvent, Followed, Snapshot};
use twitch::engine::{self, Engine};
use updater::Updater;

struct AppState {
    /// Made-up data and a simulated collector instead of Twitch.
    demo: bool,
    collector: Mutex<Collector>,
    /// When the popover last hid itself on losing focus.
    popover_hidden_at: Mutex<Option<Instant>>,
}

// ---- Saved settings ---------------------------------------------------------

#[derive(Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct Saved {
    start_paused: bool,
    /// The welcome tour has been seen or skipped. Missing in files written
    /// before the tour existed, which shows it once.
    welcomed: bool,
    /// Download updates without being asked. Missing means yes.
    auto_update: Option<bool>,
    /// Channels in priority order. `None` until the list is first changed.
    channels: Option<Vec<SavedChannel>>,
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum SavedChannel {
    /// How early builds saved a channel: just its name.
    Name(String),
    Full { login: String, name: String },
}

impl SavedChannel {
    fn into_pair(self) -> (String, String) {
        match self {
            Self::Name(name) => (name.to_lowercase(), name),
            Self::Full { login, name } => (login, name),
        }
    }
}

/// A folder named `data` beside the executable makes that copy of the app
/// portable: it keeps everything it writes in there.
fn portable_dir() -> Option<PathBuf> {
    let dir = std::env::current_exe().ok()?.parent()?.join("data");
    dir.is_dir().then_some(dir)
}

/// Where the settings, the activity log and the device id are kept: the
/// portable folder if there is one, otherwise the user's app-data folder.
pub(crate) fn data_dir(app: &AppHandle) -> Option<PathBuf> {
    portable_dir().or_else(|| app.path().app_config_dir().ok())
}

fn saved_path(app: &AppHandle) -> Option<PathBuf> {
    Some(data_dir(app)?.join("settings.json"))
}

fn load_saved(app: &AppHandle) -> Saved {
    saved_path(app)
        .and_then(|path| fs::read_to_string(path).ok())
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default()
}

fn save(app: &AppHandle, collector: &Collector) {
    // Demo mode works on made-up channels; they must never replace the real list.
    if app.state::<AppState>().demo {
        return;
    }
    let Some(path) = saved_path(app) else { return };
    let saved = Saved {
        start_paused: collector.start_paused,
        welcomed: collector.welcomed,
        auto_update: Some(collector.auto_update),
        channels: Some(
            collector
                .saved_channels()
                .into_iter()
                .map(|(login, name)| SavedChannel::Full { login, name })
                .collect(),
        ),
    };
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    if let Ok(json) = serde_json::to_string_pretty(&saved) {
        if let Err(e) = fs::write(&path, json) {
            crate::log!("could not save {}: {e}", path.display());
        }
    }
}

fn activity_path(app: &AppHandle) -> Option<PathBuf> {
    Some(data_dir(app)?.join("activity.json"))
}

fn load_activity(app: &AppHandle) -> Vec<ActivityEvent> {
    activity_path(app)
        .and_then(|path| fs::read_to_string(path).ok())
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default()
}

fn save_activity(app: &AppHandle, events: &[ActivityEvent]) {
    let Some(path) = activity_path(app) else { return };
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    if let Ok(json) = serde_json::to_string(events) {
        if let Err(e) = fs::write(&path, json) {
            crate::log!("could not save {}: {e}", path.display());
        }
    }
}

// ---- State changes ----------------------------------------------------------

/// Change the collector, then push the new state to every window.
pub(crate) fn update<R>(
    app: &AppHandle,
    persist: bool,
    change: impl FnOnce(&mut Collector) -> R,
) -> (R, Snapshot) {
    let state = app.state::<AppState>();
    let (result, snapshot) = {
        let mut collector = state.collector.lock().unwrap();
        let result = change(&mut collector);
        if persist {
            save(app, &collector);
        }
        (result, collector.snapshot())
    };
    let _ = app.emit("state", &snapshot);
    (result, snapshot)
}

/// Change the collector, and push the new state only if `change` reports
/// that something the UI shows has changed.
pub(crate) fn update_if(app: &AppHandle, change: impl FnOnce(&mut Collector) -> bool) {
    let state = app.state::<AppState>();
    let snapshot = {
        let mut collector = state.collector.lock().unwrap();
        if !change(&mut collector) {
            return;
        }
        collector.snapshot()
    };
    let _ = app.emit("state", &snapshot);
}

/// Send new or updated activity entries to the windows and save the log.
pub(crate) fn publish_activity(app: &AppHandle, events: &[ActivityEvent]) {
    if events.is_empty() {
        return;
    }
    for event in events {
        let _ = app.emit("activity", event);
    }
    let state = app.state::<AppState>();
    if !state.demo {
        let all = state.collector.lock().unwrap().activity();
        save_activity(app, &all);
    }
}

fn demo_tick(app: &AppHandle) {
    let state = app.state::<AppState>();
    let (events, snapshot) = {
        let mut collector = state.collector.lock().unwrap();
        let Some(events) = collector.demo_tick() else { return };
        (events, collector.snapshot())
    };
    for event in &events {
        let _ = app.emit("activity", event);
    }
    let _ = app.emit("state", &snapshot);
}

fn is_demo(app: &AppHandle) -> bool {
    app.state::<AppState>().demo
}

/// Demo only: loading now, signed in shortly after.
fn demo_connect(app: &AppHandle) -> Snapshot {
    let snapshot = update(app, false, |c| c.set_status(model::Status::Loading)).1;
    let app = app.clone();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(900));
        update(&app, false, |c| c.signed_in(demo::ACCOUNT));
    });
    snapshot
}

fn current(app: &AppHandle) -> Snapshot {
    app.state::<AppState>().collector.lock().unwrap().snapshot()
}

// ---- Commands ---------------------------------------------------------------

#[tauri::command]
fn get_state(state: State<AppState>) -> Snapshot {
    state.collector.lock().unwrap().snapshot()
}

#[tauri::command]
fn get_activity(state: State<AppState>) -> Vec<ActivityEvent> {
    state.collector.lock().unwrap().activity()
}

#[tauri::command]
fn set_paused(app: AppHandle, paused: bool) -> Snapshot {
    update(&app, false, |c| c.set_paused(paused)).1
}

/// In real mode this asks Twitch whether the channel exists first.
#[tauri::command]
async fn add_channel(app: AppHandle, name: String) -> Result<Snapshot, String> {
    if !is_demo(&app) {
        return engine::add_channel(&app, &name).await;
    }
    let (result, snapshot) = update(&app, true, |c| c.add_channel(&name));
    result.map(|()| snapshot)
}

/// The channels the signed-in user follows on Twitch. Read-only.
#[tauri::command]
async fn get_followed(app: AppHandle) -> Result<Vec<Followed>, String> {
    if is_demo(&app) {
        return Ok(demo::followed().into_iter().map(|(login, name)| Followed { login, name, avatar: None }).collect());
    }
    engine::followed(&app).await
}

/// Adds followed channels, by login, to the end of the list.
#[tauri::command]
fn add_followed(app: AppHandle, logins: Vec<String>) -> Snapshot {
    if !is_demo(&app) {
        return engine::add_followed(&app, &logins);
    }
    let known: Vec<_> = demo::followed().into_iter().filter(|(login, _)| logins.contains(login)).collect();
    update(&app, true, |c| {
        c.add_known(&known);
        c.settle_all();
    })
    .1
}

#[tauri::command]
fn remove_channel(app: AppHandle, id: String) -> Snapshot {
    update(&app, true, |c| c.remove_channel(&id)).1
}

#[tauri::command]
fn undo_remove(app: AppHandle) -> Snapshot {
    update(&app, true, Collector::undo_remove).1
}

#[tauri::command]
fn move_channel(app: AppHandle, id: String, index: usize) -> Snapshot {
    update(&app, true, |c| c.move_channel(&id, index)).1
}

/// Starts the device-code sign-in. The code arrives in a later snapshot, as
/// `sign_in`, once Twitch has issued it.
#[tauri::command]
fn sign_in(app: AppHandle) -> Snapshot {
    if is_demo(&app) {
        return demo_connect(&app);
    }
    tauri::async_runtime::spawn(engine::sign_in(app.clone()));
    current(&app)
}

#[tauri::command]
fn cancel_sign_in(app: AppHandle) -> Snapshot {
    if !is_demo(&app) {
        engine::cancel_sign_in(&app);
    }
    current(&app)
}

/// Opens the Twitch activation page in the user's browser.
#[tauri::command]
fn open_sign_in_page(app: AppHandle) -> Result<(), String> {
    let Some(sign_in) = current(&app).sign_in else {
        return Err("No sign-in code is waiting.".into());
    };
    // Only ever open the address Twitch gave us, and only if it is on twitch.tv.
    if !sign_in.verification_uri.starts_with("https://www.twitch.tv/") {
        return Err("Twitch returned an unexpected sign-in address.".into());
    }
    app.opener()
        .open_url(sign_in.verification_uri, None::<&str>)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn reconnect(app: AppHandle) -> Snapshot {
    if is_demo(&app) {
        return demo_connect(&app);
    }
    tauri::async_runtime::spawn(engine::connect(app.clone()));
    current(&app)
}

#[tauri::command]
fn sign_out(app: AppHandle) -> Snapshot {
    if is_demo(&app) {
        update(&app, false, Collector::signed_out).1
    } else {
        engine::sign_out(&app);
        current(&app)
    }
}

#[tauri::command]
fn set_open_at_login(app: AppHandle, enabled: bool) -> Result<Snapshot, String> {
    let autolaunch = app.autolaunch();
    let result = if enabled { autolaunch.enable() } else { autolaunch.disable() };
    result.map_err(|e| e.to_string())?;
    Ok(update(&app, false, |c| c.open_at_login = enabled).1)
}

#[tauri::command]
fn set_start_paused(app: AppHandle, enabled: bool) -> Snapshot {
    update(&app, true, |c| c.start_paused = enabled).1
}

/// Look for an update now. With "Update automatically" on, a newer version is
/// also downloaded; otherwise it is only announced.
#[tauri::command]
fn check_update(app: AppHandle) -> Snapshot {
    let download = app.state::<AppState>().collector.lock().unwrap().auto_update;
    tauri::async_runtime::spawn(updater::check(app.clone(), download));
    current(&app)
}

/// Download the update that was announced.
#[tauri::command]
fn download_update(app: AppHandle) -> Snapshot {
    tauri::async_runtime::spawn(updater::check(app.clone(), true));
    current(&app)
}

/// Put the downloaded update in place and start the app again.
#[tauri::command]
fn install_update(app: AppHandle) -> Result<(), String> {
    if !updater::install_staged(&app)? {
        return Err("No update is ready to install.".into());
    }
    relaunch(&app)
}

#[tauri::command]
fn set_auto_update(app: AppHandle, enabled: bool) -> Snapshot {
    update(&app, true, |c| c.auto_update = enabled).1
}

/// Show the log file in its folder, to read or to send.
#[tauri::command]
fn show_log(app: AppHandle) -> Result<(), String> {
    let path = log::path().ok_or("There is no log file yet.")?;
    if !path.exists() {
        log!("log file shown on request");
    }
    app.opener().reveal_item_in_dir(path).map_err(|e| e.to_string())
}

/// Start the app again, in place of this copy.
fn relaunch(app: &AppHandle) -> ! {
    // Let go of the one-copy lock first, or the new copy would find this one
    // still holding it, hand over to it and exit, leaving nothing running.
    if demo::single_instance() {
        tauri_plugin_single_instance::destroy(app);
    }
    app.restart()
}

/// Quit for good. A downloaded update goes in on the way out, so the next
/// start is the new version.
fn quit_app(app: &AppHandle) {
    if let Err(why) = updater::install_staged(app) {
        log!("update: {why}");
    }
    log!("quit");
    app.exit(0);
}

/// Quit from the window. Closing the window only hides it, and the tray icon
/// is often tucked away, so Settings offers the way out too.
#[tauri::command]
fn quit_now(app: AppHandle) {
    quit_app(&app);
}

/// Records that the welcome tour has been seen or skipped.
#[tauri::command]
fn set_welcomed(app: AppHandle, welcomed: bool) -> Snapshot {
    update(&app, true, |c| c.welcomed = welcomed).1
}

/// Shows the main window, on `section` if one is given, otherwise where it was.
#[tauri::command]
fn open_main(app: AppHandle, section: Option<String>) {
    show_main(&app);
    if let Some(section) = section {
        let _ = app.emit_to("main", "navigate", section);
    }
    if let Some(popover) = app.get_webview_window("popover") {
        let _ = popover.hide();
    }
}

// ---- Windows and tray -------------------------------------------------------

fn show_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// Show the popover next to the tray icon, or hide it if it is already open.
fn toggle_popover(app: &AppHandle, tray: Rect) {
    let Some(window) = app.get_webview_window("popover") else { return };
    if window.is_visible().unwrap_or(false) {
        let _ = window.hide();
        return;
    }
    // Clicking the tray icon while the popover is open first blurs (and hides)
    // it, then lands here. Treat that click as "close", not "reopen".
    let just_hidden = app
        .state::<AppState>()
        .popover_hidden_at
        .lock()
        .unwrap()
        .is_some_and(|at| at.elapsed() < Duration::from_millis(250));
    if just_hidden {
        return;
    }

    let scale = window.scale_factor().unwrap_or(1.0);
    let icon = tray.position.to_physical::<f64>(scale);
    let icon_size = tray.size.to_physical::<f64>(scale);
    if let (Ok(size), Ok(Some(monitor))) =
        (window.outer_size(), app.monitor_from_point(icon.x, icon.y))
    {
        const MARGIN: f64 = 12.0;
        let (width, height) = (size.width as f64, size.height as f64);
        let left = monitor.position().x as f64;
        let top = monitor.position().y as f64;
        let right = left + monitor.size().width as f64;
        let x = (icon.x + icon_size.width / 2.0 - width / 2.0)
            .min(right - width - MARGIN)
            .max(left + MARGIN);
        // Above the icon; below it when the taskbar is at the top of the screen.
        let above = icon.y - height - MARGIN;
        let y = if above >= top { above } else { icon.y + icon_size.height + MARGIN };
        let _ = window.set_position(PhysicalPosition::new(x, y));
    }
    let _ = window.show();
    let _ = window.set_focus();
}

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open Collector", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &quit])?;

    let mut tray = TrayIconBuilder::with_id("collector")
        .tooltip("Collector")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_main(app),
            "quit" => quit_app(app),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                rect,
                ..
            } = event
            {
                toggle_popover(tray.app_handle(), rect);
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}

/// Read-only check against the real Twitch. See `twitch/probe.rs`.
#[cfg(debug_assertions)]
pub fn probe(channels: Vec<String>) {
    tokio::runtime::Runtime::new()
        .expect("tokio runtime")
        .block_on(twitch::probe::run(channels));
}

/// Read-only check of the followed-channels request. See `twitch/probe.rs`.
#[cfg(debug_assertions)]
pub fn probe_follows() {
    tokio::runtime::Runtime::new()
        .expect("tokio runtime")
        .block_on(twitch::probe::follows());
}

/// Passed by the Windows startup entry, so a launch at sign-in stays in the
/// tray. Launched by hand, the app opens its window.
const HIDDEN_ARG: &str = "--hidden";

pub fn run() {
    // A portable copy also keeps the web view's own files (its cache, the
    // theme choice) in its folder instead of the user's profile. This has to
    // be set before the first window exists.
    if let Some(dir) = portable_dir() {
        if std::env::var_os("WEBVIEW2_USER_DATA_FOLDER").is_none() {
            std::env::set_var("WEBVIEW2_USER_DATA_FOLDER", dir.join("webview"));
        }
    }

    let mut builder = tauri::Builder::default();
    // Must be first. A second copy would report watch time twice; instead it
    // hands over to the running one, which shows its window. Demo mode never
    // talks to Twitch, so a demo copy may run beside the real app.
    if demo::single_instance() {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| show_main(app)));
    }
    builder
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, Some(vec![HIDDEN_ARG])))
        .plugin(tauri_plugin_opener::init())
        // Managed before any window can call a command; filled in during setup.
        .manage(AppState {
            demo: demo::enabled(),
            collector: Mutex::new(Collector::new(false, false)),
            popover_hidden_at: Mutex::new(None),
        })
        .manage(Engine::new())
        .manage(Updater::new())
        .setup(|app| {
            let handle = app.handle().clone();
            log::start(data_dir(&handle));
            log!(
                "Collector {} started{}{}",
                updater::VERSION,
                if portable_dir().is_some() { ", portable" } else { "" },
                if is_demo(&handle) { ", demo mode" } else { "" }
            );
            updater::clear_staging(&handle);

            let saved = load_saved(&handle);
            let open_at_login = handle.autolaunch().is_enabled().unwrap_or(false);
            let demo = is_demo(&handle);
            let mut collector = if demo {
                Collector::demo(saved.start_paused, open_at_login)
            } else {
                let mut collector = Collector::new(saved.start_paused, open_at_login);
                collector.load_events(load_activity(&handle), collector::now_ms());
                collector
            };
            collector.welcomed = saved.welcomed;
            collector.auto_update = saved.auto_update.unwrap_or(true);
            collector.update = handle.state::<Updater>().initial();
            if let Some(channels) = saved.channels {
                let pairs: Vec<_> = channels.into_iter().map(SavedChannel::into_pair).collect();
                collector.restore_channels(&pairs);
            }
            *handle.state::<AppState>().collector.lock().unwrap() = collector;

            build_tray(&handle)?;
            if !std::env::args().any(|arg| arg == HIDDEN_ARG) {
                show_main(&handle);
            }
            tauri::async_runtime::spawn(updater::keep_checking(handle.clone()));

            if demo {
                thread::spawn(move || loop {
                    thread::sleep(Duration::from_secs(1));
                    demo_tick(&handle);
                });
            } else {
                tauri::async_runtime::spawn(engine::connect(handle));
            }
            Ok(())
        })
        .on_window_event(|window, event| match (window.label(), event) {
            // The popover is a flyout: it closes when you click elsewhere.
            ("popover", WindowEvent::Focused(false)) => {
                let _ = window.hide();
                *window.state::<AppState>().popover_hidden_at.lock().unwrap() =
                    Some(Instant::now());
            }
            // Closing the main window keeps the app running in the tray.
            ("main", WindowEvent::CloseRequested { api, .. }) => {
                api.prevent_close();
                let _ = window.hide();
            }
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            get_state,
            get_activity,
            set_paused,
            add_channel,
            get_followed,
            add_followed,
            remove_channel,
            undo_remove,
            move_channel,
            sign_in,
            cancel_sign_in,
            open_sign_in_page,
            reconnect,
            sign_out,
            set_open_at_login,
            set_start_paused,
            set_welcomed,
            check_update,
            download_update,
            install_update,
            set_auto_update,
            show_log,
            quit_now,
            open_main,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Collector");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_settings_file_from_before_the_tour_gets_the_tour_once() {
        let old: Saved =
            serde_json::from_str(r#"{"startPaused":true,"channels":[{"login":"lirik","name":"LIRIK"}]}"#).unwrap();
        assert!(old.start_paused);
        assert!(!old.welcomed);

        let seen = Saved { welcomed: true, ..Saved::default() };
        let json = serde_json::to_string(&seen).unwrap();
        assert!(json.contains(r#""welcomed":true"#));
        let back: Saved = serde_json::from_str(&json).unwrap();
        assert!(back.welcomed);
    }
}
