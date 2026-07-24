mod config;
mod dialogs;
mod discord;
mod imp;
mod ipc;
mod mpris;
mod tray;
mod video;
mod webview;
mod window;

use gtk::{
    CssProvider,
    gdk::Display,
    gio::{self, ActionEntry, ApplicationFlags, prelude::*},
    glib::{self, ExitCode, Object},
    prelude::*,
};
use itertools::Itertools;

use crate::app::{
    config::{APP_ID, APP_NAME, STYLE},
    dialogs::{about::AboutDialog, preferences::PreferencesDialog},
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

    pub async fn run(&self, args: Vec<String>) -> ExitCode {
        let mut program = std::env::args().take(1).collect_vec();
        program.extend(args);
        self.run_with_args(&program)
    }

    fn setup_actions(&self) {
        let quit_action = ActionEntry::builder("quit")
            .activate(|app: &Self, _, _| {
                app.quit();
            })
            .build();

        let show_preferences_action = ActionEntry::builder("show-preferences")
            .activate(|app: &Self, _, _| {
                if let Some(window) = app.active_window() {
                    let dialog = PreferencesDialog::new();
                    dialog.show(&window);
                }
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

        self.add_action_entries([quit_action, show_preferences_action, show_about_action]);
    }

    fn setup_accels(&self) {
        self.set_accels_for_action("app.quit", &["<Control>q"]);
        self.set_accels_for_action("app.show-preferences", &["<Control>comma"]);
    }

    fn setup_css(&self) {
        let provider = CssProvider::new();
        provider.load_from_string(STYLE);

        let display = Display::default().expect("Failed to connect to a display");
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}
