mod app;
mod config;
mod types;

use std::path::{Path, PathBuf};
use std::ptr;
use std::sync::{Arc, Mutex, RwLock};

use cef::sys::{
    cef_drag_operations_mask_t as OperationsMask, cef_key_event_type_t::KEYEVENT_CHAR,
    cef_log_severity_t::LOGSEVERITY_VERBOSE, cef_mouse_button_type_t::MBT_LEFT,
    cef_mouse_button_type_t::MBT_MIDDLE, cef_mouse_button_type_t::MBT_RIGHT,
    cef_paint_element_type_t::PET_VIEW,
};
use cef::{Frame as MainFrame, args::Args, *};
use flume::Receiver;

use app::ChromiumApp;
use config::IPC_SENDER;
use types::Viewport;
use url::Url;

use crate::chromium::config::MAX_FRAME_RATE;
use crate::shared::{
    Frame,
    states::{KeyboardState, PointerState},
};

#[derive(Debug)]
pub enum ChromiumEvent {
    Ready,
    Loaded,
    Fullscreen(bool),
    Render(Frame),
    Open(Url),
    Ipc(String),
}

pub struct Chromium {
    args: Args,
    app: App,
    settings: Settings,
    browser: Arc<Mutex<Option<Browser>>>,
    viewport: Arc<RwLock<Viewport>>,
    receiver: Receiver<ChromiumEvent>,
}

impl Chromium {
    pub fn new(data_dir: &Path) -> Self {
        let _ = api_hash(sys::CEF_API_VERSION_LAST, 0);

        let args = Args::new();

        let browser = Arc::new(Mutex::new(None));
        let viewport = Arc::new(RwLock::new(Viewport::default()));

        let (sender, receiver) = flume::unbounded();
        let app = ChromiumApp::new(browser.clone(), viewport.clone(), sender);

        let cache_path = data_dir.join("cache");
        let log_path = data_dir.join("log");

        let settings = Settings {
            no_sandbox: true.into(),
            windowless_rendering_enabled: true.into(),
            multi_threaded_message_loop: true.into(),
            root_cache_path: cache_path.to_str().unwrap().into(),
            cache_path: cache_path.to_str().unwrap().into(),
            log_file: log_path.to_str().unwrap().into(),
            log_severity: LogSeverity::from(LOGSEVERITY_VERBOSE),
            ..Default::default()
        };

        Self {
            args,
            app,
            settings,
            browser,
            viewport,
            receiver,
        }
    }

    pub fn execute(&mut self) -> Option<i32> {
        let exit_code = cef::execute_process(
            Some(self.args.as_main_args()),
            Some(&mut self.app),
            ptr::null_mut(),
        );

        if exit_code >= 0 {
            return Some(exit_code);
        }

        None
    }

    pub fn start(&mut self) {
        cef::initialize(
            Some(self.args.as_main_args()),
            Some(&self.settings),
            Some(&mut self.app),
            ptr::null_mut(),
        );
    }

