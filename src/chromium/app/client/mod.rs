mod display_handler;
mod lifespan_handler;
mod load_handler;
mod permission_handler;
mod render_handler;

use std::sync::{Arc, RwLock};

use cef::{rc::*, *};
use flume::Sender;

use crate::chromium::{
    ChromiumEvent,
    app::client::{
        display_handler::ChromiumDisplayHandler, lifespan_handler::ChromiumLifeSpanHandler,
        load_handler::ChromiumLoadHandler, permission_handler::ChromiumPermissionHandler,
    },
    config::{IPC_MESSAGE, READY_MESSAGE},
    types::Viewport,
};
use render_handler::ChromiumRenderHandler;

wrap_client! {
    pub struct ChromiumClient {
        viewport: Arc<RwLock<Viewport>>,
        sender: Sender<ChromiumEvent>,
    }

    impl Client {
        fn render_handler(&self) -> Option<RenderHandler> {
            Some(ChromiumRenderHandler::new(
                self.viewport.clone(),
                self.sender.clone(),
            ))
        }

        fn load_handler(&self) -> Option<LoadHandler> {
            Some(ChromiumLoadHandler::new(self.sender.clone()))
        }

        fn life_span_handler(&self) -> Option<LifeSpanHandler> {
            Some(ChromiumLifeSpanHandler::new(self.sender.clone()))
        }

        fn permission_handler(&self) -> Option<PermissionHandler> {
            Some(ChromiumPermissionHandler::new())
        }

        fn display_handler(&self) -> Option<DisplayHandler> {
            Some(ChromiumDisplayHandler::new(self.sender.clone()))
        }

        fn on_process_message_received(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            _source_process: ProcessId,
            message: Option<&mut ProcessMessage>,
        ) -> i32 {
            if let Some(message) = message {
                let name = CefString::from(&message.name());

                let ready_message_name = CefString::from(READY_MESSAGE);
                if name.as_slice() == ready_message_name.as_slice() {
                    self.sender.send(ChromiumEvent::Ready).ok();
                }

                let ipc_message_name = CefString::from(IPC_MESSAGE);
                if name.as_slice() == ipc_message_name.as_slice() {
                    let arguments = message.argument_list().unwrap();
                    let data = CefString::from(&arguments.string(0));

                    self.sender.send(ChromiumEvent::Ipc(data.to_string())).ok();
                }
            }

            Default::default()
        }
    }
}
