mod about;
mod config;
mod imp;
mod ipc;
mod tray;
mod video;
mod webview;
mod window;

use gtk::{
    gio::{self, ActionEntry, ApplicationFlags, prelude::*},
    glib::{self, ExitCode, Object},
    prelude::*,
};

use crate::app::{
    about::AboutDialog,
    config::{APP_ID, APP_NAME},
};

glib::wrapper! {
    pub struct Application(ObjectSubclass<imp::Application>)
    @extends gio::Application, gtk::Application, adw::Application,
    @implements gio::ActionGroup, gio::ActionMap;
}

impl Default for Application {
    fn default() -> Self {
        Self::new()
    }
}

impl Application {
    pub fn new() -> Self {
        glib::set_application_name(APP_NAME);

        Object::builder()
            .property("application-id", APP_ID)
            .property("flags", ApplicationFlags::HANDLES_OPEN)
            .build()
    }

    pub fn run(&self) -> ExitCode {
        let args: Vec<String> = vec![];
        self.run_with_args(&args)
    }

    fn setup_actions(&self) {
        let quit_action = ActionEntry::builder("quit")
            .activate(|app: &Self, _, _| {
                app.quit();
            })
            .build();

        let show_about_action = ActionEntry::builder("show-about")
            .activate(|app: &Self, _, _| {
                if let Some(window) = app.active_window() {
                    let dialog = AboutDialog::new();
                    dialog.show(&window);
                }
            })
            .build();

        self.add_action_entries([quit_action, show_about_action]);
    }

    fn setup_accels(&self) {
        self.set_accels_for_action("app.quit", &["<Control>q"]);
    }
}
