use cef::{rc::*, *};
use flume::Sender;
use url::Url;

use crate::chromium::ChromiumEvent;

wrap_life_span_handler! {
    pub struct ChromiumLifeSpanHandler {
        sender: Sender<ChromiumEvent>,
    }

    impl LifeSpanHandler {
        fn on_before_popup(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            _popup_id: i32,
            target_url: Option<&CefString>,
            _target_frame_name: Option<&CefString>,
            _target_disposition: WindowOpenDisposition,
            _user_gesture: i32,
            _popup_features: Option<&PopupFeatures>,
            _window_info: Option<&mut WindowInfo>,
            _client: Option<&mut Option<Client>>,
            _settings: Option<&mut BrowserSettings>,
            _extra_info: Option<&mut Option<DictionaryValue>>,
            _no_javascript_access: Option<&mut i32>,
        ) -> i32 {
            if let Some(target_url) = target_url {
                let target_url = target_url.to_string();

                if let Ok(url) = Url::parse(&target_url) {
                    self.sender.send(ChromiumEvent::Open(url)).ok();
                }
            }

            true.into()
        }
    }
}
