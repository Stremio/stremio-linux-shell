use std::{fs, path::Path, process::Command};

use anyhow::Result;

pub const GETTEXT_DOMAIN: &str = "stremio";
pub const GETTEXT_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/po");
pub const DATA_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/data");

fn main() -> Result<()> {
    setup_po()?;
    setup_schemas("com.stremio.Stremio.gschema.xml")?;

    Ok(())
}

fn setup_po() -> Result<()> {
    println!("cargo:rerun-if-changed={GETTEXT_DIR}");

    let po_dir = Path::new(GETTEXT_DIR);

    for entry in fs::read_dir(po_dir)? {
        let entry = entry?;
        let path = entry.path();

        if let Some(extension) = path.extension()
            && extension == "po"
            && let Some(po_lang) = path.file_stem()
        {
            let mo_dir = po_dir.join(po_lang).join("LC_MESSAGES");

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

fn setup_schemas(filename: &str) -> Result<()> {
    println!("cargo:rerun-if-changed={DATA_DIR}");

    let out_dir = Path::new(env!("GSETTINGS_SCHEMA_DIR"));
    fs::create_dir_all(&out_dir)?;

    let source_file = Path::new(DATA_DIR).join(filename);
    fs::copy(source_file, out_dir.join(filename))?;

    let output = Command::new("glib-compile-schemas")
        .arg(&out_dir)
        .output()?;

    assert!(
        output.status.success(),
        "Failed to compile schemas: {}\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );

    Ok(())
}
