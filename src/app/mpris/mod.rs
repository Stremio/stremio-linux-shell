mod imp;

use gtk::glib::{self, closure_local, prelude::*, subclass::prelude::*};

glib::wrapper! {
    pub struct Mpris(ObjectSubclass<imp::Mpris>);
}

impl Default for Mpris {
    fn default() -> Self {
        glib::Object::builder().build()
    }
}

impl Mpris {
    pub fn start(&self) {
        self.imp().start();
    }

    pub fn stop(&self) {
        self.imp().stop();
    }

    pub fn set_status(&self, paused: bool) {
        self.imp().set_status(paused);
    }

    pub fn set_metadata(&self, title: String, artist: Option<String>, art_url: Option<String>) {
        self.imp().set_metadata(title, artist, art_url);
    }

    pub fn connect_paused<T: Fn(bool) + 'static>(&self, callback: T) {
        self.connect_closure(
            "paused",
            false,
            closure_local!(move |_: Mpris, status: bool| {
                callback(status);
            }),
        );
    }

    pub fn connect_raise<T: Fn() + 'static>(&self, callback: T) {
        self.connect_closure(
            "raise",
            false,
            closure_local!(move |_: Mpris| {
                callback();
            }),
        );
    }
}
