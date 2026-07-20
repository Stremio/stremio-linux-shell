use std::{cell::Cell, fs::File, os::fd::AsFd, sync::Arc};

use adw::prelude::*;
use adw::subclass::prelude::*;
use ashpd::{
    Uri, WindowIdentifier,
    desktop::{
        Request,
        background::Background,
        inhibit::{InhibitFlags, InhibitOptions, InhibitProxy},
        open_uri::OpenFileRequest,
    },
    enumflags2::BitFlags,
};
use gtk::{
    gio::Settings,
    glib::{self, clone, subclass::InitializingObject},
    prelude::WidgetExt,
};
use tokio::sync::Mutex;
use tracing::error;

use crate::{app::config::APP_ID, spawn_local, utils::IS_DESKTOP_KDE};

#[derive(Default, glib::Properties, gtk::CompositeTemplate)]
#[properties(wrapper_type = super::Window)]
#[template(file = "window.ui")]
pub struct Window {
    #[property(get, set)]
    decorations: Cell<bool>,
    #[template_child]
    header: TemplateChild<adw::HeaderBar>,
    #[template_child]
    pub overlay: TemplateChild<gtk::Overlay>,
    pub inhibit_request: Arc<Mutex<Option<Request<()>>>>,
}

impl Window {
    pub fn request_backgound(&self) {
        let object = self.obj();

        spawn_local!(clone!(
            #[weak]
            object,
            async move {
                if let Some(identifier) = WindowIdentifier::from_native(&object).await {
                    let request = Background::request().identifier(identifier);
                    request
                        .send()
                        .await
                        .map_err(|e| error!("Failed to set background mode: {e}"))
                        .ok();
                }
            }
        ));
    }

    pub fn disable_idling(&self) {
        let object = self.obj();
        let inhibit_request = self.inhibit_request.clone();

        spawn_local!(clone!(
            #[weak]
            object,
            async move {
                if let Some(identifier) = WindowIdentifier::from_native(&object).await
                    && let Ok(proxy) = InhibitProxy::new().await
                {
                    let mut flags = BitFlags::empty();
                    flags.insert(InhibitFlags::Idle);

                    let options = InhibitOptions::default()
                        .set_reason("Prevent screen from going blank during media playback");

                    let mut inhibit_request = inhibit_request.lock().await;
                    if let Some(request) = inhibit_request.take() {
                        if let Err(e) = request.close().await {
                            error!("Failed to close the inhibit request: {e}");
                        }

                        *inhibit_request = proxy
                            .inhibit(Some(&identifier), flags, options)
                            .await
                            .map_err(|e| error!("Failed to prevent idling: {e}"))
                            .ok();
                    }
                }
            }
        ));
    }

    pub fn enable_idling(&self) {
        let inhibit_request = self.inhibit_request.clone();

        spawn_local!(async move {
            let mut inhibit_request = inhibit_request.lock().await;
            if let Some(request) = inhibit_request.take() {
                request
                    .close()
                    .await
                    .map_err(|e| error!("Failed to allow idling: {e}"))
                    .ok();
            }
        });
    }

    pub fn open_uri(&self, uri: String) {
        let object = self.obj();

        spawn_local!(clone!(
            #[weak]
            object,
            async move {
                if let Some(identifier) = WindowIdentifier::from_native(&object).await
                    && let Ok(uri) = Uri::parse(&uri)
                {
                    let request = OpenFileRequest::default().identifier(identifier);

                    request
                        .send_uri(&uri)
                        .await
                        .map_err(|e| error!("Failed to open uri: {e}"))
                        .ok();
                }
            }
        ));
    }

    pub fn open_file(&self, file_path: String) {
        let object = self.obj();

        spawn_local!(clone!(
            #[weak]
            object,
            async move {
                if let Some(identifier) = WindowIdentifier::from_native(&object).await {
                    let request = OpenFileRequest::default().identifier(identifier);

                    if let Ok(file) = File::open(&file_path) {
                        request
                            .send_file(&file.as_fd())
                            .await
                            .map_err(|e| error!("Failed to open file: {e}"))
                            .ok();
                    }
                }
            }
        ));
    }

    pub fn show_header(&self, state: bool) {
        self.header.set_visible(self.decorations.get() && state);
    }
}

#[glib::object_subclass]
impl ObjectSubclass for Window {
    const NAME: &'static str = "Window";
    type Type = super::Window;
    type ParentType = adw::ApplicationWindow;

    fn class_init(klass: &mut Self::Class) {
        klass.bind_template();
    }

    fn instance_init(obj: &InitializingObject<Self>) {
        obj.init_template();
    }
}

#[glib::derived_properties]
impl ObjectImpl for Window {
    fn constructed(&self) {
        self.parent_constructed();

        let settings = Settings::new(APP_ID);

        let kde_theme_enabled = settings.boolean("kde-theme");

        if *IS_DESKTOP_KDE && kde_theme_enabled {
            self.header.add_css_class("kde");
        }

        if cfg!(debug_assertions) {
            self.obj().add_css_class("devel");
        }
    }
}

impl WidgetImpl for Window {
    fn realize(&self) {
        self.parent_realize();

        let widget = self.obj();
        let settings = Settings::new(APP_ID);

        if !self.decorations.get() {
            self.show_header(false);
            widget.remove_css_class("csd");
        }

        let remember_window_state = settings.boolean("remember-window-state");
        if remember_window_state {
            let maximized = settings.boolean("window-maximized");
            widget.set_maximized(maximized);

            let fullscreen = settings.boolean("window-fullscreen");
            widget.set_fullscreen(fullscreen);

            if !maximized && !fullscreen {
                let height = settings.int("window-height");
                widget.set_default_height(height);

                let width = settings.int("window-width");
                widget.set_default_width(width);
            }
        }
    }

    fn unrealize(&self) {
        let widget = self.obj();
        let settings = Settings::new(APP_ID);

        let remember_window_state = settings.boolean("remember-window-state");
        if remember_window_state {
            let maximized = widget.is_maximized();
            settings.set_boolean("window-maximized", maximized).ok();

            let fullscreen = widget.is_fullscreen();
            settings.set_boolean("window-fullscreen", fullscreen).ok();

            if !maximized && !fullscreen {
                let height = widget.default_height();
                settings.set_int("window-height", height).ok();

                let width = widget.default_width();
                settings.set_int("window-width", width).ok();
            }
        }

        self.parent_unrealize();
    }
}

impl WindowImpl for Window {
    fn activate_default(&self) {
        self.parent_activate_default();

        let widget = self.obj();
        widget.request_backgound();
    }

    fn close_request(&self) -> glib::Propagation {
        self.parent_close_request();

        let widget = self.obj();
        widget.set_visible(false);

        glib::Propagation::Stop
    }
}

impl ApplicationWindowImpl for Window {}
impl AdwApplicationWindowImpl for Window {}
