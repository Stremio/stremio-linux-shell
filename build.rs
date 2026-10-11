use std::{fs, path::Path, process::Command};

use anyhow::Result;

const APP_ID: &str = "com.stremio.Stremio";
const GETTEXT_DOMAIN: &str = "stremio";
const GETTEXT_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/po");
const DATA_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/data");

fn main() -> Result<()> {
    setup_po()?;
    setup_schemas()?;

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
