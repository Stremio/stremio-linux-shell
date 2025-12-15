use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use adw::{prelude::*, subclass::prelude::*};
use gtk::glib::{self, ControlFlow, Properties, clone};

use crate::{
    app::{config::URI_SCHEME, tray::Tray, video::Video, webview::WebView, window::Window},
    chromium::{Chromium, ChromiumEvent},
    shared::ipc::{
        self,
        event::{IpcEvent, IpcEventMpv},
    },
};

#[derive(Properties, Default)]
#[properties(wrapper_type = super::Application)]
pub struct Application {
    #[property(get, set)]
    dev_mode: Cell<bool>,
    #[property(get, set)]
    startup_url: RefCell<String>,
    #[property(get, set)]
    open_uri: RefCell<Option<String>>,
    #[property(get, set)]
    decorations: Cell<bool>,
    tray: RefCell<Option<Tray>>,
    browser: Rc<RefCell<Option<Chromium>>>,
    deeplink: Rc<RefCell<Option<String>>>,
}

impl Application {
    pub fn set_browser(&self, browser: Chromium) {
        *self.browser.borrow_mut() = Some(browser);
    }
}

#[glib::object_subclass]
impl ObjectSubclass for Application {
    const NAME: &'static str = "Application";
    type Type = super::Application;
    type ParentType = adw::Application;
}

#[glib::derived_properties]
impl ObjectImpl for Application {}

impl ApplicationImpl for Application {
    fn startup(&self) {
        self.parent_startup();

        let app = self.obj();
        app.setup_actions();
        app.setup_accels();

        if let Some(ref mut browser) = *self.browser.borrow_mut() {
            browser.start();
        }
    }

