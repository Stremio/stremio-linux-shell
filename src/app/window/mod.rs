mod imp;

use adw::subclass::prelude::*;
use gtk::{
    Widget,
    gdk::prelude::{DisplayExt, MonitorExt},
    gio,
    glib::{self, object::IsA},
    prelude::{GtkWindowExt, NativeExt, WidgetExt},
};
use url::Url;

use crate::app::Application;

glib::wrapper! {
    pub struct Window(ObjectSubclass<imp::Window>)
    @extends gtk::Window, gtk::ApplicationWindow, adw::ApplicationWindow, gtk::Widget,
    @implements gio::ActionGroup, gio::ActionMap, gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::ShortcutManager, gtk::Native, gtk::Root;
}

impl Window {
    pub fn new(application: &Application) -> Self {
        glib::Object::builder()
            .property("application", application)
            .build()
    }

    pub fn set_underlay(&self, widget: &impl IsA<Widget>) {
        let window = self.imp();

        window.overlay.set_child(Some(&graphics_offload(widget)));
    }

    pub fn set_overlay(&self, widget: &impl IsA<Widget>) {
        let window = self.imp();

        window.overlay.add_overlay(&graphics_offload(widget));
    }

    pub fn set_fullscreen(&self, fullscreen: bool) {
        self.imp().show_header(!fullscreen);
        self.set_fullscreened(fullscreen);
    }

    pub fn connect_monitor_info<F: Fn(f64, i32) + 'static>(&self, callback: F) {
        self.connect_realize(move |window| {
            let display = window.display();
            let surface = window.surface();

            if let Some(surface) = surface
                && let Some(monitor) = display.monitor_at_surface(&surface)
            {
                let refresh_rate = monitor.refresh_rate() as f64 / 1000.0;
                let scale_factor = monitor.scale_factor();

                callback(refresh_rate, scale_factor);
            }
        });
    }

    pub fn connect_visibility<T: Fn(bool) + 'static>(&self, callback: T) {
        self.connect_visible_notify(move |window| {
            callback(window.is_visible());
        });
    }

    fn request_backgound(&self) {
        self.imp().request_backgound();
    }

    pub fn disable_idling(&self) {
        self.imp().disable_idling();
    }

    pub fn enable_idling(&self) {
        self.imp().enable_idling();
    }

    pub fn open_uri(&self, uri: Url) {
        self.imp().open_uri(uri);
    }
}

fn graphics_offload(widget: &impl IsA<Widget>) -> gtk::GraphicsOffload {
    gtk::GraphicsOffload::builder()
        .vexpand(true)
        .hexpand(true)
        .child(widget)
        .build()
}
