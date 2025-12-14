use std::sync::{Arc, Mutex, RwLock};

use cef::sys::cef_state_t::STATE_ENABLED;
use cef::{rc::*, *};
use flume::Sender;

use crate::chromium::{ChromiumEvent, app::client::ChromiumClient, types::Viewport};

wrap_browser_process_handler! {
    pub struct ChromiumBrowserProcessHandler {
        browser: Arc<Mutex<Option<Browser>>>,
        viewport: Arc<RwLock<Viewport>>,
        sender: Sender<ChromiumEvent>,
    }

    impl BrowserProcessHandler {
        fn on_context_initialized(&self) {
            let mut client = ChromiumClient::new(self.viewport.clone(), self.sender.clone());
            let url = CefString::from("about:blank");

            let window_info = WindowInfo {
                windowless_rendering_enabled: true.into(),
                shared_texture_enabled: false.into(),
                ..Default::default()
            };

            let settings = BrowserSettings {
                javascript_access_clipboard: STATE_ENABLED.into(),
                javascript_dom_paste: STATE_ENABLED.into(),
                ..Default::default()
            };

            let browser_result = browser_host_create_browser_sync(
                Some(&window_info),
                Some(&mut client),
                Some(&url),
                Some(&settings),
                None,
                None,
            );

            if let Ok(mut browser) = self.browser.lock() {
                *browser = browser_result;
            }
        }
    }
}
