use std::{fs, path::Path, process::Command};

use anyhow::{Context, Result, bail};

const APP_ID: &str = "com.stremio.Stremio";
const GETTEXT_DOMAIN: &str = "stremio";
const GETTEXT_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/po");
const DATA_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/data");
const MANIFEST_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml");
const SERVER_URL: &str = "https://dl.strem.io/server";

fn main() -> Result<()> {
    setup_po()?;
    setup_schemas()?;
    setup_server()?;

    Ok(())
}

fn setup_po() -> Result<()> {
    println!("cargo:rerun-if-changed={GETTEXT_DIR}");

    let po_dir = Path::new(GETTEXT_DIR);
    let mo_dir = Path::new(env!("GETTEXT_DIR"));

    for entry in fs::read_dir(po_dir)? {
        let entry = entry?;
        let path = entry.path();

        if let Some(extension) = path.extension()
            && extension == "po"
            && let Some(po_lang) = path.file_stem()
        {
            let mo_dir = mo_dir.join(po_lang).join("LC_MESSAGES");

            fs::create_dir_all(&mo_dir)?;

            let mo_path = mo_dir.join(format!("{GETTEXT_DOMAIN}.mo"));

            Command::new("msgfmt")
                .arg("-o")
                .arg(mo_path)
                .arg(path)
                .spawn()?;
        }
    }

    Ok(())
}

fn setup_schemas() -> Result<()> {
    println!("cargo:rerun-if-changed={DATA_DIR}");

    let out_dir = Path::new(env!("GSETTINGS_SCHEMA_DIR"));
    fs::create_dir_all(out_dir)?;

    let source_filename = &format!("{APP_ID}.gschema.xml");
    let source_file = Path::new(DATA_DIR).join(source_filename);

    let output_app_id = match cfg!(debug_assertions) {
        true => &format!("{APP_ID}.Devel"),
        false => APP_ID,
    };

    let output_filename = &format!("{output_app_id}.gschema.xml");
    let output_file = out_dir.join(output_filename);

    fs::copy(source_file, &output_file)?;

    if cfg!(debug_assertions) {
        let content = fs::read_to_string(&output_file)?;
        let new_content = content.replace(APP_ID, output_app_id);

        fs::write(output_file, new_content)?;
    }

    let output = Command::new("glib-compile-schemas").arg(out_dir).output()?;

    assert!(
        output.status.success(),
        "Failed to compile schemas: {}\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );

    Ok(())
}

fn setup_server() -> Result<()> {
    println!("cargo:rerun-if-changed={MANIFEST_PATH}");

    if cfg!(feature = "offline-build") {
        return Ok(());
    }

    let server_file = Path::new(env!("SERVER_PATH"));
    let version_file = server_file.with_extension("version");
    println!("cargo:rerun-if-changed={}", server_file.display());

    if let Some(server_dir) = server_file.parent() {
        fs::create_dir_all(server_dir)?;
    }

    let manifest: toml::Table = fs::read_to_string(MANIFEST_PATH)?.parse()?;
    let version = manifest
        .get("package")
        .and_then(|package| package.get("metadata"))
        .and_then(|metadata| metadata.get("server"))
        .and_then(|server| server.get("version"))
        .and_then(|version| version.as_str())
        .context("Missing package.metadata.server.version in Cargo.toml")?;

    let current_version = fs::read_to_string(&version_file).ok();
    if server_file.exists() && current_version.as_deref().map(str::trim) == Some(version) {
        return Ok(());
    }

    fs::remove_file(&version_file).ok();

    let url = format!("{SERVER_URL}/{version}/desktop/server.js");
    let output = Command::new("curl")
        .args([
            "--fail",
            "--silent",
            "--show-error",
            "--location",
            "--output",
        ])
        .arg(server_file)
        .arg(&url)
        .output()
        .context("Failed to run curl")?;

    if !output.status.success() {
        bail!(
            "Failed to download {url}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fs::write(&version_file, version)?;

    Ok(())
}
