use std::os::raw::c_int;

use crate::{
    cef_impl,
    webview::{
        SENDER, WebViewEvent,
        constants::{IPC_RECEIVER, IPC_SENDER, PRELOAD_SCRIPT},
    },
};

cef_impl!(
    prefix = "WebView",
    name = LoadHandler,
    sys_type = cef_dll_sys::cef_load_handler_t,
    {
        fn on_load_start(
            &self,
            _browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            _transition_type: TransitionType,
        ) {
            if let Some(frame) = frame
                && frame.is_main() == 1
            {
                let script = PRELOAD_SCRIPT
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
            http_status_code: c_int,
        ) {
            if let Some(frame) = frame
                && frame.is_main() == 1
                && http_status_code == 200
                && let Some(sender) = SENDER.get()
            {
                sender.send(WebViewEvent::Loaded).ok();
            }
        }
    }
);
