use std::{
    sync::mpsc::{Receiver, RecvTimeoutError},
    time::{Duration, Instant},
};

use discord_rich_presence::activity;
use tracing::{debug, warn};

use super::ipc::DiscordIpcTransport;
use crate::app::ipc::event::DiscordActivity;

const DISCORD_APP_ID: &str = "1452620752263319665";
const FALLBACK_LARGE_IMAGE: &str = "stremio_logo";
const LARGE_IMAGE_TEXT: &str = "Stremio";
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(3);
const LIVENESS_INTERVAL: Duration = Duration::from_secs(1);

/// Maximum length of the user-visible `state` and `details` fields in
/// UTF-16 code units, matching how Discord's JavaScript client measures
/// string length.
const MAX_FIELD_LENGTH: usize = 128;

pub enum DiscordCommand {
    Connect,
    Disconnect,
    SetActivity(DiscordActivity),
    ClearActivity,
}

/// Minimal seam over the Discord IPC client so the command handling
/// state machine can be tested without a running Discord client.
pub trait DiscordClient {
    fn connect(&mut self) -> Result<(), String>;
    fn set_activity(&mut self, activity: &DiscordActivity) -> Result<(), String>;
    fn clear_activity(&mut self) -> Result<(), String>;
    fn poll_liveness(&mut self) -> Result<(), String>;
    fn close(&mut self) -> Result<(), String>;
}

pub struct RichPresenceClient {
    client: DiscordIpcTransport,
}

impl RichPresenceClient {
    pub fn new() -> Self {
        Self {
            client: DiscordIpcTransport::new(DISCORD_APP_ID, HANDSHAKE_TIMEOUT),
        }
    }

    #[cfg(test)]
    fn with_path(path: std::path::PathBuf, handshake_timeout: Duration) -> Self {
        Self {
            client: DiscordIpcTransport::with_path(DISCORD_APP_ID, path, handshake_timeout),
        }
    }
}

impl DiscordClient for RichPresenceClient {
    fn connect(&mut self) -> Result<(), String> {
        self.client.connect()
    }

    fn set_activity(&mut self, activity: &DiscordActivity) -> Result<(), String> {
        let payload = serde_json::to_value(build_activity(activity))
            .map_err(|error| format!("failed to serialize Discord activity: {error}"))?;
        self.client.set_activity(payload)
    }

    fn clear_activity(&mut self) -> Result<(), String> {
        self.client.clear_activity()
    }

    fn poll_liveness(&mut self) -> Result<(), String> {
        self.client.poll_liveness()
    }

    fn close(&mut self) -> Result<(), String> {
        self.client.close()
    }
}

/// Runs the Discord command loop until all command senders are dropped,
/// then closes the connection. Blocking: must run on a dedicated thread.
pub fn run<C, S, F>(receiver: Receiver<DiscordCommand>, status: S, make_client: F)
where
    C: DiscordClient,
    S: FnMut(bool),
    F: Fn() -> C,
{
    run_with_liveness_interval(receiver, status, make_client, LIVENESS_INTERVAL);
}

fn run_with_liveness_interval<C, S, F>(
    receiver: Receiver<DiscordCommand>,
    mut status: S,
    make_client: F,
    liveness_interval: Duration,
) where
    C: DiscordClient,
    S: FnMut(bool),
    F: Fn() -> C,
{
    debug!("Discord worker thread started");

    let mut client: Option<C> = None;
    let mut cached_activity: Option<DiscordActivity> = None;
    let mut next_liveness_check = Instant::now() + liveness_interval;

    loop {
        let wait = next_liveness_check.saturating_duration_since(Instant::now());
        match receiver.recv_timeout(wait) {
            Ok(command) => handle_command(
                &mut client,
                &mut cached_activity,
                &make_client,
                command,
                &mut status,
            ),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }

        if Instant::now() >= next_liveness_check {
            next_liveness_check = Instant::now() + liveness_interval;
            if let Some(current_client) = client.as_mut()
                && let Err(e) = current_client.poll_liveness()
            {
                debug!("Discord transport lost; liveness failure dropped client: {e}");
                client = None;
                status(false);
            }
        }
    }

    if let Some(mut current_client) = client {
        let _ = current_client.close();
    }

    debug!("Discord worker thread exiting");
}

