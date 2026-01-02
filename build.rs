use std::{
    fs::{self, File},
    io::BufReader,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Error, Result};
use bzip2::bufread::BzDecoder;
use globset::GlobBuilder;
use serde::{Deserialize, Serialize};
use tar::Archive;
use toml::Value;

fn main() -> Result<()> {
    setup_cef()?;
    setup_po()?;

    Ok(())
}

pub const GETTEXT_DOMAIN: &str = "stremio";
pub const GETTEXT_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/po");

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

const CEF_CDN: &str = "https://cef-builds.spotifycdn.com";
const CEF_CDN_INDEX: &str = "index.json";
const CEF_FILE_TYPE: &str = "minimal";
const CEF_ARCHIVE_FILES: &[[&str; 2]] = &[
    ["*/Resources/locales/**", "locales"],
    ["*/Resources/*.pak", ""],
    ["*/Resources/icudtl.dat", ""],
    ["*/Release/libcef.so", ""],
    ["*/Release/libEGL.so", ""],
    ["*/Release/libGLESv2.so", ""],
    ["*/Release/libvk_swiftshader.so", ""],
    ["*/Release/v8_context_snapshot.bin", ""],
];

#[derive(Deserialize, Serialize, Debug)]
pub struct CefFile {
    #[serde(rename = "type")]
    pub file_type: String,
    pub name: String,
    pub sha1: String,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct CefVersion {
    pub cef_version: String,
    pub files: Vec<CefFile>,
}

#[derive(Deserialize, Serialize, Default, Debug)]
pub struct CefPlatform {
    pub versions: Vec<CefVersion>,
}

#[derive(Deserialize, Serialize, Default, Debug)]
pub struct CefIndex {
    pub linux64: CefPlatform,
}

fn setup_cef() -> Result<()> {
    let cef_path = PathBuf::from(std::env::var("CEF_PATH")?);

    #[cfg(not(feature = "offline-build"))]
    if !cef_path.exists() {
        let cef_version = get_version()?;
        let archive_name = get_archive_name(cef_version)?;
        let archive_url = format!("{CEF_CDN}/{archive_name}");
        let archive_path = cef_path.join(archive_name);

        fs::create_dir_all(&cef_path)?;
        download_archive(&archive_url, &archive_path)?;
        unpack_archive(&archive_path, &cef_path)?;
        fs::remove_file(&archive_path)?;
    }

    println!(
        "cargo:rustc-env=LD_LIBRARY_PATH={}",
        cef_path.to_str().unwrap()
    );

    Ok(())
}

fn get_version() -> Result<String> {
    let cargo_toml = fs::read_to_string("Cargo.toml")?;
    let value: Value = toml::from_str(&cargo_toml)?;

    value
        .get("dependencies")
        .and_then(|deps| deps.get("cef"))
        .and_then(|dep| {
            if let Value::Table(table) = dep {
                table.get("version")
            } else {
                Some(dep)
            }
        })
        .and_then(|v| v.as_str())
        .and_then(|s| s.split("+").last())
        .map(|s| s.to_string())
        .ok_or(Error::msg("Failed to get cef version"))
}

fn get_archive_name(cef_version: String) -> Result<String> {
    println!("Fetch archive name...");

    let index_url = format!("{CEF_CDN}/{CEF_CDN_INDEX}");
    let index = ureq::get(index_url)
        .call()?
        .into_body()
        .read_json::<CefIndex>()?;

    let version = index
        .linux64
        .versions
        .iter()
        .find(|version| version.cef_version.starts_with(&cef_version))
        .ok_or(Error::msg("Failed to find version"))?;

    let file = version
        .files
        .iter()
        .find(|file| file.file_type == CEF_FILE_TYPE)
        .ok_or(Error::msg("Failed to find file"))?;

    Ok(file.name.to_owned())
}

fn download_archive(url: &str, out: &Path) -> Result<()> {
    println!("Downloading archive...");

    if !out.exists() {
        let resp = ureq::get(url).call()?;
        let mut file = File::create(out)?;

        std::io::copy(&mut resp.into_body().into_reader(), &mut file)?;
    }

    Ok(())
}

fn unpack_archive(path: &Path, out: &Path) -> Result<()> {
    println!("Unpacking archive...");

    if path.exists() {
        let decoder = BzDecoder::new(BufReader::new(File::open(path)?));
        let mut archive = Archive::new(decoder);

        for entry in archive.entries()? {
            let mut entry = entry?;
            let file_path = entry.path()?.into_owned();

            if let Some(file_name) = file_path.file_name() {
                for archive_file in CEF_ARCHIVE_FILES {
                    let glob = GlobBuilder::new(archive_file[0])
                        .literal_separator(true)
                        .build()?
                        .compile_matcher();

                    if glob.is_match(&file_path) {
                        let dest_path = out.join(archive_file[1]);
                        let dest_file_path = dest_path.join(file_name);
                        println!("Writting {file_path:?} to {dest_file_path:?}");

                        fs::create_dir_all(&dest_path)?;
                        entry.unpack(dest_file_path)?;
                    }
                }
            }
        }
    }

    Ok(())
}
