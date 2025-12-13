use std::{
    io::{BufRead, BufReader},
    process::{self, Child, Command},
    thread,
};

use anyhow::{Context, Ok};
use tracing::debug;

use crate::config::ServerConfig;

pub struct Server {
    config: ServerConfig,
    process: Option<Child>,
}

impl Server {
    pub fn new(config: ServerConfig) -> Self {
        Self {
            config,
            process: None,
        }
    }

    pub fn start(&mut self, dev: bool) -> anyhow::Result<()> {
        if std::net::TcpStream::connect("127.0.0.1:11470").is_ok() {
            debug!(target: "server", "Server already running on port 11470, skipping spawn");
            return Ok(());
        }

        use std::os::unix::process::CommandExt;
        let mut child = unsafe {
            Command::new("node")
                .env("NO_CORS", (dev as i32).to_string())
                .arg(self.config.file.as_os_str())
                .stdout(process::Stdio::piped())
                .pre_exec(|| {
                    libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM);
                    Result::Ok(())
                })
                .spawn()
                .context("Failed to start server")?
        };

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