fn handle_command<C, F, S>(
    client: &mut Option<C>,
    cached_activity: &mut Option<DiscordActivity>,
    make_client: &F,
    command: DiscordCommand,
    status: &mut S,
) where
    C: DiscordClient,
    F: Fn() -> C,
    S: FnMut(bool) + ?Sized,
{
    match command {
        DiscordCommand::Connect => {
            if client.is_some() {
                status(true);
                return;
            }

            let mut next_client = make_client();
            match next_client.connect() {
                Ok(()) => {
                    debug!("Discord connect succeeded");
                    *client = Some(next_client);
                    status(true);

                    if let Some(activity) = cached_activity.as_ref() {
                        debug!("Discord cached activity replay started");
                        if let Err(e) = client
                            .as_mut()
                            .expect("client was just connected")
                            .set_activity(activity)
                        {
                            warn!("Discord cached activity replay failed: {e}");
                            *client = None;
                            status(false);
                        } else {
                            debug!("Discord cached activity replay succeeded");
                        }
                    }
                }
                Err(e) => {
                    debug!("Discord connect failed: {e}");
                    status(false);
                }
            }
        }
        DiscordCommand::Disconnect => {
            *cached_activity = None;

            if let Some(mut current_client) = client.take()
                && let Err(e) = current_client.close()
            {
                debug!("Discord transport unavailable during disconnect: {e}");
            }

            status(false);
        }
        DiscordCommand::SetActivity(activity) => {
            *cached_activity = Some(activity);

            let Some(current_client) = client.as_mut() else {
                return;
            };

            if let Err(e) = current_client
                .set_activity(cached_activity.as_ref().expect("activity was just cached"))
            {
                warn!("Failed to set Discord activity after transport loss: {e}");
                *client = None;
                status(false);
            }
        }
        DiscordCommand::ClearActivity => {
            *cached_activity = None;

            let Some(current_client) = client.as_mut() else {
                return;
            };

            if let Err(e) = current_client.clear_activity() {
                warn!("Failed to clear Discord activity after transport loss: {e}");
                *client = None;
                status(false);
            }
        }
    }
}

/// Builds the Discord rich-presence payload for a web-UI activity,
/// clamping the user-visible fields to Discord's length limit.
fn build_activity(activity: &DiscordActivity) -> activity::Activity<'_> {
    let state = clamp_field(&activity.state);
    let details = clamp_field(&activity.details);

    let mut payload = activity::Activity::new()
        .activity_type(activity::ActivityType::Watching)
        .assets(
            activity::Assets::new()
                .large_image(activity.image.as_deref().unwrap_or(FALLBACK_LARGE_IMAGE))
                .large_text(LARGE_IMAGE_TEXT),
        );

    // Omit empty optional fields instead of sending empty strings.
    if !state.is_empty() {
        payload = payload.state(state);
    }
    if !details.is_empty() {
        payload = payload.details(details);
    }

    let timestamps = match (activity.start_timestamp, activity.end_timestamp) {
        (Some(start), Some(end)) => Some(activity::Timestamps::new().start(start).end(end)),
        (Some(start), None) => Some(activity::Timestamps::new().start(start)),
        (None, Some(end)) => Some(activity::Timestamps::new().end(end)),
        (None, None) => None,
    };
    if let Some(timestamps) = timestamps {
        payload = payload.timestamps(timestamps);
    }

    payload
}

/// Limits a user-visible activity field to Discord's length limit, measured
/// in UTF-16 code units (as Discord's JavaScript client counts), keeping as
/// much of the input as fits without splitting a Unicode scalar value.
fn clamp_field(value: &str) -> String {
    let mut units = 0;
    let mut end = value.len();

    for (index, character) in value.char_indices() {
        units += character.len_utf16();
        if units > MAX_FIELD_LENGTH {
            end = index;
            break;
        }
    }

    value[..end].to_string()
}

#[cfg(test)]
mod tests {
    use std::{
        cell::Cell,
        fs,
        io::{Read, Write},
        os::unix::net::{UnixListener, UnixStream},
        path::PathBuf,
        sync::{
            Arc, Mutex,
            atomic::{AtomicU64, AtomicUsize, Ordering},
            mpsc,
        },
        time::{Duration, Instant},
    };

    use super::*;

    fn activity() -> DiscordActivity {
        DiscordActivity {
            state: "Watching".to_owned(),
            details: "Movie".to_owned(),
            image: Some("https://example.com/poster.jpg".to_owned()),
            start_timestamp: Some(1_752_700_000),
            end_timestamp: Some(1_752_707_200),
        }
    }

    #[derive(Default)]
    struct MockClient {
        closed: usize,
        activities: usize,
        cleared: usize,
        fail_connect: bool,
        fail_set_activity: bool,
        fail_clear_activity: bool,
        fail_liveness: bool,
    }

    impl DiscordClient for MockClient {
        fn connect(&mut self) -> Result<(), String> {
            if self.fail_connect {
                return Err("connect failed".to_owned());
            }

            Ok(())
        }

