mod adapters;
mod app;
mod cef_impl;
mod constants;

use std::path::PathBuf;

use adapters::{NativeKeyCode, WindowsKeyCode};
use app::WebViewApp;
use cef::{
    App, Browser, BrowserHost, BrowserSettings, CefString, Client, Frame, ImplBrowser,
    ImplBrowserHost, ImplCommandLine, ImplDragData, ImplFrame, LogSeverity, Settings, api_hash,
    args::Args, execute_process, initialize,
};
use cef_dll_sys::{
    cef_drag_operations_mask_t, cef_event_flags_t, cef_key_event_type_t, cef_log_severity_t,
    cef_mouse_button_type_t, cef_paint_element_type_t, cef_pointer_type_t, cef_touch_event_type_t,
};
use constants::IPC_SENDER;
use crossbeam_channel::{Receiver, Sender, unbounded};
use once_cell::sync::OnceCell;
use url::Url;
use winit::{
    event::{KeyEvent, MouseButton, Touch, TouchPhase},
    event_loop::EventLoopProxy,
    keyboard::{ModifiersState, PhysicalKey},
};

use crate::{
    config::WebViewConfig,
    shared::types::{Cursor, MouseState, UserEvent},
};

static SENDER: OnceCell<Sender<WebViewEvent>> = OnceCell::new();
pub static PROXY: OnceCell<EventLoopProxy<UserEvent>> = OnceCell::new();
static BROWSER: OnceCell<Browser> = OnceCell::new();

pub enum WebViewEvent {
    Ready,
    Loaded,
    Paint,
    Resized,
    Cursor(Cursor),
    Open(Url),
    Ipc(String),
}

pub struct WebView {
    args: Args,
    settings: Settings,
    app: App,
    receiver: Receiver<WebViewEvent>,
    scroll_accumulator_x: f64,
    scroll_accumulator_y: f64,
}

impl WebView {
    pub fn new(config: WebViewConfig, proxy: EventLoopProxy<UserEvent>) -> Self {
        let _ = api_hash(cef_dll_sys::CEF_API_VERSION_LAST, 0);

        let args = Args::new();

        let (sender, receiver) = unbounded::<WebViewEvent>();
        SENDER.get_or_init(|| sender);
        PROXY.get_or_init(|| proxy);

        let app = WebViewApp::new();

        config.remove_lock_file();

        let settings = Settings {
            no_sandbox: 1,
            windowless_rendering_enabled: 1,
            multi_threaded_message_loop: 1,
            cache_path: config.cache_dir.to_str().unwrap().into(),
            log_file: config.log_file.to_str().unwrap().into(),
            log_severity: LogSeverity::from(cef_log_severity_t::LOGSEVERITY_VERBOSE),
            ..Default::default()
        };

        Self {
            args,
            settings,
            app,
            receiver,
            scroll_accumulator_x: 0.0,
            scroll_accumulator_y: 0.0,
        }
    }

    fn browser_host(&self) -> Option<BrowserHost> {
        if let Some(browser) = BROWSER.get() {
            return browser.host();
        }

        None
    }

    fn main_frame(&self) -> Option<Frame> {
        if let Some(browser) = BROWSER.get() {
            return browser.main_frame();
        }

        None
    }

    pub fn should_exit(&mut self) -> bool {
        let ret = execute_process(
            Some(self.args.as_main_args()),
            Some(&mut self.app),
            std::ptr::null_mut(),
        );

        let cmd = self.args.as_cmd_line().unwrap();

        let switch = CefString::from("type");
        let is_browser_process = cmd.has_switch(Some(&switch)) != 1;

        if is_browser_process {
            assert!(ret == -1, "cannot execute browser process");
            false
        } else {
            assert!(ret >= 0, "cannot execute non-browser process");
            true
        }
    }

    pub fn start(&mut self) {
        assert_eq!(
            initialize(
                Some(self.args.as_main_args()),
                Some(&self.settings),
                Some(&mut self.app),
                std::ptr::null_mut(),
            ),
            1
        );
    }

    pub fn stop(&self) {
        if let Some(host) = self.browser_host() {
            host.close_browser(0);
        }
    }

