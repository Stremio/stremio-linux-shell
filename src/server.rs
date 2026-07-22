use std::{
    env,
    io::{BufRead, BufReader},
    os::unix::process::CommandExt,
    path::PathBuf,
    process::{self, Child, Command},
    thread,
};

use anyhow::Context;
use tracing::debug;

use crate::config::IPC_KEY;

pub struct Server {
    process: Option<Child>,
    file: PathBuf,
}

impl Server {
    pub fn new() -> Self {
        let server_path = env::var("SERVER_PATH").expect("Failed to read SERVER_PATH env");
        let file = PathBuf::from(&server_path);

        Self {
            process: None,
            file,
        }
    }

    pub fn start(&mut self, dev: bool) -> anyhow::Result<()> {
        let mut command = Command::new("node");
        command
            .env("NO_CORS", (dev as i32).to_string())
            .env("SERVER_IPC_KEY", IPC_KEY)
            .arg(self.file.as_os_str())
            .stdout(process::Stdio::piped())
            .process_group(0);

        unsafe {
            command.pre_exec(move || {
                libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM);
                Ok(())
            });
        }

        let mut child = command.spawn()?;

        if let Some(stdout) = child.stdout.take() {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();

            thread::spawn(move || {
                while let Some(Ok(line)) = lines.next() {
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