    fn activate(&self) {
        self.parent_activate();

        let app = self.obj();

        if let Some(window) = app.active_window() {
            window.present();
            return;
        }

        let tray = Tray::default();
        let video = Video::default();
        let webview = WebView::default();

        let window = Window::new(&app);
        window.set_property("decorations", self.decorations.get());
        window.set_underlay(&video);
        window.set_overlay(&webview);

        let browser = self.browser.clone();
        window.connect_monitor_info(clone!(
            #[weak]
            video,
            #[weak]
            webview,
            move |refresh_rate, scale_factor| {
                if let Some(ref browser) = *browser.borrow() {
                    browser.set_monitor_info(refresh_rate, scale_factor);
                    video.set_property("scale-factor", scale_factor);
                    webview.set_property("scale-factor", scale_factor);
                }
            }
        ));

        video.connect_playback_started(clone!(
            #[weak]
            window,
            move || {
                window.disable_idling();
            }
        ));

        video.connect_playback_ended(clone!(
            #[weak]
            window,
            move || {
                window.enable_idling();
            }
        ));

        let browser = self.browser.clone();
        video.connect_mpv_property_change(move |name, value| {
            if let Some(ref browser) = *browser.borrow() {
                let message = ipc::create_response(IpcEvent::Mpv(IpcEventMpv::Change((
                    name.to_string(),
                    value,
                ))));

                browser.post_message(message);
            }
        });

        let browser = self.browser.clone();
        let dev_mode = self.dev_mode.get();
        let startup_url = self.startup_url.clone();
        let open_uri = self.open_uri.clone();
        let deeplink = self.deeplink.clone();
        glib::idle_add_local(clone!(
            #[weak]
            webview,
            #[weak]
            video,
            #[weak]
            window,
            #[weak]
            app,
            #[upgrade_or]
            ControlFlow::Continue,
            move || {
                if let Some(ref browser) = *browser.borrow() {
                    browser.on_event(|event| match event {
                        ChromiumEvent::Ready => {
                            browser.dev_tools(dev_mode);
                            browser.load_url(&startup_url.borrow());
                        }
                        ChromiumEvent::Loaded => {
                            if let Some(ref uri) = *open_uri.borrow()
                                && uri.starts_with(URI_SCHEME)
                            {
                                let message =
                                    ipc::create_response(IpcEvent::OpenMedia(uri.to_string()));
                                browser.post_message(message);
                            }
                        }
                        ChromiumEvent::Fullscreen(state) => window.set_fullscreen(state),
                        ChromiumEvent::Render(frame) => webview.render(frame),
                        ChromiumEvent::Open(url) => window.open_uri(url),
                        ChromiumEvent::Ipc(message) => {
                            if let Ok(event) = ipc::parse_request(&message) {
                                match event {
                                    IpcEvent::Init => {
                                        let message = ipc::create_response(IpcEvent::Init);
                                        browser.post_message(message);
                                    }
                                    IpcEvent::Ready => {
                                        if let Some(ref uri) = *deeplink.borrow() {
                                            let message = ipc::create_response(
                                                IpcEvent::OpenMedia(uri.to_string()),
                                            );
                                            browser.post_message(message);
                                        }
                                    }
                                    IpcEvent::Quit => {
                                        app.quit();
                                    }
                                    IpcEvent::Mpv(event) => match event {
                                        IpcEventMpv::Observe(name) => {
                                            video.observe_mpv_property(name)
                                        }
                                        IpcEventMpv::Command((name, args)) => {
                                            video.send_command(name, args)
                                        }
                                        IpcEventMpv::Set((name, value)) => {
                                            video.set_mpv_property(name, value)
                                        }
                                        _ => {}
                                    },
                                    _ => {}
                                }
                            }
                        }
                    });
                }

                ControlFlow::Continue
            }
        ));

        let browser = self.browser.clone();
        window.connect_visibility(clone!(
            #[weak]
            tray,
            move |state| {
                if let Some(ref browser) = *browser.borrow() {
                    browser.hidden(!state);

                    let message = ipc::create_response(IpcEvent::Visibility(state));
                    browser.post_message(message);
                }

                tray.update(state);
            }
        ));

        let browser = self.browser.clone();
        webview.connect_has_focus_notify(move |_| {
            if let Some(ref browser) = *browser.borrow() {
                browser.focus(true);
            }
        });

        let browser = self.browser.clone();
        webview.connect_resized(move |width, height| {
            if let Some(ref browser) = *browser.borrow() {
                browser.resize(width, height);
            }
        });

        let browser = self.browser.clone();
        webview.connect_motion(move |pointer_state| {
            if let Some(ref browser) = *browser.borrow() {
                browser.forward_motion(&pointer_state);
            }
        });

        let browser = self.browser.clone();
        webview.connect_scroll(move |pointer_state, delta_x, delta_y| {
            if let Some(ref browser) = *browser.borrow() {
                browser.forward_scroll(&pointer_state, delta_x, delta_y);
            }
        });

        let browser = self.browser.clone();
        webview.connect_click(clone!(
            #[weak]
            webview,
            move |pointer_state, count| {
                if let Some(ref browser) = *browser.borrow() {
                    webview.grab_focus();
                    browser.forward_click(&pointer_state, count);
                }
            }
        ));

        let browser = self.browser.clone();
        webview.connect_keys(clone!(
            #[weak]
            webview,
            move |keyboard_state| {
                if let Some(ref browser) = *browser.borrow() {
                    webview.grab_focus();
                    browser.forward_key(&keyboard_state);
                }
            }
        ));

        let browser = self.browser.clone();
        webview.connect_clipboard(move |text| {
            if let Some(ref browser) = *browser.borrow() {
                browser.clipboard(text);
            }
        });

        let browser = self.browser.clone();
        webview.connect_file_enter(move |pointer_state, path| {
            if let Some(ref browser) = *browser.borrow() {
                browser.forward_file_enter(pointer_state.as_ref(), path);
            }
        });

        let browser = self.browser.clone();
        webview.connect_file_leave(move || {
            if let Some(ref browser) = *browser.borrow() {
                browser.forward_file_leave();
            }
        });

        let browser = self.browser.clone();
        webview.connect_file_motion(move |pointer_state| {
            if let Some(ref browser) = *browser.borrow() {
                browser.forward_file_hover(pointer_state.as_ref());
            }
        });

        let browser = self.browser.clone();
        webview.connect_file_drop(move |pointer_state| {
            if let Some(ref browser) = *browser.borrow() {
                browser.forward_file_drop(pointer_state.as_ref());
            }
        });

        tray.connect_show(clone!(
            #[weak]
            window,
            move || {
                window.set_visible(true);
            }
        ));

        tray.connect_hide(clone!(
            #[weak]
            window,
            move || {
                window.set_visible(false);
            }
        ));

        tray.connect_quit(clone!(
            #[weak]
            app,
            move || {
                app.quit();
            }
        ));

        *self.tray.borrow_mut() = Some(tray);

        window.present();
    }

    fn open(&self, files: &[gtk::gio::File], hint: &str) {
        self.parent_open(files, hint);

        self.activate();

        if let Some(file) = files.first() {
            let uri = file.uri().to_string();
            if uri.starts_with(URI_SCHEME) {
                let mut deeplink = self.deeplink.borrow_mut();
                *deeplink = Some(uri.clone());

                if let Some(ref browser) = *self.browser.borrow() {
                    let message = ipc::create_response(IpcEvent::OpenMedia(uri));
                    browser.post_message(message);
                }
            }
        }
    }

    fn shutdown(&self) {
        if let Some(browser) = self.browser.take() {
            browser.stop();
        }

        self.parent_shutdown();
    }
}

impl GtkApplicationImpl for Application {}
impl AdwApplicationImpl for Application {}
