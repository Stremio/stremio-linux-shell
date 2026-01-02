mod browser_process_handler;
mod client;
mod process;
mod render_process_handler;

use std::sync::{Arc, Mutex, RwLock};

use cef::{rc::*, *};
use flume::Sender;

use crate::chromium::{
    ChromiumEvent, app::render_process_handler::ChromiumRenderProcessHandler, config::CMD_SWITCHES,
    types::Viewport,
};
use browser_process_handler::ChromiumBrowserProcessHandler;

wrap_app! {
    pub struct ChromiumApp {
        browser: Arc<Mutex<Option<Browser>>>,
        viewport: Arc<RwLock<Viewport>>,
        sender: Sender<ChromiumEvent>,
    }

    impl App {
        fn on_before_command_line_processing(
            &self,
            _process_type: Option<&CefString>,
            command_line: Option<&mut CommandLine>,
        ) {
            if let Some(line) = command_line {
                CMD_SWITCHES.iter().for_each(|switch| {
                    line.append_switch(Some(&CefString::from(switch.to_owned())));
                });
            }
        }

        fn browser_process_handler(&self) -> Option<BrowserProcessHandler> {
            Some(ChromiumBrowserProcessHandler::new(
                self.browser.clone(),
                self.viewport.clone(),
                self.sender.clone(),
            ))
        }

        fn render_process_handler(&self) -> Option<RenderProcessHandler> {
            Some(ChromiumRenderProcessHandler::new())
        }
    }
}
