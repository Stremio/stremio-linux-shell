pub const DATA_DIR: &str = "stremio";

pub const GETTEXT_DOMAIN: &str = "stremio";
pub const GETTEXT_DIR_DEV: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/po");
/// Locale directory of a system install, set at build time by distribution packages
/// (e.g. `STREMIO_LOCALEDIR=/usr/share/locale`); unset for development and Flatpak builds.
pub const GETTEXT_DIR_SYSTEM: Option<&str> = option_env!("STREMIO_LOCALEDIR");
pub const GETTEXT_DIR_FLATPAK: &str = "/app/share/locale";

pub const STARTUP_URL: &str = "http://127.0.0.1:11470/proxy/d=https%3A%2F%2Fweb.stremio.com/";
pub const IPC_KEY: &str = "LINUX";
