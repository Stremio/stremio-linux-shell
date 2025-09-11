pub const APP_ID: &str = match cfg!(debug_assertions) {
    true => "com.stremio.Stremio.Devel",
    false => "com.stremio.Stremio",
};
pub const APP_NAME: &str = "Stremio";

pub const URI_SCHEME: &str = "stremio://";
pub const URL_PROD: &str = "http://127.0.0.1:11470/proxy/d=https%3A%2F%2Fweb.stremio.com/";
pub const URL_DEV: &str = "http://localhost:8080/";
pub const PRELOAD_SCRIPT: &str = include_str!("preload.js");
