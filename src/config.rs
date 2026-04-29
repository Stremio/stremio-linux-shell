pub const DATA_DIR: &str = "stremio";

pub const GETTEXT_DOMAIN: &str = "stremio";
pub const GETTEXT_DIR_DEV: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/po");
pub const GETTEXT_DIR_FLATPAK: &str = "/app/share/locale";

pub const STARTUP_URL: &str = "http://127.0.0.1:11470/proxy/d=https%3A%2F%2Fweb.stremio.com/";