        fn set_activity(&mut self, _activity: &DiscordActivity) -> Result<(), String> {
            if self.fail_set_activity {
                return Err("set activity failed".to_owned());
            }

            self.activities += 1;
            Ok(())
        }

        fn clear_activity(&mut self) -> Result<(), String> {
            if self.fail_clear_activity {
                return Err("clear activity failed".to_owned());
            }

            self.cleared += 1;
            Ok(())
        }

        fn poll_liveness(&mut self) -> Result<(), String> {
            if self.fail_liveness {
                return Err("connection reset".to_owned());
            }

            Ok(())
        }

        fn close(&mut self) -> Result<(), String> {
            self.closed += 1;
            Ok(())
        }
    }

    fn make_client() -> MockClient {
        MockClient::default()
    }

    fn make_failing_client() -> MockClient {
        MockClient {
            fail_connect: true,
            ..Default::default()
        }
    }

    #[test]
    fn connect_success_reports_connected() {
        let mut client: Option<MockClient> = None;
        let mut cached_activity = None;
        let mut statuses = vec![];

        handle_command(
            &mut client,
            &mut cached_activity,
            &make_client,
            DiscordCommand::Connect,
            &mut |c| statuses.push(c),
        );

        assert_eq!(statuses, [true]);
        assert!(client.is_some());
    }

    #[test]
    fn connect_is_idempotent() {
        let mut client: Option<MockClient> = None;
        let mut cached_activity = None;
        let constructed = Cell::new(0);
        let make_counting_client = || {
            constructed.set(constructed.get() + 1);
            MockClient::default()
        };
        let mut statuses = vec![];

        handle_command(
            &mut client,
            &mut cached_activity,
            &make_counting_client,
            DiscordCommand::Connect,
            &mut |c| statuses.push(c),
        );
        handle_command(
            &mut client,
            &mut cached_activity,
            &make_counting_client,
            DiscordCommand::Connect,
            &mut |c| statuses.push(c),
        );

        assert_eq!(statuses, [true, true]);
        assert_eq!(constructed.get(), 1);
    }

    #[test]
    fn connect_failure_reports_disconnected() {
        let mut client: Option<MockClient> = None;
        let mut cached_activity = None;
        let mut statuses = vec![];

        handle_command(
            &mut client,
            &mut cached_activity,
            &make_failing_client,
            DiscordCommand::Connect,
            &mut |c| statuses.push(c),
        );

        assert_eq!(statuses, [false]);
        assert!(client.is_none());
    }

    #[test]
    fn disconnect_closes_client_and_reports_disconnected() {
        let mut client: Option<MockClient> = None;
        let mut cached_activity = None;
        let mut statuses = vec![];

        handle_command(
            &mut client,
            &mut cached_activity,
            &make_client,
            DiscordCommand::Connect,
            &mut |c| statuses.push(c),
        );
        handle_command(
            &mut client,
            &mut cached_activity,
            &make_client,
            DiscordCommand::Disconnect,
            &mut |c| statuses.push(c),
        );

        assert_eq!(statuses, [true, false]);
        assert!(client.is_none());
    }

    #[test]
    fn disconnect_without_client_still_reports_disconnected() {
        let mut client: Option<MockClient> = None;
        let mut cached_activity = None;
        let mut statuses = vec![];

        handle_command(
            &mut client,
            &mut cached_activity,
            &make_client,
            DiscordCommand::Disconnect,
            &mut |c| statuses.push(c),
        );

        assert_eq!(statuses, [false]);
    }

    #[test]
    fn set_activity_while_disconnected_is_replayed_on_connect() {
        let mut client: Option<MockClient> = None;
        let mut cached_activity = None;
        let mut statuses = vec![];

        handle_command(
            &mut client,
            &mut cached_activity,
            &make_client,
            DiscordCommand::SetActivity(activity()),
            &mut |c| statuses.push(c),
        );
        handle_command(
            &mut client,
            &mut cached_activity,
            &make_client,
            DiscordCommand::Connect,
            &mut |c| statuses.push(c),
        );

        assert_eq!(statuses, [true]);
        assert_eq!(cached_activity, Some(activity()));
        assert_eq!(
            client
                .as_ref()
                .expect("client should be connected")
                .activities,
            1
        );
    }

    #[test]
    fn set_activity_is_forwarded_to_client() {
        let mut client: Option<MockClient> = None;
        let mut cached_activity = None;
        let mut statuses = vec![];

        handle_command(
            &mut client,
            &mut cached_activity,
            &make_client,
            DiscordCommand::Connect,
            &mut |c| statuses.push(c),
        );
        handle_command(
            &mut client,
            &mut cached_activity,
            &make_client,
            DiscordCommand::SetActivity(activity()),
            &mut |c| statuses.push(c),
        );

        assert_eq!(statuses, [true]);
        assert_eq!(
            client
                .as_ref()
                .expect("client should be connected")
                .activities,
            1
        );
    }

