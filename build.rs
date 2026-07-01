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

    let out_dir = dirs::data_dir().expect("Failed to get data dir");
    let out_dir = out_dir.join("glib-2.0/schemas");

    fs::create_dir_all(&out_dir)?;

    let target = Path::new(DATA_DIR).join(filename);
    fs::copy(target, out_dir.join(filename))?;

    let mut command = Command::new("glib-compile-schemas");

    let output = command.arg(out_dir).output()?;

    assert!(
        output.status.success(),
        "glib-compile-schemas failed with exit status {} and stderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );

    Ok(())
}
