mod config;

use std::{
    fs,
    io::{BufRead, BufReader},
    path::PathBuf,
    process::{self, Child, Command},
    thread,
};

use anyhow::{Context, Ok};
use serde::Deserialize;
use tracing::debug;
use url::Url;

use crate::{
    config::DATA_DIR,
    server::config::{DOWNLOAD_ENDPOINT, FILE, UPDATER_ENDPOINT, VERSION_FILE},
};

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct ServerUpdaterResponse {
    latest_version: String,
}

pub struct Server {
    process: Option<Child>,
    file: PathBuf,
    version_file: PathBuf,
}

impl Server {
    pub fn new() -> Self {
        let data_dir = dirs::data_dir()
            .expect("Failed to get data dir")
            .join(DATA_DIR);

        fs::create_dir_all(&data_dir).expect("Failed to create data directory");

        let file = data_dir.join(FILE);
        let version_file = data_dir.join(VERSION_FILE);

        Self {
            process: None,
            file,
            version_file,
        }
    }

    pub async fn setup(&self) -> anyhow::Result<()> {
        let response = reqwest::get(UPDATER_ENDPOINT)
            .await
            .context("Failed to send request to updater endpoint")?;

        let latest_version = response
            .json::<ServerUpdaterResponse>()
            .await
            .context("Failed to parse updater response JSON")?
            .latest_version;

        let current_version = fs::read_to_string(&self.version_file).ok();
        let should_download = current_version.as_deref() != Some(latest_version.as_str());

        if should_download {
            let download_url = Url::parse(&DOWNLOAD_ENDPOINT.replace("VERSION", &latest_version))
                .context("Failed to construct download URL")?;

            let file_response = reqwest::get(download_url)
                .await
                .context("Failed to send request for server file")?;

            let latest_file = file_response
                .bytes()
                .await
                .context("Failed to read server file response body")?;

            fs::write(&self.file, &latest_file).with_context(|| {
                format!("Failed to write server file to {}", self.file.display())
            })?;

            fs::write(&self.version_file, &latest_version).with_context(|| {
                format!(
                    "Failed to write version file to {}",
                    self.version_file.display()
                )
            })?;
        }

        Ok(())
    }

    pub fn start(&mut self, dev: bool) -> anyhow::Result<()> {
        let mut child = Command::new("node")
            .env("NO_CORS", (dev as i32).to_string())
            .arg(self.file.as_os_str())
            .stdout(process::Stdio::piped())
            .spawn()
            .context("Failed to start server")?;

        if let Some(stdout) = child.stdout.take() {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();

            thread::spawn(move || {
                while let Some(Result::Ok(line)) = lines.next() {
                    debug!(target: "server", "{}", line);
                }
            });
        }

        self.process = Some(child);

        Ok(())
    }

    pub fn stop(&mut self) -> anyhow::Result<()> {
        if let Some(mut process) = self.process.take() {
            process.kill().context("Failed to kill server process")?;
        }

        Ok(())
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop().expect("Failed to stop server");
    }
}
