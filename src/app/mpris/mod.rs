mod imp;

use gtk::glib::{self, subclass::prelude::*};

glib::wrapper! {
    pub struct Mpris(ObjectSubclass<imp::Mpris>);
}

impl Default for Mpris {
    fn default() -> Self {
        glib::Object::builder().build()
    }
}

impl Mpris {
    pub fn start(&self, id: &'static str, name: &'static str) {
        self.imp().start(id, name);
    }

    pub fn set_status(&self, paused: bool) {
        self.imp().set_status(paused);
    }

    pub fn set_metadata(&self, title: String, artist: Option<String>, art_url: Option<String>) {
        self.imp().set_metadata(title, artist, art_url);
    }

    pub fn connect_status<F: Fn(bool) + 'static>(&self, callback: F) {
        self.imp().set_status_callback(callback);
    }

    pub fn connect_raise<F: Fn() + 'static>(&self, callback: F) {
        self.imp().set_raise_callback(callback);
    }
}
