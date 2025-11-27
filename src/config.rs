use std::{
    env, fs,
    path::{Path, PathBuf},
};

use crate::constants::DATA_DIR;

pub struct Config {
    pub instance: InstanceConfig,
    pub server: ServerConfig,
    pub webview: WebViewConfig,
    pub tray: TrayConfig,
}

impl Config {
    pub fn new() -> Self {
        let data_dir = dirs::data_dir()
            .expect("Failed to get data dir")
            .join(DATA_DIR);

        let runtime_dir = dirs::runtime_dir()
            .expect("Failed to get runtime dir")
            .join(DATA_DIR);

        fs::create_dir_all(&data_dir).expect("Failed to create data directory");
        fs::create_dir_all(&runtime_dir).expect("Failed to create runtime directory");

        let current_exe_path = env::current_exe().expect("Failed to get current exe path");
        let current_dir = current_exe_path
            .parent()
            .expect("Failed to get current directory");

        let instance = InstanceConfig::new(&runtime_dir);
        let server = ServerConfig::new(current_dir);
        let webview = WebViewConfig::new(&data_dir);
        let tray = TrayConfig::new(&runtime_dir);

        Self {
            instance,
            server,
            webview,
            tray,
        }
    }
}

const INSTANCE_SOCKET_FILE: &str = "stremio.sock";

pub struct InstanceConfig {
    pub socket_file: PathBuf,
}

impl InstanceConfig {
    pub fn new(runtime_dir: &Path) -> Self {
        let socket_file = runtime_dir.join(INSTANCE_SOCKET_FILE);

        Self { socket_file }
    }

    pub fn remove_socket_file(&self) {
        let _ = fs::remove_file(&self.socket_file);
    }
}

const SERVER_FILE: &str = "server.js";

pub struct ServerConfig {
    pub file: PathBuf,
}

impl ServerConfig {
    pub fn new(current_dir: &Path) -> Self {
        let file = current_dir.join(SERVER_FILE);

        Self { file }
    }
}

const CEF_DIR: &str = "cef";
const CEF_CACHE_DIR: &str = "cache";
const CEF_LOG_FILE: &str = "log";
const CEF_LOCK_FILE: &str = "SingletonLock";

pub struct WebViewConfig {
    pub cache_dir: PathBuf,
    pub log_file: PathBuf,
    pub lock_file: PathBuf,
}

impl WebViewConfig {
    pub fn new(data_dir: &Path) -> Self {
        let cef_dir = data_dir.join(CEF_DIR);
        let cache_dir = cef_dir.join(CEF_CACHE_DIR);
        let log_file = cef_dir.join(CEF_LOG_FILE);
        let lock_file = cache_dir.join(CEF_LOCK_FILE);

        Self {
            cache_dir,
            log_file,
            lock_file,
        }
    }

    pub fn remove_lock_file(&self) {
        let _ = fs::remove_file(&self.lock_file);
    }
}

const TRAY_ICON_DIR: &str = "tray";

pub struct TrayConfig {
    pub icon_path: PathBuf,
}

impl TrayConfig {
    pub fn new(runtime_path: &Path) -> Self {
        let icon_path = runtime_path.join(TRAY_ICON_DIR);

        Self { icon_path }
    }
}
