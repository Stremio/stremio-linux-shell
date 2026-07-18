use std::{
    collections::HashSet,
    env,
    fs::Metadata,
    io::{self, Read, Write},
    net::Shutdown,
    os::unix::{fs::MetadataExt, net::UnixStream},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

use serde_json::{Value, json};
use tracing::warn;

const ENV_KEYS: [&str; 4] = ["XDG_RUNTIME_DIR", "TMPDIR", "TMP", "TEMP"];
const APP_SUBPATHS: [&str; 7] = [
    "",
    "app/com.discordapp.Discord/",
    "app/dev.vencord.Vesktop/",
    ".flatpak/com.discordapp.Discord/xdg-run/",
    ".flatpak/dev.vencord.Vesktop/xdg-run/",
    "snap.discord-canary/",
    "snap.discord/",
];
const MAX_FRAME_LENGTH: usize = 1024 * 1024;
const MAX_PENDING_NONCES: usize = 64;
const HANDSHAKE_READ_SLICE: Duration = Duration::from_millis(100);
const LIVENESS_READ_TIMEOUT: Duration = Duration::from_millis(10);
const WRITE_TIMEOUT: Duration = Duration::from_secs(1);

static NEXT_NONCE: AtomicU64 = AtomicU64::new(1);

pub struct DiscordIpcTransport {
    client_id: String,
    connection: Option<Connection>,
    configured_path: Option<PathBuf>,
    handshake_timeout: Duration,
}

struct Connection {
    socket: UnixStream,
    path: PathBuf,
    identity: SocketIdentity,
    received: Vec<u8>,
    pending_nonces: HashSet<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SocketIdentity {
    device: u64,
    inode: u64,
}

impl SocketIdentity {
    fn from_metadata(metadata: &Metadata) -> Self {
        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
        }
    }
}

impl DiscordIpcTransport {
    pub fn new(client_id: &str, handshake_timeout: Duration) -> Self {
        Self {
            client_id: client_id.to_owned(),
            connection: None,
            configured_path: None,
            handshake_timeout,
        }
    }

    #[cfg(test)]
    pub fn with_path(client_id: &str, path: PathBuf, handshake_timeout: Duration) -> Self {
        let mut transport = Self::new(client_id, handshake_timeout);
        transport.configured_path = Some(path);
        transport
    }

    pub fn connect(&mut self) -> Result<(), String> {
        self.connection = None;

        let paths = self
            .configured_path
            .clone()
            .map_or_else(find_pipes, |path| vec![path]);
        if paths.is_empty() {
            return Err("failed to find Discord IPC socket".to_owned());
        }

        let mut last_error = None;
        for path in paths {
            match Connection::connect(path.clone()) {
                Ok(mut connection) => {
                    if let Err(error) =
                        connection.handshake(&self.client_id, self.handshake_timeout)
                    {
                        last_error = Some(format!("{}: {error}", path.display()));
                        continue;
                    }
                    self.connection = Some(connection);
                    return Ok(());
                }
                Err(error) => last_error = Some(format!("{}: {error}", path.display())),
            }
        }

        Err(last_error.unwrap_or_else(|| "failed to connect to Discord IPC".to_owned()))
    }

    pub fn set_activity(&mut self, activity: Value) -> Result<(), String> {
        self.send_command(Some(activity))
    }

    pub fn clear_activity(&mut self) -> Result<(), String> {
        self.send_command(None)
    }

    fn send_command(&mut self, activity: Option<Value>) -> Result<(), String> {
        let connection = self
            .connection
            .as_mut()
            .ok_or_else(|| "Discord IPC is not connected".to_owned())?;
        let nonce = format!(
            "{}-{}",
            std::process::id(),
            NEXT_NONCE.fetch_add(1, Ordering::Relaxed)
        );
        let payload = json!({
            "cmd": "SET_ACTIVITY",
            "args": {
                "pid": std::process::id(),
                "activity": activity,
            },
            "nonce": nonce,
        });

        connection.send(1, &payload)?;
        connection.track_nonce(nonce);
        Ok(())
    }

    pub fn poll_liveness(&mut self) -> Result<(), String> {
        self.connection
            .as_mut()
            .ok_or_else(|| "Discord IPC is not connected".to_owned())?
            .poll_liveness()
    }

    pub fn close(&mut self) -> Result<(), String> {
        let Some(mut connection) = self.connection.take() else {
            return Ok(());
        };

        let _ = connection.send(2, &json!({}));
        match connection.socket.shutdown(Shutdown::Both) {
            Ok(()) => Ok(()),
            Err(error) if is_closed_socket(&error) => Ok(()),
            Err(error) => Err(format!("Discord IPC socket shutdown failed: {error}")),
        }
    }
}

impl Connection {
    /// Tracks responses for draining and correlation, not synchronous command success.
    fn track_nonce(&mut self, nonce: String) {
        if self.pending_nonces.len() >= MAX_PENDING_NONCES {
            self.pending_nonces.clear();
        }
        self.pending_nonces.insert(nonce);
    }

    fn connect(path: PathBuf) -> Result<Self, String> {
        let socket = UnixStream::connect(&path)
            .map_err(|error| format!("failed to connect to socket: {error}"))?;
        socket
            .set_write_timeout(Some(WRITE_TIMEOUT))
            .map_err(|error| format!("failed to set Discord IPC write timeout: {error}"))?;
        let metadata = path
            .metadata()
            .map_err(|error| format!("failed to inspect connected socket: {error}"))?;

        Ok(Self {
            socket,
            path,
            identity: SocketIdentity::from_metadata(&metadata),
            received: Vec::new(),
            pending_nonces: HashSet::new(),
        })
    }

    fn handshake(&mut self, client_id: &str, timeout: Duration) -> Result<(), String> {
        self.send(0, &json!({ "v": 1, "client_id": client_id }))?;
        let deadline = Instant::now() + timeout;

        loop {
            let now = Instant::now();
            if now >= deadline {
                return Err(format!(
                    "Discord IPC handshake timed out after {} ms",
                    timeout.as_millis()
                ));
            }

            self.socket
                .set_read_timeout(Some(HANDSHAKE_READ_SLICE.min(deadline - now)))
                .map_err(|error| format!("failed to set handshake read timeout: {error}"))?;

            match self.read_once() {
                Ok(frames) => {
                    for (opcode, payload) in frames {
                        if opcode == 2 {
                            return Err("Discord closed IPC during handshake".to_owned());
                        }
                        if opcode == 1
                            && payload.get("evt").and_then(Value::as_str) == Some("READY")
                        {
                            self.socket
                                .set_read_timeout(Some(LIVENESS_READ_TIMEOUT))
                                .map_err(|error| {
                                    format!("failed to set liveness read timeout: {error}")
                                })?;
                            return Ok(());
                        }
                    }
                }
                Err(error) if is_transient_read_error(&error) => {}
                Err(error) => return Err(format!("Discord IPC handshake read failed: {error}")),
            }
        }
    }

    fn poll_liveness(&mut self) -> Result<(), String> {
        let metadata = self
            .path
            .metadata()
            .map_err(|error| format!("Discord IPC socket disappeared: {error}"))?;
        if SocketIdentity::from_metadata(&metadata) != self.identity {
            return Err("Discord IPC socket was replaced".to_owned());
        }

        match self.read_once() {
            Ok(frames) => self.process_frames(frames),
            Err(error) if is_transient_read_error(&error) => Ok(()),
            Err(error) => Err(format!("Discord IPC read failed: {error}")),
        }
    }

    fn process_frames(&mut self, frames: Vec<(u32, Value)>) -> Result<(), String> {
        for (opcode, payload) in frames {
            if opcode == 2 {
                return Err("Discord closed the IPC connection".to_owned());
            }
            if let Some(nonce) = payload.get("nonce").and_then(Value::as_str) {
                self.pending_nonces.remove(nonce);
            }
        }
        Ok(())
    }

    fn send(&mut self, opcode: u32, payload: &Value) -> Result<(), String> {
        let data = serde_json::to_vec(payload)
            .map_err(|error| format!("failed to serialize Discord IPC payload: {error}"))?;
        let length =
            u32::try_from(data.len()).map_err(|_| "Discord IPC payload is too large".to_owned())?;
        let mut header = [0_u8; 8];
        header[..4].copy_from_slice(&opcode.to_le_bytes());
        header[4..].copy_from_slice(&length.to_le_bytes());

        self.socket
            .write_all(&header)
            .and_then(|()| self.socket.write_all(&data))
            .map_err(|error| format!("Discord IPC write failed: {error}"))
    }

    fn read_once(&mut self) -> io::Result<Vec<(u32, Value)>> {
        let mut buffer = [0_u8; 8192];
        match self.socket.read(&mut buffer) {
            Ok(0) => {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "Discord IPC EOF",
                ));
            }
            Ok(length) => self.received.extend_from_slice(&buffer[..length]),
            Err(error) => return Err(error),
        }

        let mut frames = Vec::new();
        loop {
            if self.received.len() < 8 {
                break;
            }
            let opcode = u32::from_le_bytes(
                self.received[..4]
                    .try_into()
                    .expect("four-byte opcode slice"),
            );
            let length = u32::from_le_bytes(
                self.received[4..8]
                    .try_into()
                    .expect("four-byte length slice"),
            ) as usize;
            if length > MAX_FRAME_LENGTH {
                warn!("Discord IPC frame exceeds size limit");
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Discord IPC frame exceeds size limit",
                ));
            }
            if self.received.len() < 8 + length {
                break;
            }

            let payload =
                serde_json::from_slice(&self.received[8..8 + length]).map_err(|error| {
                    warn!("Discord IPC frame contains malformed JSON");
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("invalid Discord IPC JSON: {error}"),
                    )
                })?;
            self.received.drain(..8 + length);
            frames.push((opcode, payload));
        }
        Ok(frames)
    }
}

