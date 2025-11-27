mod display_handler;
mod keyboard_handler;
mod lifespan_handler;
mod load_handler;
mod render_handler;

use std::os::raw::c_int;

use display_handler::WebViewDisplayHandler;
use lifespan_handler::WebViewLifeSpanHandler;
use load_handler::WebViewLoadHandler;
use render_handler::WebViewRenderHandler;

use crate::{
    WebViewEvent, cef_impl,
    webview::{
        SENDER,
        app::client::keyboard_handler::WebViewKeyboardHandler,
        constants::{IPC_MESSAGE, READY_MESSAGE},
    },
};

cef_impl!(
    prefix = "WebView",
    name = Client,
    sys_type = cef_dll_sys::cef_client_t,
    {
        fn display_handler(&self) -> Option<DisplayHandler> {
            Some(WebViewDisplayHandler::new())
        }

        fn render_handler(&self) -> Option<RenderHandler> {
            Some(WebViewRenderHandler::new())
        }

        fn life_span_handler(&self) -> Option<LifeSpanHandler> {
            Some(WebViewLifeSpanHandler::new())
        }

        fn load_handler(&self) -> Option<LoadHandler> {
            Some(WebViewLoadHandler::new())
        }

        fn keyboard_handler(&self) -> Option<KeyboardHandler> {
            Some(WebViewKeyboardHandler::new())
        }

        fn on_process_message_received(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            _source_process: ProcessId,
            message: Option<&mut ProcessMessage>,
        ) -> c_int {
            if let Some(message) = message {
                let name = CefString::from(&message.name());

                let ready_message_name = CefString::from(READY_MESSAGE);
                if name.as_slice() == ready_message_name.as_slice()
                    && let Some(sender) = SENDER.get()
                {
                    sender.send(WebViewEvent::Ready).ok();
                }

                let ipc_message_name = CefString::from(IPC_MESSAGE);
                if name.as_slice() == ipc_message_name.as_slice() {
                    let arguments = message.argument_list().unwrap();
                    let data = CefString::from(&arguments.string(0));

                    if let Some(sender) = SENDER.get() {
                        sender.send(WebViewEvent::Ipc(data.to_string())).ok();
                    }
                }
            }

            Default::default()
        }
    }
);
