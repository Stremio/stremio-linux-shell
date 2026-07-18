mod imp;
mod ipc;
mod worker;

use adw::subclass::prelude::ObjectSubclassIsExt;
use gtk::glib;

use crate::app::ipc::event::DiscordActivity;

glib::wrapper! {
    pub struct Discord(ObjectSubclass<imp::Discord>);
}

impl Default for Discord {
    fn default() -> Self {
        glib::Object::builder().build()
    }
}

impl Discord {
    pub fn start(&self) {
        self.imp().start();
    }

    pub fn stop(&self) {
        self.imp().stop();
    }

    pub fn connect(&self) {
        self.imp().connect();
    }

    pub fn disconnect(&self) {
        self.imp().disconnect();
    }

    pub fn set_activity(&self, activity: DiscordActivity) {
        self.imp().set_activity(activity);
    }

    pub fn clear_activity(&self) {
        self.imp().clear_activity();
    }

    pub fn connect_status<F: Fn(bool) + 'static>(&self, callback: F) {
        self.imp().set_status_callback(callback);
    }
}
