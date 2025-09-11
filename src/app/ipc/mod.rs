pub mod event;
mod request;
mod response;

use event::IpcEvent;
use response::IpcMessageResponse;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const TRANSPORT_NAME: &str = "transport";

pub fn parse_request(data: &str) -> Result<IpcEvent, ()> {
    IpcEvent::try_from(data).map_err(|e| eprintln!("{e}"))
}

pub fn create_response(event: IpcEvent) -> String {
    let message = IpcMessageResponse::try_from(event).ok();
    serde_json::to_string(&message).expect("Failed to convert IpcMessage to string")
}
