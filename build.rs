use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Error, Result};

fn main() -> Result<()> {
    println!("cargo:rerun-if-env-changed=CEF_PATH");

    // $ORIGIN--------: finds libcef.so when binary and CEF are in the same directory (installed flat)
    // $ORIGIN/bundle-: finds libcef.so inside the bundle/ staging directory produced by this build
    println!("cargo:rustc-link-arg=-Wl,-rpath,$ORIGIN");
    println!("cargo:rustc-link-arg=-Wl,-rpath,$ORIGIN/bundle");

    #[cfg(not(feature = "offline-build"))]
    copy_cef_to_target()?;

    Ok(())
}

#[cfg(not(feature = "offline-build"))]
fn copy_cef_to_target() -> Result<()> {
    // cef-dll-sys's build script downloads (or locates) CEF and emits its path via
    // cargo::metadata=CEF_DIR. Cargo exposes this to our build script as DEP_CEF_DLL_WRAPPER_CEF_DIR.
    let cef_dir = std::env::var("DEP_CEF_DLL_WRAPPER_CEF_DIR")
        .map(PathBuf::from)
        .map_err(|_| Error::msg("DEP_CEF_DLL_WRAPPER_CEF_DIR not set — ensure cef-dll-sys ran first"))?;

    // OUT_DIR is target/<profile>/build/<crate>-<hash>/out and only three leves up is target/<profile>/.
    let out_dir = PathBuf::from(std::env::var("OUT_DIR")?);
    let target_dir = out_dir
        .ancestors()
        .nth(3)
        .ok_or_else(|| Error::msg("Cannot determine target directory from OUT_DIR"))?;

    let bundle_dir = target_dir.join("bundle");
    fs::create_dir_all(&bundle_dir)?;
    copy_dir(&cef_dir, &bundle_dir)
}

#[cfg(not(feature = "offline-build"))]
fn copy_dir(src: &Path, dest: &Path) -> Result<()> {
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let dest_path = dest.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            fs::create_dir_all(&dest_path)?;
            copy_dir(&entry.path(), &dest_path)?;
        } else {
            fs::copy(entry.path(), &dest_path)?;
        }
    }
    Ok(())
}
