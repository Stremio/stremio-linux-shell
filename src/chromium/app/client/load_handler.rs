use cef::{rc::*, *};
use flume::Sender;

use crate::chromium::{
    ChromiumEvent,
    config::{IPC_RECEIVER, IPC_SCRIPT, IPC_SENDER},
};

wrap_load_handler! {
    pub struct ChromiumLoadHandler {
        sender: Sender<ChromiumEvent>,
    }

    impl LoadHandler {
        fn on_load_start(
            &self,
            _browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            _transition_type: TransitionType,
        ) {
            if let Some(frame) = frame
                && frame.is_main() == 1
            {
                let script = IPC_SCRIPT
                    .replace("IPC_SENDER", IPC_SENDER)
                    .replace("IPC_RECEIVER", IPC_RECEIVER);
                let code = CefString::from(script.as_str());
                frame.execute_java_script(Some(&code), None, 0);
            }
        }

        fn on_load_end(
            &self,
            _browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            http_status_code: i32,
        ) {
            if let Some(frame) = frame
                && frame.is_main() == 1
                && http_status_code == 200
            {
                self.sender.send(ChromiumEvent::Loaded).ok();
            }
        }
    }
}