fn is_transient_read_error(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::Interrupted | io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
    )
}

fn is_closed_socket(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::NotConnected
}

fn find_pipes() -> Vec<PathBuf> {
    let snap = env::var_os("SNAP").is_some();
    let mut paths = Vec::new();

    for key in ENV_KEYS {
        let Some(value) = env::var_os(key) else {
            continue;
        };
        let mut base = PathBuf::from(value);
        if snap && key == "XDG_RUNTIME_DIR" {
            base.pop();
        }
        if !base.is_dir() {
            continue;
        }

        for index in 0..10 {
            for subpath in APP_SUBPATHS {
                let path = base.join(subpath).join(format!("discord-ipc-{index}"));
                if Path::new(&path).exists() {
                    paths.push(path);
                }
            }
        }
    }

    paths
}

#[cfg(test)]
mod tests {
    use std::os::unix::net::UnixStream;

    use super::*;

    fn connection() -> (Connection, UnixStream) {
        let (socket, peer) = UnixStream::pair().expect("test socket pair should be created");
        (
            Connection {
                socket,
                path: PathBuf::new(),
                identity: SocketIdentity {
                    device: 0,
                    inode: 0,
                },
                received: Vec::new(),
                pending_nonces: HashSet::new(),
            },
            peer,
        )
    }