    pub fn events<T: FnMut(WebViewEvent)>(&self, handler: T) {
        self.receiver.try_iter().for_each(handler);
    }

    pub fn navigate(&self, url: &str) {
        if let Some(main_frame) = self.main_frame() {
            let url = CefString::from(url);
            main_frame.load_url(Some(&url));
        }
    }

    pub fn dev_tools(&self, state: bool) {
        if let Some(host) = self.browser_host() {
            if state {
                host.show_dev_tools(
                    None,
                    Option::<&mut Client>::None,
                    Option::<&BrowserSettings>::None,
                    None,
                );
            } else {
                host.close_dev_tools();
            }
        }
    }

    pub fn post_message(&self, message: String) {
        if let Some(main_frame) = self.main_frame() {
            let serialized_message =
                serde_json::to_string(&message).expect("Failed to serialize as JSON string");

            let script = format!("{IPC_SENDER}({serialized_message})");

            let code = CefString::from(script.as_str());
            main_frame.execute_java_script(Some(&code), None, 0);
        }
    }

    pub fn send_clipboard_response(&self, text: String) {
        if let Some(main_frame) = self.main_frame() {
            let escaped = serde_json::to_string(&text).expect("Failed to serialize clipboard text");
            let script = format!("CLIPBOARD_RESPONSE({escaped})");
            let code = CefString::from(script.as_str());
            main_frame.execute_java_script(Some(&code), None, 0);
        }
    }

    /// Tells the webview to update its bounds and repaint.
    pub fn update(&self) {
        if let Some(host) = self.browser_host() {
            host.was_resized();
            // Invalidate the view to ensure the `view_rect` callback is triggered,
            // since repeated calls to `was_resized` may fail to invoke it consistently.
            host.invalidate(cef_paint_element_type_t::PET_VIEW.into());
        }
    }

    pub fn focused(&mut self, state: bool) {
        if let Some(host) = self.browser_host() {
            host.set_focus(state.into());
        }
    }

    pub fn scale_factor_changed(&self, scale_factor: f64) {
        use crate::shared::types::SCALE_FACTOR;
        SCALE_FACTOR.store(scale_factor.to_bits(), std::sync::atomic::Ordering::Relaxed);

        if let Some(host) = self.browser_host() {
            // host.notify_screen_info_changed();
            host.was_resized();

            // Calculate and apply zoom level: Zoom = ln(scale) / ln(1.2)
            // 1.2 is the default zoom step in Chromium
            if scale_factor > 0.1 {
                let zoom_level = scale_factor.ln() / 1.2f64.ln();
                host.set_zoom_level(zoom_level);
            }
        }
    }

    pub fn apply_zoom(&self) {
        use crate::shared::types::SCALE_FACTOR;
        let scale_factor = SCALE_FACTOR.load(std::sync::atomic::Ordering::Relaxed);
        let scale = f64::from_bits(scale_factor);

        if scale > 0.1 {
            if let Some(host) = self.browser_host() {
                let zoom_level = scale.ln() / 1.2f64.ln();
                host.set_zoom_level(zoom_level);
            }
        }
    }

    pub fn mouse_moved(&mut self, state: MouseState) {
        if let Some(host) = self.browser_host() {
            let event = state.into();
            let mouse_leave = (!state.over).into();
            host.send_mouse_move_event(Some(&event), mouse_leave);
        }
    }

    pub fn mouse_wheel(&mut self, state: MouseState) {
        if let Some(host) = self.browser_host() {
            let event = state.into();

            self.scroll_accumulator_x += state.delta.0;
            self.scroll_accumulator_y += state.delta.1;

            let delta_x = self.scroll_accumulator_x as i32;
            let delta_y = self.scroll_accumulator_y as i32;

            if delta_x != 0 || delta_y != 0 {
                host.send_mouse_wheel_event(Some(&event), delta_x, delta_y);

                self.scroll_accumulator_x -= delta_x as f64;
                self.scroll_accumulator_y -= delta_y as f64;
            }
        }
    }

