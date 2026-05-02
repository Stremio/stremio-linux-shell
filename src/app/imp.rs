use std::cell::{Cell, RefCell};

use adw::{prelude::*, subclass::prelude::*};
use gtk::glib::{self, Properties, clone};

use crate::app::{
    config::{APP_ID, APP_NAME, URI_SCHEME},
    ipc::{
        self,
        event::{IpcEvent, IpcEventMpv},
    },
    mpris::Mpris,
    tray::Tray,
    video::Video,
    webview::WebView,
    window::Window,
};

const PRELOAD_SCRIPT: &str = include_str!("ipc/preload.js");

#[derive(Properties, Default)]
#[properties(wrapper_type = super::Application)]
pub struct Application {
    #[property(get, set)]
    dev_mode: Cell<bool>,
    #[property(get, set)]
    startup_url: RefCell<String>,
    #[property(get, set)]
    decorations: Cell<bool>,
    tray: RefCell<Option<Tray>>,
    mpris: RefCell<Option<Mpris>>,
    webview: RefCell<Option<WebView>>,
    deeplink: RefCell<Option<String>>,
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
        let mpris = Mpris::default();

        let startup_url = self.startup_url.borrow();
        let dev_mode = self.dev_mode.get();

        let webview = WebView::default();
        webview.load_uri(&startup_url);
        webview.inject_script(PRELOAD_SCRIPT);
        webview.dev_mode(dev_mode);

        let window = Window::new(&app);
        window.set_property("decorations", self.decorations.get());
        window.set_underlay(&video);
        window.set_overlay(&webview);

        window.connect_monitor_info(clone!(
            #[weak]
            video,
            move |scale_factor| {
                video.set_property("scale-factor", scale_factor);
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

        video.connect_mpv_property_change(clone!(
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

        let deeplink = self.deeplink.clone();
        webview.connect_ipc(clone!(
            #[weak]
            app,
            #[weak]
            window,
            #[weak]
            video,
            #[weak]
            mpris,
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
                        IpcEvent::MediaStatus(status) => {
                            mpris.set_status(status);
                        }
                        IpcEvent::MediaMetadata((title, artist, artwork)) => {
                            mpris.set_metadata(title, artist, artwork);
                        }
                        IpcEvent::Quit => {
                            app.quit();
                        }
                        IpcEvent::Mpv(event) => match event {
                            IpcEventMpv::Observe(name) => video.observe_mpv_property(name),
                            IpcEventMpv::Command((name, args)) => {
                                video.send_mpv_command(name, args)
                            }
                            IpcEventMpv::Set((name, value)) => video.set_mpv_property(name, value),
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
                window.open_uri(uri);
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

        mpris.connect_status(clone!(
            #[weak]
            webview,
            move |paused| {
                let message = ipc::create_response(IpcEvent::MediaStatus(paused));
                webview.send(&message);
            }
        ));

        mpris.connect_raise(clone!(
            #[weak]
            window,
            move || {
                window.activate();
            }
        ));

        mpris.start(APP_ID, APP_NAME);

        *self.tray.borrow_mut() = Some(tray);
        *self.mpris.borrow_mut() = Some(mpris);
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