    #[test]
    fn interrupted_reads_are_transient() {
        assert!(is_transient_read_error(&io::Error::from(
            io::ErrorKind::Interrupted
        )));
    }

    #[test]
    fn not_connected_shutdown_is_success() {
        assert!(is_closed_socket(&io::Error::from(
            io::ErrorKind::NotConnected
        )));
        assert!(!is_closed_socket(&io::Error::from(
            io::ErrorKind::PermissionDenied
        )));
    }

    #[test]
    fn pending_nonces_never_exceed_the_bound() {
        let (mut connection, _peer) = connection();

        for nonce in 0..(MAX_PENDING_NONCES * 3) {
            connection.track_nonce(nonce.to_string());
            assert!(connection.pending_nonces.len() <= MAX_PENDING_NONCES);
        }
    }

    #[test]
    fn matching_response_removes_pending_nonce() {
        let (mut connection, mut peer) = connection();
        connection.track_nonce("expected".to_owned());
        let payload = serde_json::to_vec(&json!({ "nonce": "expected" })).unwrap();
        peer.write_all(&1_u32.to_le_bytes()).unwrap();
        peer.write_all(&(payload.len() as u32).to_le_bytes())
            .unwrap();
        peer.write_all(&payload).unwrap();

        let frames = connection.read_once().expect("response should be readable");
        connection.process_frames(frames).unwrap();

        assert!(connection.pending_nonces.is_empty());
    }

    #[test]
    fn unknown_response_nonce_is_harmless() {
        let (mut connection, mut peer) = connection();
        connection.track_nonce("expected".to_owned());
        let payload = serde_json::to_vec(&json!({ "nonce": "unknown" })).unwrap();
        peer.write_all(&1_u32.to_le_bytes()).unwrap();
        peer.write_all(&(payload.len() as u32).to_le_bytes())
            .unwrap();
        peer.write_all(&payload).unwrap();

        let frames = connection.read_once().expect("response should be readable");
        connection.process_frames(frames).unwrap();

        assert!(connection.pending_nonces.contains("expected"));
    }

    #[test]
    fn close_opcode_fails_liveness_frame_processing() {
        let (mut connection, _peer) = connection();

        let error = connection
            .process_frames(vec![(2, json!({}))])
            .expect_err("close opcode should fail liveness");

        assert!(error.contains("closed"));
    }

    #[test]
    fn oversized_frame_is_rejected() {
        let (mut connection, mut peer) = connection();
        peer.write_all(&1_u32.to_le_bytes()).unwrap();
        peer.write_all(&((MAX_FRAME_LENGTH + 1) as u32).to_le_bytes())
            .unwrap();

        let error = connection
            .read_once()
            .expect_err("oversized frame should fail");

        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn malformed_json_frame_is_rejected() {
        let (mut connection, mut peer) = connection();
        let payload = b"{";
        peer.write_all(&1_u32.to_le_bytes()).unwrap();
        peer.write_all(&(payload.len() as u32).to_le_bytes())
            .unwrap();
        peer.write_all(payload).unwrap();

        let error = connection
            .read_once()
            .expect_err("malformed JSON should fail");

        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn partial_header_followed_by_eof_is_rejected() {
        let (mut connection, mut peer) = connection();
        peer.write_all(&1_u32.to_le_bytes()).unwrap();

        assert!(connection.read_once().unwrap().is_empty());
        peer.shutdown(Shutdown::Write).unwrap();

        let error = connection
            .read_once()
            .expect_err("EOF after partial header should fail");
        assert_eq!(error.kind(), io::ErrorKind::UnexpectedEof);
    }
}
