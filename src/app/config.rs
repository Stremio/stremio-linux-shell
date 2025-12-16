pub const APP_ID: &str = match cfg!(debug_assertions) {
    true => "com.stremio.Stremio.Devel",
    false => "com.stremio.Stremio",
};
pub const APP_NAME: &str = "Stremio";

pub const URI_SCHEME: &str = "stremio://";