    pub fn mouse_input(&self, state: MouseState) {
        if let Some(browser) = BROWSER.get() {
            let mouse_up = !state.pressed;

            let button_type = match state.button {
                MouseButton::Back if mouse_up => {
                    browser.go_back();
                    None
                }
                MouseButton::Forward if mouse_up => {
                    browser.go_forward();
                    None
                }
                MouseButton::Left => Some(cef_mouse_button_type_t::MBT_LEFT),
                MouseButton::Right => Some(cef_mouse_button_type_t::MBT_RIGHT),
                MouseButton::Middle => Some(cef_mouse_button_type_t::MBT_MIDDLE),
                _ => None,
            };

            if let Some(button_type) = button_type
                && let Some(host) = browser.host()
            {
                let event = state.into();

                host.send_mouse_click_event(Some(&event), button_type.into(), mouse_up.into(), 1);
            }
        }
    }

    pub fn touch_input(&self, touch: Touch) {
        if let Some(host) = self.browser_host() {
            let event_type = match touch.phase {
                TouchPhase::Started => cef_touch_event_type_t::CEF_TET_PRESSED,
                TouchPhase::Ended => cef_touch_event_type_t::CEF_TET_RELEASED,
                TouchPhase::Moved => cef_touch_event_type_t::CEF_TET_MOVED,
                TouchPhase::Cancelled => cef_touch_event_type_t::CEF_TET_CANCELLED,
            };

            let event = cef::TouchEvent {
                type_: event_type.into(),
                pointer_type: cef_pointer_type_t::CEF_POINTER_TYPE_TOUCH.into(),
                x: touch.location.x as f32,
                y: touch.location.y as f32,
                ..Default::default()
            };

            host.send_touch_event(Some(&event));
        }
    }

    pub fn keyboard_input(&self, key_event: KeyEvent, modifiers: ModifiersState) {
        if let Some(host) = self.browser_host() {
            if let PhysicalKey::Code(code) = key_event.physical_key
                && let (Ok(WindowsKeyCode(windows_key_code)), Ok(NativeKeyCode(native_key_code))) =
                    (code.try_into(), code.try_into())
            {
                let event_type = match key_event.state.is_pressed() {
                    true => cef_key_event_type_t::KEYEVENT_KEYDOWN.into(),
                    false => cef_key_event_type_t::KEYEVENT_KEYUP.into(),
                };

                let modifiers = if modifiers.control_key() {
                    cef_event_flags_t::EVENTFLAG_CONTROL_DOWN.0
                } else {
                    cef_event_flags_t::EVENTFLAG_NONE.0
                };

                let event = cef::KeyEvent {
                    type_: event_type,
                    windows_key_code,
                    native_key_code,
                    modifiers,
                    ..Default::default()
                };

                host.send_key_event(Some(&event));
            }

            if key_event.state.is_pressed()
                && let Some(text) = key_event.text
            {
                let event = cef::KeyEvent {
                    type_: cef_key_event_type_t::KEYEVENT_CHAR.into(),
                    character: text.as_str().chars().next().map(|c| c as u16).unwrap(),
                    ..Default::default()
                };

                host.send_key_event(Some(&event));
            }
        }
    }

    pub fn file_hover(&self, path: PathBuf, state: MouseState) {
        if let Some(host) = self.browser_host() {
            let event = state.into();

            let file_path = path.to_str().map(CefString::from);
            let file_name = path
                .file_name()
                .and_then(|name| name.to_str())
                .map(CefString::from);

            if let Some(mut drag_data) = cef::drag_data_create() {
                drag_data.add_file(file_path.as_ref(), file_name.as_ref());

                host.drag_target_drag_enter(
                    Some(&mut drag_data),
                    Some(&event),
                    cef_drag_operations_mask_t::DRAG_OPERATION_MOVE.into(),
                );

                host.drag_target_drag_over(
                    Some(&event),
                    cef_drag_operations_mask_t::DRAG_OPERATION_MOVE.into(),
                );
            }
        }
    }

    pub fn file_drop(&self, state: MouseState) {
        if let Some(host) = self.browser_host() {
            let event = state.into();
            host.drag_target_drop(Some(&event));
        }
    }

    pub fn file_cancel(&self) {
        if let Some(host) = self.browser_host() {
            host.drag_target_drag_leave();
        }
    }
}
