use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use adw::{prelude::*, subclass::prelude::*};
use gtk::glib::{self, Properties, clone};

use crate::app::{
    config::{PRELOAD_SCRIPT, URI_SCHEME, URL_DEV, URL_PROD},
    ipc::{
        self,
        event::{IpcEvent, IpcEventMpv},
    },
    tray::Tray,
    video::Video,
    webview::WebView,
    window::Window,
};

#[derive(Properties, Default)]
#[properties(wrapper_type = super::Application)]
pub struct Application {
    #[property(get, set)]
    dev_mode: Cell<bool>,
    tray: RefCell<Option<Tray>>,
    video: RefCell<Option<Video>>,
    webview: RefCell<Option<WebView>>,
    deeplink: Rc<RefCell<Option<String>>>,
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

        let dev_mode = self.dev_mode.get();

        let url = match dev_mode {
            true => URL_DEV,
            false => URL_PROD,
        };

        let webview = WebView::default();
        webview.load_uri(url);
        webview.inject_script(PRELOAD_SCRIPT);
        webview.dev_mode(dev_mode);

        let window = Window::new(&app);
        window.set_underlay(&video);
        window.set_overlay(&webview);

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

        video.connect_property_change(clone!(
            #[weak]
            webview,
            move |name, value| {
                let message = ipc::create_response(IpcEvent::Mpv(IpcEventMpv::Change((
                    name.to_string(),
                    value,
                ))));

                webview.send(&message);
            }
        ));

        video.connect_mpv_ended(clone!(
            #[weak]
            webview,
            move |error| {
                let message = ipc::create_response(IpcEvent::Mpv(IpcEventMpv::Ended(error)));
                webview.send(&message);
            }
        ));

        video.connect_mpv_error(clone!(
            #[weak]
            webview,
            move |error| {
                let message = ipc::create_response(IpcEvent::Mpv(IpcEventMpv::Ended(Some(error))));
                webview.send(&message);
            }
        ));

        let deeplink = self.deeplink.clone();
        webview.connect_ipc(clone!(
            #[weak]
            app,
            #[weak]
            window,
            #[weak]
            video,
            move |webview: WebView, message: &str| {
                if let Ok(event) = ipc::parse_request(message) {
                    match event {
                        IpcEvent::Init => {
                            let message = ipc::create_response(IpcEvent::Init);
                            webview.send(&message);
                        }
                        IpcEvent::Ready => {
                            if let Some(ref uri) = *deeplink.borrow() {
                                let message =
                                    ipc::create_response(IpcEvent::OpenMedia(uri.to_string()));
                                webview.send(&message);
                            }
                        }
                        IpcEvent::Fullscreen(state) => {
                            window.set_fullscreen(state);

                            let message = ipc::create_response(IpcEvent::Fullscreen(state));
                            webview.send(&message);
                        }
                        IpcEvent::Quit => {
                            app.quit();
                        }
                        IpcEvent::Mpv(event) => match event {
                            IpcEventMpv::Observe(name) => video.observe_property(name),
                            IpcEventMpv::Command((name, args)) => video.send_command(name, args),
                            IpcEventMpv::Set((name, value)) => video.set_property(name, value),
                            _ => {}
                        },
                        _ => {}
                    }
                }
            }
        ));

        webview.connect_fullscreen(clone!(
            #[weak]
            window,
            move |fullscreen: bool| {
                window.set_fullscreen(fullscreen);
            }
        ));

        webview.connect_open_external(clone!(
            #[weak]
            window,
            move |uri| {
                window.open_uri(uri.to_string());
            }
        ));

        window.connect_visibility(clone!(
            #[weak]
            webview,
            #[weak]
            tray,
            move |state| {
                let message = ipc::create_response(IpcEvent::Visibility(state));
                webview.send(&message);

                tray.update(state);
            }
        ));

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
        *self.video.borrow_mut() = Some(video);
        *self.webview.borrow_mut() = Some(webview);

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

                if let Some(ref webview) = *self.webview.borrow() {
                    let message = ipc::create_response(IpcEvent::OpenMedia(uri));
                    webview.send(&message);
                }
            }
        }
    }
}

impl GtkApplicationImpl for Application {}
impl AdwApplicationImpl for Application {}
