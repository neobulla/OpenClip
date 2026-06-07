#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]


#[cfg(not(target_env = "msvc"))]
use mimalloc::MiMalloc;

#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce
};
use tray_icon::menu::accelerator::{Accelerator, Modifiers, Code};
use tray_icon::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    Icon, TrayIcon,
};
use clipboard_rs::{Clipboard, ClipboardContext, ClipboardWatcher, ClipboardWatcherContext};
use std::collections::VecDeque;

mod config;
use config::Config;

// ------------- COMMON SECURITY/HISTORY FUNCS -------------
fn get_or_create_encryption_key() -> Key<Aes256Gcm> {
    let output = std::process::Command::new("security")
        .args(["find-generic-password", "-s", "com.openclip.key", "-a", "key", "-w"])
        .output();
        
    if let Ok(out) = output {
        if out.status.success() {
            if let Ok(json) = String::from_utf8(out.stdout) {
                if let Ok(bytes) = serde_json::from_str::<[u8; 32]>(&json.trim()) {
                    return *Key::<Aes256Gcm>::from_slice(&bytes);
                }
            }
        }
    }
    
    let key = Aes256Gcm::generate_key(OsRng);
    let bytes: [u8; 32] = key.into();
    let json = serde_json::to_string(&bytes).unwrap();
    
    let _ = std::process::Command::new("security")
        .args([
            "add-generic-password",
            "-s", "com.openclip.key",
            "-a", "key",
            "-w", &json,
            "-U"
        ])
        .output();
        
    key
}

