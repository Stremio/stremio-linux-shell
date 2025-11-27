use std::os::raw::c_int;

use cef_dll_sys::{cef_event_flags_t, cef_key_event_type_t};

use crate::{cef_impl, webview::constants::ZOOM_AMOUNT};

const MINUS_KEY_CODE: c_int = 61;
const EQUAL_KEY_CODE: c_int = 173;

cef_impl!(
    prefix = "WebView",
    name = KeyboardHandler,
    sys_type = cef_dll_sys::cef_keyboard_handler_t,
    {
        fn on_key_event(
            &self,
            browser: Option<&mut Browser>,
            event: Option<&KeyEvent>,
            _os_event: Option<&mut cef_dll_sys::XEvent>,
        ) -> c_int {
            if let Some(event) = event
                && event.type_ == cef_key_event_type_t::KEYEVENT_RAWKEYDOWN.into()
                && event.modifiers == cef_event_flags_t::EVENTFLAG_CONTROL_DOWN as u32
            {
                if event.windows_key_code == MINUS_KEY_CODE {
                    set_zoom_level(browser, ZOOM_AMOUNT);
                    return true.into();
                }

                if event.windows_key_code == EQUAL_KEY_CODE {
                    set_zoom_level(browser, -ZOOM_AMOUNT);
                    return true.into();
                }
            }

            false.into()
        }
    }
);

fn set_zoom_level(browser: Option<&mut Browser>, amount: f64) {
    if let Some(browser) = browser
        && let Some(host) = browser.host()
    {
        let zoom_level = host.zoom_level();
        host.set_zoom_level(zoom_level + amount);
    }
}