    #[test]
    fn clear_activity_is_forwarded_to_client() {
        let mut client: Option<MockClient> = None;
        let mut cached_activity = None;
        let mut statuses = vec![];

        handle_command(
            &mut client,
            &mut cached_activity,
            &make_client,
            DiscordCommand::Connect,
            &mut |c| statuses.push(c),
        );
        handle_command(
            &mut client,
            &mut cached_activity,
            &make_client,
            DiscordCommand::ClearActivity,
            &mut |c| statuses.push(c),
        );

        assert_eq!(statuses, [true]);
        assert_eq!(
            client.as_ref().expect("client should be connected").cleared,
            1
        );
    }

    #[test]
    fn set_activity_failure_drops_client_and_reports_disconnected() {
        let mut client: Option<MockClient> = None;
        let mut cached_activity = None;
        let mut statuses = vec![];

        handle_command(
            &mut client,
            &mut cached_activity,
            &make_client,
            DiscordCommand::Connect,
            &mut |c| statuses.push(c),
        );
        client
            .as_mut()
            .expect("client should be connected")
            .fail_set_activity = true;
        handle_command(
            &mut client,
            &mut cached_activity,
            &make_client,
            DiscordCommand::SetActivity(activity()),
            &mut |c| statuses.push(c),
        );

        assert_eq!(statuses, [true, false]);
        assert!(client.is_none());
        assert_eq!(cached_activity, Some(activity()));
    }

    #[test]
    fn transport_failure_preserves_activity_for_reconnect() {
        let mut client: Option<MockClient> = None;
        let mut cached_activity = None;
        let mut statuses = vec![];

        handle_command(
            &mut client,
            &mut cached_activity,
            &make_client,
            DiscordCommand::Connect,
            &mut |c| statuses.push(c),
        );
        handle_command(
            &mut client,
            &mut cached_activity,
            &make_client,
            DiscordCommand::SetActivity(activity()),
            &mut |c| statuses.push(c),
        );
        client
            .as_mut()
            .expect("client should be connected")
            .fail_set_activity = true;
        handle_command(
            &mut client,
            &mut cached_activity,
            &make_client,
            DiscordCommand::SetActivity(activity()),
            &mut |c| statuses.push(c),
        );
        handle_command(
            &mut client,
            &mut cached_activity,
            &make_client,
            DiscordCommand::Connect,
            &mut |c| statuses.push(c),
        );

        assert_eq!(statuses, [true, false, true]);
        assert_eq!(cached_activity, Some(activity()));
        assert_eq!(
            client.as_ref().expect("client should reconnect").activities,
            1
        );
    }

    #[test]
    fn liveness_failure_reports_disconnected_and_preserves_activity() {
        let (commands, receiver) = mpsc::channel();
        let (status_sender, status_receiver) = mpsc::channel();
        let constructed = Arc::new(AtomicUsize::new(0));
        let client_count = Arc::clone(&constructed);

        let worker = std::thread::spawn(move || {
            run_with_liveness_interval(
                receiver,
                move |connected| {
                    status_sender.send(connected).ok();
                },
                move || MockClient {
                    fail_liveness: client_count.fetch_add(1, Ordering::SeqCst) == 0,
                    ..Default::default()
                },
                Duration::from_millis(10),
            );
        });

        commands.send(DiscordCommand::Connect).unwrap();
        commands
            .send(DiscordCommand::SetActivity(activity()))
            .unwrap();
        assert_eq!(
            status_receiver.recv_timeout(Duration::from_secs(1)),
            Ok(true)
        );
        assert_eq!(
            status_receiver.recv_timeout(Duration::from_secs(1)),
            Ok(false)
        );

        commands.send(DiscordCommand::Connect).unwrap();
        assert_eq!(
            status_receiver.recv_timeout(Duration::from_secs(1)),
            Ok(true)
        );
        drop(commands);
        worker.join().expect("worker thread should terminate");
        assert_eq!(constructed.load(Ordering::SeqCst), 2);
    }

    static NEXT_TEST_SOCKET: AtomicU64 = AtomicU64::new(0);

    struct TestSocketDir(PathBuf);

