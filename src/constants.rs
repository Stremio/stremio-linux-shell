pub const APP_ID: &str = match cfg!(debug_assertions) {
    true => "com.stremio.Stremio.Devel",
    false => "com.stremio.Stremio",
};

pub const APP_NAME: &str = "Stremio";
pub const WINDOW_SIZE: (i32, i32) = (1700, 1050);
pub const STARTUP_URL: &str = "https:/web.stremio.com";
pub const URI_SCHEME: &str = "stremio://";
pub const DATA_DIR: &str = "stremio";

pub const CMD_SWITCHES: &[&str] = &[
    "enable-gpu",
    "enable-gpu-rasterization",
    "ignore-gpu-blocklist",
    "enable-zero-copy",
    "enable-native-gpu-memory-buffers",
    "enable-accelerated-video-decode",
    // "enable-begin-frame-scheduling",
    // "disable-frame-rate-limit",
    // "disable-gpu-vsync",
    // "disable-gpu-compositing",
    // "disable-direct-composition",
    // "disable-gpu",
];