    pub fn stop(&self) {
        if let Ok(mut browser) = self.browser.lock()
            && let Some(browser) = browser.take()
            && let Some(browser_host) = browser.host()
        {
            browser_host.close_browser(0);
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

    pub fn load_url(&self, url: &str) {
        if let Some(main_frame) = self.main_frame() {
            let url = CefString::from(url);
            main_frame.load_url(Some(&url));
        }
    }

    pub fn on_event<F: Fn(ChromiumEvent)>(&self, handler: F) {
        self.receiver.try_iter().for_each(handler);
    }

    pub fn set_monitor_info(&self, refresh_rate: f64, scale_factor: i32) {
        if let Ok(mut viewport) = self.viewport.write() {
            viewport.scale_factor = scale_factor;
        }

        if let Some(browser_host) = self.browser_host() {
            browser_host.set_windowless_frame_rate(refresh_rate.min(MAX_FRAME_RATE) as i32);
            browser_host.notify_screen_info_changed();
        }
    }

    pub fn resize(&self, width: i32, height: i32) {
        if let Ok(mut viewport) = self.viewport.write() {
            viewport.width = width;
            viewport.height = height;

            if let Some(browser_host) = self.browser_host() {
                browser_host.was_resized();
                browser_host.invalidate(PET_VIEW.into());
            }
        }
    }

    pub fn hidden(&self, state: bool) {
        if let Some(browser_host) = self.browser_host() {
            browser_host.was_hidden(state.into());
        }
    }

    pub fn focus(&self, state: bool) {
        if let Some(browser_host) = self.browser_host() {
            browser_host.set_focus(state.into());
        }
    }

    pub fn forward_motion(&self, pointer_state: &PointerState) {
        if let Some(browser_host) = self.browser_host() {
            let mouse_event = MouseEvent::from(pointer_state);
            let mouse_leave = (!pointer_state.over()).into();
            browser_host.send_mouse_move_event(Some(&mouse_event), mouse_leave);
        }
    }

    pub fn forward_scroll(&self, pointer_state: &PointerState, delta_x: f64, delta_y: f64) {
        if let Some(browser_host) = self.browser_host() {
            let mouse_event = MouseEvent::from(pointer_state);

            browser_host.send_mouse_wheel_event(Some(&mouse_event), delta_x as i32, delta_y as i32);
        }
    }

    pub fn forward_click(&self, pointer_state: &PointerState, count: i32) {
        if let Some(browser_host) = self.browser_host() {
            let pressed = pointer_state.pressed();
            let button = pointer_state.button();

            let mouse_event = MouseEvent::from(pointer_state);

            let r#type = match button {
                1 => Some(MBT_LEFT.into()),
                2 => Some(MBT_MIDDLE.into()),
                3 => Some(MBT_RIGHT.into()),
                8 if self.go_back() => None,
                9 if self.go_forward() => None,
                _ => None,
            };

            let mouse_up = (!pressed).into();

            if let Some(r#type) = r#type {
                browser_host.send_mouse_click_event(Some(&mouse_event), r#type, mouse_up, count);
            }
        }
    }

    pub fn forward_key(&self, keyboard_state: &KeyboardState) {
        if let Some(browser_host) = self.browser_host() {
            if let Some(character) = keyboard_state.character()
                && keyboard_state.pressed()
                && !keyboard_state.control_modifier()
            {
                let event = cef::KeyEvent {
                    type_: KEYEVENT_CHAR.into(),
                    character: character as u16,
                    ..Default::default()
                };

                browser_host.send_key_event(Some(&event));
            }

            let key_event = KeyEvent::from(keyboard_state);
            browser_host.send_key_event(Some(&key_event));
        }
    }

    pub fn forward_file_enter(&self, pointer_state: &PointerState, path: PathBuf) {
        if let Some(browser_host) = self.browser_host() {
            let file_path = path.to_str().map(CefString::from);
            let file_name = path
                .file_name()
                .and_then(|name| name.to_str())
                .map(CefString::from);

            if let Some(mut drag_data) = cef::drag_data_create() {
                drag_data.add_file(file_path.as_ref(), file_name.as_ref());

                let mouse_event = MouseEvent::from(pointer_state);

                browser_host.drag_target_drag_enter(
                    Some(&mut drag_data),
                    Some(&mouse_event),
                    OperationsMask::DRAG_OPERATION_MOVE.into(),
                );
            }
        }
    }

    pub fn forward_file_hover(&self, pointer_state: &PointerState) {
        if let Some(browser_host) = self.browser_host() {
            let mouse_event = MouseEvent::from(pointer_state);

            browser_host.drag_target_drag_over(
                Some(&mouse_event),
                OperationsMask::DRAG_OPERATION_MOVE.into(),
            );
        }
    }

    pub fn forward_file_drop(&self, pointer_state: &PointerState) {
        if let Some(browser_host) = self.browser_host() {
            let mouse_event = MouseEvent::from(pointer_state);

            browser_host.drag_target_drop(Some(&mouse_event));
        }
    }

    pub fn forward_file_leave(&self) {
        if let Some(browser_host) = self.browser_host() {
            browser_host.drag_target_drag_leave();
        }
    }

    pub fn clipboard(&self, data: String) {
        if let Some(browser_host) = self.browser_host() {
            for character in data.chars() {
                let event = cef::KeyEvent {
                    type_: KEYEVENT_CHAR.into(),
                    character: character as u16,
                    ..Default::default()
                };

                browser_host.send_key_event(Some(&event));
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

    fn go_back(&self) -> bool {
        if let Ok(browser) = self.browser.lock()
            && let Some(browser) = browser.as_ref()
        {
            browser.go_back();
        }

        true
    }

    fn go_forward(&self) -> bool {
        if let Ok(browser) = self.browser.lock()
            && let Some(browser) = browser.as_ref()
        {
            browser.go_forward();
        }

        true
    }

    fn browser_host(&self) -> Option<BrowserHost> {
        if let Ok(browser) = self.browser.lock()
            && let Some(browser) = browser.as_ref()
        {
            return browser.host();
        }

        None
    }

    fn main_frame(&self) -> Option<MainFrame> {
        if let Ok(browser) = self.browser.lock()
            && let Some(browser) = browser.as_ref()
        {
            return browser.main_frame();
        }

        None
    }
}