    impl TestSocketDir {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "stremio-discord-test-{}-{}",
                std::process::id(),
                NEXT_TEST_SOCKET.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).expect("test socket directory should be created");
            Self(path)
        }

        fn socket_path(&self) -> PathBuf {
            self.0.join("discord-ipc-0")
        }
    }

    impl Drop for TestSocketDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn read_frame(stream: &mut UnixStream) -> (u32, serde_json::Value) {
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("server read timeout should be set");
        let mut header = [0_u8; 8];
        stream
            .read_exact(&mut header)
            .expect("server should receive frame header");
        let opcode = u32::from_le_bytes(header[..4].try_into().unwrap());
        let length = u32::from_le_bytes(header[4..].try_into().unwrap()) as usize;
        let mut payload = vec![0_u8; length];
        stream
            .read_exact(&mut payload)
            .expect("server should receive frame payload");
        (
            opcode,
            serde_json::from_slice(&payload).expect("client frame should contain JSON"),
        )
    }

    fn write_frame(stream: &mut UnixStream, opcode: u32, payload: serde_json::Value) {
        let payload = serde_json::to_vec(&payload).expect("server payload should serialize");
        stream
            .write_all(&opcode.to_le_bytes())
            .and_then(|()| stream.write_all(&(payload.len() as u32).to_le_bytes()))
            .and_then(|()| stream.write_all(&payload))
            .expect("server should send frame");
    }

    fn accept_handshake(listener: &UnixListener) -> UnixStream {
        let (mut stream, _) = listener.accept().expect("server should accept client");
        let (opcode, handshake) = read_frame(&mut stream);
        assert_eq!(opcode, 0);
        assert_eq!(handshake["v"], 1);
        assert_eq!(handshake["client_id"], DISCORD_APP_ID);
        write_frame(
            &mut stream,
            1,
            serde_json::json!({ "cmd": "DISPATCH", "evt": "READY", "data": {} }),
        );
        stream
    }

    #[test]
    fn eof_drops_client_and_reconnect_replays_cached_activity() {
        let socket_dir = TestSocketDir::new();
        let socket_path = socket_dir.socket_path();
        let listener = UnixListener::bind(&socket_path).expect("fake Discord should bind");
        let (captured_sender, captured_receiver) = mpsc::channel();
        let (release_sender, release_receiver) = mpsc::channel();

        let server = std::thread::spawn(move || {
            let mut first = accept_handshake(&listener);
            let (opcode, first_activity) = read_frame(&mut first);
            assert_eq!(opcode, 1);
            assert!(first_activity["nonce"].as_str().is_some());
            captured_sender.send(first_activity).unwrap();
            drop(first);

            let mut second = accept_handshake(&listener);
            let (opcode, replayed_activity) = read_frame(&mut second);
            assert_eq!(opcode, 1);
            assert!(replayed_activity["nonce"].as_str().is_some());
            captured_sender.send(replayed_activity).unwrap();
            release_receiver
                .recv_timeout(Duration::from_secs(2))
                .expect("test should release second connection");
        });

        let (commands, receiver) = mpsc::channel();
        let (status_sender, status_receiver) = mpsc::channel();
        let client_path = socket_path.clone();
        let worker = std::thread::spawn(move || {
            run_with_liveness_interval(
                receiver,
                move |connected| {
                    status_sender.send(connected).ok();
                },
                move || {
                    RichPresenceClient::with_path(client_path.clone(), Duration::from_millis(250))
                },
                Duration::from_millis(10),
            );
        });

        commands
            .send(DiscordCommand::SetActivity(activity()))
            .unwrap();
        commands.send(DiscordCommand::Connect).unwrap();
        assert_eq!(
            status_receiver.recv_timeout(Duration::from_secs(1)),
            Ok(true)
        );
        let initially_sent = captured_receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("first activity should arrive");
        assert_eq!(
            status_receiver.recv_timeout(Duration::from_secs(1)),
            Ok(false)
        );

        commands.send(DiscordCommand::Connect).unwrap();
        assert_eq!(
            status_receiver.recv_timeout(Duration::from_secs(1)),
            Ok(true)
        );
        let replayed = captured_receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("cached activity should be replayed");
        assert_eq!(
            initially_sent["args"]["activity"],
            replayed["args"]["activity"]
        );
        assert!(
            status_receiver
                .recv_timeout(Duration::from_millis(50))
                .is_err(),
            "a second liveness monitor must not survive reconnect"
        );

        commands.send(DiscordCommand::Disconnect).unwrap();
        assert_eq!(
            status_receiver.recv_timeout(Duration::from_secs(1)),
            Ok(false)
        );
        drop(commands);
        release_sender.send(()).unwrap();
        worker.join().expect("worker should terminate");
        server.join().expect("fake Discord should terminate");
    }

    #[test]
    fn socket_replacement_is_detected_while_old_stream_remains_open() {
        let socket_dir = TestSocketDir::new();
        let socket_path = socket_dir.socket_path();
        let listener = UnixListener::bind(&socket_path).expect("fake Discord should bind");
        let (connected_sender, connected_receiver) = mpsc::channel();
        let (release_sender, release_receiver) = mpsc::channel();
        let server = std::thread::spawn(move || {
            let _stream = accept_handshake(&listener);
            connected_sender.send(()).unwrap();
            release_receiver
                .recv_timeout(Duration::from_secs(2))
                .expect("test should release stale connection");
        });

        let mut client =
            RichPresenceClient::with_path(socket_path.clone(), Duration::from_millis(250));
        client.connect().expect("client should handshake");
        connected_receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("server should hold old stream open");

        fs::remove_file(&socket_path).expect("old socket path should be removed");
        let replacement = UnixListener::bind(&socket_path).expect("replacement should bind");
        let error = client
            .poll_liveness()
            .expect_err("replaced socket should fail liveness");
        assert!(error.contains("replaced"));

        drop(replacement);
        release_sender.send(()).unwrap();
        server.join().expect("fake Discord should terminate");
    }

    #[test]
    fn worker_shutdown_finishes_after_handshake_timeout() {
        let socket_dir = TestSocketDir::new();
        let socket_path = socket_dir.socket_path();
        let listener = UnixListener::bind(&socket_path).expect("fake Discord should bind");
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("server should accept client");
            let _ = read_frame(&mut stream);
            std::thread::sleep(Duration::from_millis(300));
        });

        let (commands, receiver) = mpsc::channel();
        let path = socket_path.clone();
        let worker = std::thread::spawn(move || {
            run_with_liveness_interval(
                receiver,
                |_| {},
                move || RichPresenceClient::with_path(path.clone(), Duration::from_millis(100)),
                Duration::from_millis(10),
            );
        });

        let started = Instant::now();
        commands.send(DiscordCommand::Connect).unwrap();
        drop(commands);
        worker.join().expect("worker should terminate");
        assert!(started.elapsed() < Duration::from_secs(1));
        server.join().expect("fake Discord should terminate");
    }

    #[test]
    fn clear_activity_failure_drops_client_and_reports_disconnected() {
        let mut client: Option<MockClient> = None;
        let mut cached_activity = None;
        let mut statuses = vec![];

        handle_command(
            &mut client,
            &mut cached_activity,
            &make_client,
            DiscordCommand::Connect,
            &mut |c| statuses.push(c),
        );
        client
            .as_mut()
            .expect("client should be connected")
            .fail_clear_activity = true;
        handle_command(
            &mut client,
            &mut cached_activity,
            &make_client,
            DiscordCommand::ClearActivity,
            &mut |c| statuses.push(c),
        );

        assert_eq!(statuses, [true, false]);
        assert!(client.is_none());
        assert!(cached_activity.is_none());
    }

    #[test]
    fn clear_activity_prevents_replay_after_reconnect() {
        let mut client: Option<MockClient> = None;
        let mut cached_activity = None;
        let mut statuses = vec![];

        handle_command(
            &mut client,
            &mut cached_activity,
            &make_client,
            DiscordCommand::Connect,
            &mut |c| statuses.push(c),
        );
        handle_command(
            &mut client,
            &mut cached_activity,
            &make_client,
            DiscordCommand::SetActivity(activity()),
            &mut |c| statuses.push(c),
        );
        client
            .as_mut()
            .expect("client should be connected")
            .fail_clear_activity = true;
        handle_command(
            &mut client,
            &mut cached_activity,
            &make_client,
            DiscordCommand::ClearActivity,
            &mut |c| statuses.push(c),
        );
        handle_command(
            &mut client,
            &mut cached_activity,
            &make_client,
            DiscordCommand::Connect,
            &mut |c| statuses.push(c),
        );

        assert_eq!(statuses, [true, false, true]);
        assert!(cached_activity.is_none());
        assert_eq!(
            client.as_ref().expect("client should reconnect").activities,
            0
        );
    }

    #[test]
    fn failed_replay_retains_activity_for_later_reconnect() {
        let mut client: Option<MockClient> = None;
        let mut cached_activity = None;
        let constructed = Cell::new(0);
        let make_replay_failing_client = || {
            let fail_set_activity = constructed.get() == 0;
            constructed.set(constructed.get() + 1);
            MockClient {
                fail_set_activity,
                ..Default::default()
            }
        };
        let mut statuses = vec![];

        handle_command(
            &mut client,
            &mut cached_activity,
            &make_replay_failing_client,
            DiscordCommand::SetActivity(activity()),
            &mut |c| statuses.push(c),
        );
        handle_command(
            &mut client,
            &mut cached_activity,
            &make_replay_failing_client,
            DiscordCommand::Connect,
            &mut |c| statuses.push(c),
        );

        assert_eq!(statuses, [true, false]);
        assert!(client.is_none());
        assert_eq!(cached_activity, Some(activity()));

        handle_command(
            &mut client,
            &mut cached_activity,
            &make_replay_failing_client,
            DiscordCommand::Connect,
            &mut |c| statuses.push(c),
        );

        assert_eq!(statuses, [true, false, true]);
        assert_eq!(
            client.as_ref().expect("client should reconnect").activities,
            1
        );
    }

    #[test]
    fn run_closes_client_when_channel_closes() {
        let (commands, receiver) = mpsc::channel();
        let (status_sender, status_receiver) = mpsc::channel();

        let worker = std::thread::spawn(move || {
            run(
                receiver,
                move |connected| {
                    status_sender.send(connected).ok();
                },
                make_client,
            );
        });

        commands.send(DiscordCommand::Connect).ok();
        commands.send(DiscordCommand::SetActivity(activity())).ok();
        drop(commands);

        worker.join().expect("worker thread should terminate");

        let statuses: Vec<bool> = status_receiver.try_iter().collect();
        assert_eq!(statuses, [true]);
    }

    #[test]
    fn command_ordering_is_deterministic() {
        struct RecordingClient(Arc<Mutex<Vec<&'static str>>>);

        impl DiscordClient for RecordingClient {
            fn connect(&mut self) -> Result<(), String> {
                self.0.lock().unwrap().push("connect");
                Ok(())
            }

            fn set_activity(&mut self, _activity: &DiscordActivity) -> Result<(), String> {
                self.0.lock().unwrap().push("set");
                Ok(())
            }

            fn clear_activity(&mut self) -> Result<(), String> {
                self.0.lock().unwrap().push("clear");
                Ok(())
            }

            fn poll_liveness(&mut self) -> Result<(), String> {
                self.0.lock().unwrap().push("poll");
                Ok(())
            }

            fn close(&mut self) -> Result<(), String> {
                self.0.lock().unwrap().push("close");
                Ok(())
            }
        }

        let calls = Arc::new(Mutex::new(Vec::new()));
        let client_calls = Arc::clone(&calls);
        let (commands, receiver) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            run_with_liveness_interval(
                receiver,
                |_| {},
                move || RecordingClient(Arc::clone(&client_calls)),
                Duration::from_secs(1),
            );
        });

        commands.send(DiscordCommand::Connect).unwrap();
        commands
            .send(DiscordCommand::SetActivity(activity()))
            .unwrap();
        commands.send(DiscordCommand::ClearActivity).unwrap();
        commands.send(DiscordCommand::Disconnect).unwrap();
        drop(commands);
        worker.join().expect("worker should terminate");

        assert_eq!(*calls.lock().unwrap(), ["connect", "set", "clear", "close"]);
    }

    #[test]
    fn explicit_disconnect_clears_activity_before_reenable() {
        let mut client: Option<MockClient> = None;
        let mut cached_activity = None;
        let mut statuses = vec![];

        handle_command(
            &mut client,
            &mut cached_activity,
            &make_client,
            DiscordCommand::Connect,
            &mut |c| statuses.push(c),
        );
        handle_command(
            &mut client,
            &mut cached_activity,
            &make_client,
            DiscordCommand::SetActivity(activity()),
            &mut |c| statuses.push(c),
        );
        handle_command(
            &mut client,
            &mut cached_activity,
            &make_client,
            DiscordCommand::Disconnect,
            &mut |c| statuses.push(c),
        );
        handle_command(
            &mut client,
            &mut cached_activity,
            &make_client,
            DiscordCommand::Connect,
            &mut |c| statuses.push(c),
        );

        assert_eq!(statuses, [true, false, true]);
        assert!(cached_activity.is_none());
        assert_eq!(
            client.as_ref().expect("client should reconnect").activities,
            0
        );
    }

    fn activity_json(activity: &DiscordActivity) -> serde_json::Value {
        serde_json::to_value(build_activity(activity)).expect("activity should serialize")
    }

    #[test]
    fn build_activity_populates_state_details_and_type() {
        let json = activity_json(&activity());

        assert_eq!(json["state"], "Watching");
        assert_eq!(json["details"], "Movie");
        assert_eq!(json["type"], 3);
        assert_eq!(json["assets"]["large_text"], "Stremio");
    }

    #[test]
    fn build_activity_omits_empty_state() {
        let json = activity_json(&DiscordActivity {
            state: String::new(),
            ..activity()
        });

        assert!(json.get("state").is_none());
        assert_eq!(json["details"], "Movie");
    }

    #[test]
    fn build_activity_omits_empty_details() {
        let json = activity_json(&DiscordActivity {
            details: String::new(),
            ..activity()
        });

        assert_eq!(json["state"], "Watching");
        assert!(json.get("details").is_none());
    }

    #[test]
    fn build_activity_uses_provided_image() {
        let json = activity_json(&activity());

        assert_eq!(
            json["assets"]["large_image"],
            "https://example.com/poster.jpg"
        );
    }

    #[test]
    fn build_activity_without_image_uses_fallback_asset() {
        let json = activity_json(&DiscordActivity {
            image: None,
            ..activity()
        });

        assert_eq!(json["assets"]["large_image"], "stremio_logo");
        assert_eq!(json["assets"]["large_text"], "Stremio");
    }

    #[test]
    fn build_activity_without_timestamps_omits_them() {
        let json = activity_json(&DiscordActivity {
            start_timestamp: None,
            end_timestamp: None,
            ..activity()
        });

        assert!(json.get("timestamps").is_none());
    }

    #[test]
    fn build_activity_with_only_start_timestamp() {
        let json = activity_json(&DiscordActivity {
            end_timestamp: None,
            ..activity()
        });

        assert_eq!(json["timestamps"]["start"], 1_752_700_000);
        assert!(json["timestamps"].get("end").is_none());
    }

    #[test]
    fn build_activity_with_only_end_timestamp() {
        let json = activity_json(&DiscordActivity {
            start_timestamp: None,
            ..activity()
        });

        assert_eq!(json["timestamps"]["end"], 1_752_707_200);
        assert!(json["timestamps"].get("start").is_none());
    }

    #[test]
    fn build_activity_with_both_timestamps() {
        let json = activity_json(&activity());

        assert_eq!(json["timestamps"]["start"], 1_752_700_000);
        assert_eq!(json["timestamps"]["end"], 1_752_707_200);
    }

    #[test]
    fn build_activity_clamps_fields_to_utf16_limit() {
        let json = activity_json(&DiscordActivity {
            state: "s".repeat(MAX_FIELD_LENGTH + 50),
            details: "🎬".repeat(MAX_FIELD_LENGTH + 1),
            ..activity()
        });

        let state = json["state"].as_str().expect("state should be present");
        assert_eq!(utf16_len(state), MAX_FIELD_LENGTH);
        let details = json["details"].as_str().expect("details should be present");
        assert_eq!(details, "🎬".repeat(MAX_FIELD_LENGTH / 2));
        assert_eq!(utf16_len(details), MAX_FIELD_LENGTH);
    }

    fn utf16_len(value: &str) -> usize {
        value.chars().map(char::len_utf16).sum()
    }

    #[test]
    fn clamp_field_keeps_strings_below_limit() {
        assert_eq!(clamp_field("Watching"), "Watching");
        assert_eq!(clamp_field(""), "");
    }

    #[test]
    fn clamp_field_keeps_strings_exactly_at_limit() {
        let value = "b".repeat(MAX_FIELD_LENGTH);
        assert_eq!(clamp_field(&value), value);
    }

    #[test]
    fn clamp_field_limits_overlong_bmp_strings() {
        let value = "a".repeat(MAX_FIELD_LENGTH + 50);
        let clamped = clamp_field(&value);

        assert_eq!(utf16_len(&clamped), MAX_FIELD_LENGTH);
        assert!(value.starts_with(&clamped));
    }

    #[test]
    fn clamp_field_counts_multibyte_bmp_chars_as_one_unit() {
        // 'á' is one UTF-16 code unit but two UTF-8 bytes.
        let value = "á".repeat(MAX_FIELD_LENGTH + 1);
        let clamped = clamp_field(&value);

        assert_eq!(clamped, "á".repeat(MAX_FIELD_LENGTH));
    }

    #[test]
    fn clamp_field_limits_non_bmp_emoji_by_utf16_units() {
        // '🎬' is outside the BMP and takes two UTF-16 code units.
        let value = "🎬".repeat(MAX_FIELD_LENGTH / 2 + 1);
        let clamped = clamp_field(&value);

        assert_eq!(clamped, "🎬".repeat(MAX_FIELD_LENGTH / 2));
        assert_eq!(utf16_len(&clamped), MAX_FIELD_LENGTH);
    }

    #[test]
    fn clamp_field_never_splits_a_surrogate_pair() {
        // With a single UTF-16 unit left, the two-unit emoji is dropped whole.
        let value = format!("{}🎬", "a".repeat(MAX_FIELD_LENGTH - 1));
        let clamped = clamp_field(&value);

        assert_eq!(clamped, "a".repeat(MAX_FIELD_LENGTH - 1));
        assert_eq!(utf16_len(&clamped), MAX_FIELD_LENGTH - 1);
    }
}
