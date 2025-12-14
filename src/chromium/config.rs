pub const IPC_SENDER: &str = "__postMessage";
pub const IPC_RECEIVER: &str = "__onMessage";

pub const IPC_MESSAGE: &str = "IPC";
pub const READY_MESSAGE: &str = "READY";

pub const IPC_SCRIPT: &str = include_str!("ipc.js");

pub const CMD_SWITCHES: &[&str] = &[
    // "disable-web-security",
];
