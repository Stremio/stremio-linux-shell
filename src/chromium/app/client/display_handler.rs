use cef::{rc::*, *};
use flume::Sender;

use crate::chromium::ChromiumEvent;

wrap_display_handler! {
    pub struct ChromiumDisplayHandler {
        sender: Sender<ChromiumEvent>,
    }

    impl DisplayHandler {
        fn on_fullscreen_mode_change(
            &self,
            _browser: Option<&mut Browser>,
            fullscreen: i32,
        ) {
            let state = fullscreen == 1;

            self.sender.send(ChromiumEvent::Fullscreen(state)).ok();
        }
    }
}
