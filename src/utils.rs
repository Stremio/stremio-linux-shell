use std::{
    ffi::{CStr, c_char},
    io::{self, Cursor},
    sync::LazyLock,
};

use anyhow::{Context, Error, Result, anyhow};
use base64::{engine::general_purpose, read};
use gtk::{gdk, prelude::*};
use tokio::fs;

#[macro_export]
macro_rules! spawn_local {
    ($body:expr) => {
        glib::MainContext::default().spawn_local($body)
    };
}

pub static IS_DESKTOP_KDE: LazyLock<bool> = LazyLock::new(|| {
    std::env::var("XDG_CURRENT_DESKTOP")
        .ok()
        .is_some_and(|value| value == "KDE")
});

/// Whether GTK renders with the NVIDIA proprietary OpenGL driver.
///
/// Must be called from the main thread after GTK has been initialized.
pub fn is_nvidia_gl_driver() -> bool {
    let Some(display) = gdk::Display::default() else {
        return false;
    };

    let Ok(context) = display.create_gl_context() else {
        return false;
    };

    if context.realize().is_err() {
        return false;
    }

    context.make_current();

    let vendor = unsafe { epoxy::GetString(epoxy::VENDOR) };
    let is_nvidia = !vendor.is_null()
        && unsafe { CStr::from_ptr(vendor as *const c_char) }
            .to_string_lossy()
            .contains("NVIDIA");

    gdk::GLContext::clear_current();

    is_nvidia
}

pub fn decode_base64(data: &str) -> Result<Vec<u8>> {
    let mut input = Cursor::new(data);
    let mut output = Vec::new();

    let engine = general_purpose::STANDARD;
    let mut decoder = read::DecoderReader::new(&mut input, &engine);

    if let Err(e) = io::copy(&mut decoder, &mut output) {
        return Err(Error::msg(format!("Failed to decode base64: {e}")));
    }

    Ok(output)
}

pub async fn download_file(file_name: &str, data: String) -> Result<String> {
    let base_64 = data
        .strip_prefix("application/octet-stream;charset=utf-8;base64,")
        .ok_or_else(|| anyhow!("Failed to parse data URL"))?;
    let bytes = decode_base64(base_64)?;

    let out_dir = dirs::download_dir()
        .ok_or_else(|| anyhow!("Failed to get download dir"))?
        .join("Stremio");

    fs::create_dir_all(&out_dir)
        .await
        .context("Failed to create download dir")?;

    let file_name = out_dir.join(file_name);
    fs::write(&file_name, bytes)
        .await
        .context("Failed to write file to download dir")?;

    file_name
        .into_os_string()
        .into_string()
        .map_err(|os_str| anyhow!("Failed to convert path to string: {:?}", os_str))
}