fn get_history_file_path() -> std::path::PathBuf {
    let mut path = directories::ProjectDirs::from("com", "openclip", "OpenClip")
        .unwrap()
        .data_dir()
        .to_path_buf();
    std::fs::create_dir_all(&path).unwrap_or_default();
    path.push("history.enc");
    path
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct Clipping {
    pub preview: String,
    pub compressed_data: Vec<u8>,
}

impl Clipping {
    pub fn new(text: &str) -> Self {
        let preview = if text.chars().count() > 40 {
            let truncated: String = text.chars().take(37).collect();
            format!("{}...", truncated)
        } else {
            text.to_string()
        };
        let compressed_data = lz4_flex::compress_prepend_size(text.as_bytes());
        Self { preview, compressed_data }
    }

    pub fn decompress(&self) -> Option<String> {
        lz4_flex::decompress_size_prepended(&self.compressed_data)
            .ok()
            .and_then(|bytes| String::from_utf8(bytes).ok())
    }
}

fn load_history(encryption_key: &Key<Aes256Gcm>) -> VecDeque<Clipping> {
    let file_path = get_history_file_path();
    if let Ok(data) = std::fs::read(&file_path) {
        if data.len() > 12 {
            let nonce = Nonce::from_slice(&data[..12]);
            let cipher = Aes256Gcm::new(encryption_key);
            if let Ok(plaintext) = cipher.decrypt(nonce, &data[12..]) {
                if let Ok(json) = String::from_utf8(plaintext) {
                    if let Ok(saved) = serde_json::from_str::<VecDeque<Clipping>>(&json) {
                        return saved;
                    }
                    // Migration from old string format
                    if let Ok(saved_strings) = serde_json::from_str::<VecDeque<String>>(&json) {
                        let mut migrated = VecDeque::new();
                        for s in saved_strings {
                            migrated.push_back(Clipping::new(&s));
                        }
                        return migrated;
                    }
                }
            }
        }
    }
    VecDeque::new()
}

fn save_history_securely(history: &VecDeque<Clipping>, encryption_key: &Key<Aes256Gcm>) {
    if let Ok(json) = serde_json::to_string(history) {
        let cipher = Aes256Gcm::new(encryption_key);
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        if let Ok(mut ciphertext) = cipher.encrypt(&nonce, json.as_bytes()) {
            let mut data = nonce.to_vec();
            data.append(&mut ciphertext);
            let _ = std::fs::write(get_history_file_path(), data);
        }
    }
}

// ------------- DAEMON MODE -------------
enum DaemonEvent {
    ClipboardChanged(String),
    MenuEvent(tray_icon::menu::MenuEvent),
    TrayEvent(tray_icon::TrayIconEvent),
    ConfigReload,
}

struct WatcherImpl {
    proxy: winit::event_loop::EventLoopProxy<DaemonEvent>,
}
impl clipboard_rs::ClipboardHandler for WatcherImpl {
    fn on_clipboard_change(&mut self) {
        if let Ok(ctx) = clipboard_rs::ClipboardContext::new() {
            if let Ok(text) = ctx.get_text() {
                let _ = self.proxy.send_event(DaemonEvent::ClipboardChanged(text));
            }
        }
    }
}

fn update_menu(tray_icon: &TrayIcon, history: &VecDeque<Clipping>, config: &Config) -> Vec<MenuItem> {
    let menu = Menu::new();
    let mut history_items = Vec::new();
    
    let items_to_show = std::cmp::min(history.len(), config.display_clippings);
    for i in 0..items_to_show {
        let item = &history[i];
        let display = item.preview.replace('\n', " ");

        let mut accelerator = None;
        if i < 9 {
            if let Ok(accel) = std::str::FromStr::from_str(&format!("cmd+Digit{}", i + 1)) {
                accelerator = Some(accel);
            }
        }

        let menu_item = MenuItem::new(&display, true, accelerator);
        history_items.push(menu_item.clone());
        let _ = menu.append(&menu_item);
    }

    if history.is_empty() {
        let empty = MenuItem::with_id("empty", "History is empty", false, None);
        let _ = menu.append(&empty);
    }

    let _ = menu.append(&PredefinedMenuItem::separator());
    let clear = MenuItem::with_id("clear", "Clear History", !history.is_empty(), None);
    let _ = menu.append(&clear);
    let _ = menu.append(&PredefinedMenuItem::separator());
    
    let prefs = MenuItem::with_id("prefs", "Preferences...", true, None);
    let _ = menu.append(&prefs);
    
    let quit = MenuItem::with_id("quit", "Quit OpenClip", true, None);
    let _ = menu.append(&quit);

    let _ = tray_icon.set_menu(Some(Box::new(menu)));
    history_items
}

fn run_daemon_mode() -> Result<(), Box<dyn std::error::Error>> {
    // Single instance lock (Secure POSIX flock inside user's Application Support dir)
    let _single_instance_lock = match Config::lock_instance("daemon") {
        Some(lock) => lock,
        None => {
            println!("OpenClip is already running. Exiting.");
            std::process::exit(0);
        }
    };

    let mut config = Config::load();
    #[cfg(target_os = "macos")]
    {
        use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
        use objc2::MainThreadMarker;
        unsafe {
            if let Some(mtm) = MainThreadMarker::new() {
                let app = NSApplication::sharedApplication(mtm);
                app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
            }
        }
    }

    let mut config = Config::load();
    let encryption_key = get_or_create_encryption_key();
    let mut history = load_history(&encryption_key);

    let icon_data = include_bytes!("../MenuBarIcon.png");
    let image = image::load_from_memory(icon_data).unwrap().into_rgba8();
    let (width, height) = image.dimensions();
    let icon = Icon::from_rgba(image.into_raw(), width, height).unwrap();

    let tray_icon = tray_icon::TrayIconBuilder::new()
        .with_icon(icon)
        .with_icon_as_template(true)
        .with_tooltip("OpenClip")
        .build()?;

    let mut history_items = update_menu(&tray_icon, &history, &config);

    let event_loop = winit::event_loop::EventLoop::<DaemonEvent>::with_user_event().build()?;
    let proxy = event_loop.create_proxy();

    let proxy_tray = proxy.clone();
    tray_icon::TrayIconEvent::set_event_handler(Some(move |e| {
        let _ = proxy_tray.send_event(DaemonEvent::TrayEvent(e));
    }));

    let proxy_menu = proxy.clone();
    tray_icon::menu::MenuEvent::set_event_handler(Some(move |e| {
        let _ = proxy_menu.send_event(DaemonEvent::MenuEvent(e));
    }));

    // Spawn watcher
    let mut watcher = clipboard_rs::ClipboardWatcherContext::new().map_err(|e| e.to_string())?;
    let watcher_impl = WatcherImpl { proxy: proxy.clone() };
    watcher.add_handler(watcher_impl);
    std::thread::spawn(move || {
        watcher.start_watch();
    });
    
    // Config reload loop
    let proxy_cfg = proxy.clone();
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(std::time::Duration::from_secs(2));
            let _ = proxy_cfg.send_event(DaemonEvent::ConfigReload);
        }
    });

    let clipboard_ctx = clipboard_rs::ClipboardContext::new().map_err(|e| e.to_string())?;

    event_loop.run(move |event, elwt| {
        elwt.set_control_flow(winit::event_loop::ControlFlow::Wait);

        if let winit::event::Event::UserEvent(e) = event {
            match e {
                DaemonEvent::ConfigReload => {
                    let mut new_config = Config::load();
                    if new_config.clear_history_flag {
                        history.clear();
                        save_history_securely(&history, &encryption_key);
                        new_config.clear_history_flag = false;
                        new_config.save();
                        history_items = update_menu(&tray_icon, &history, &new_config);
                    }
                    config = new_config;
                    if history.len() > config.remember_clippings {
                        history.truncate(config.remember_clippings);
                        save_history_securely(&history, &encryption_key);
                        history_items = update_menu(&tray_icon, &history, &config);
                    }
                }
                DaemonEvent::ClipboardChanged(text) => {
                    if !config.record_history { return; }
                    let trimmed = text.trim();
                    if trimmed.is_empty() { return; }
                    if history.front().and_then(|c| c.decompress()) == Some(trimmed.to_string()) { return; }
                    if let Some(pos) = history.iter().position(|x| x.decompress() == Some(trimmed.to_string())) {
                        history.remove(pos);
                    }
                    history.push_front(Clipping::new(trimmed));
                    if history.len() > config.remember_clippings {
                        history.truncate(config.remember_clippings);
                    }
                    save_history_securely(&history, &encryption_key);
                    history_items = update_menu(&tray_icon, &history, &config);
                }
                DaemonEvent::MenuEvent(event) => {
                    if event.id() == "prefs" {
                        let mut current_exe = std::env::current_exe().unwrap();
                        current_exe.pop();
                        current_exe.push("openclip_gui");
                        let _ = std::process::Command::new(current_exe).spawn();
                    } else if event.id() == "clear" {
                        history.clear();
                        save_history_securely(&history, &encryption_key);
                        history_items = update_menu(&tray_icon, &history, &config);
                    } else if event.id() == "quit" {
                        elwt.exit();
                    } else {
                        if let Some(idx) = history_items.iter().position(|item| item.id() == event.id()) {
                            if let Some(item) = history.get(idx) {
                                if let Some(text) = item.decompress() {
                                    let _ = clipboard_ctx.set_text(text);
                                }
                            }
                        }
                    }
                }
                DaemonEvent::TrayEvent(_) => {}
            }
        }
    })?;

    Ok(())
}


fn main() -> Result<(), Box<dyn std::error::Error>> {
    run_daemon_mode()?;
    Ok(())
}
