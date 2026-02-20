mod config;
mod imp;

use adw::subclass::prelude::ObjectSubclassIsExt;
use gtk::glib::{self, closure_local, object::ObjectExt};

glib::wrapper! {
    pub struct Tray(ObjectSubclass<imp::Tray>);
}

impl Default for Tray {
    fn default() -> Self {
        glib::Object::builder().build()
    }
}

impl Tray {
    pub fn update(&self, state: bool) {
        self.imp().update(state);
    }

    pub fn connect_show<T: Fn() + 'static>(&self, callback: T) {
        self.connect_closure(
            "show",
            false,
            closure_local!(move |_: Tray| {
                callback();
            }),
        );
    }

    pub fn connect_hide<T: Fn() + 'static>(&self, callback: T) {
        self.connect_closure(
            "hide",
            false,
            closure_local!(move |_: Tray| {
                callback();
            }),
        );
    }

    pub fn connect_quit<T: Fn() + 'static>(&self, callback: T) {
        self.connect_closure(
            "quit",
            false,
            closure_local!(move |_: Tray| {
                callback();
            }),
        );
    }
}
