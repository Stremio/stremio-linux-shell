mod imp;

use adw::subclass::prelude::*;
use gtk::{
    Widget, gio,
    glib::{self, object::IsA},
    prelude::*,
};

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

        window.overlay.set_child(Some(widget));
    }

    pub fn set_overlay(&self, widget: &impl IsA<Widget>) {
        let window = self.imp();

        window.overlay.add_overlay(&graphics_offload(widget));
    }

    pub fn set_fullscreen(&self, fullscreen: bool) {
        let window = self.imp();

        window.header.set_visible(!fullscreen);
        self.set_fullscreened(fullscreen);
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

    pub fn open_uri(&self, uri: String) {
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
