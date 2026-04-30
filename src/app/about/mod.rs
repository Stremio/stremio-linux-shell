use adw::prelude::AdwDialogExt;
use chrono::{Datelike, Utc};
use gtk::glib::object::IsA;
use itertools::Itertools;

use crate::app::config::{APP_ID, APP_NAME};

pub struct AboutDialog {
    dialog: adw::AboutDialog,
}

impl AboutDialog {
    pub fn new() -> Self {
        let authors = env!("CARGO_PKG_AUTHORS").split(':').collect_vec();
        let copyright = format!("© {} {}", Utc::now().year(), authors[0]);

        let dialog = adw::AboutDialog::builder()
            .application_icon(APP_ID)
            .application_name(APP_NAME)
            .version(env!("CARGO_PKG_VERSION"))
            .website(env!("CARGO_PKG_HOMEPAGE"))
            .issue_url(env!("CARGO_PKG_REPOSITORY"))
            .license_type(gtk::License::Gpl30Only)
            .copyright(copyright)
            .developers(&*authors)
            .designers(&*authors)
            .build();

        Self { dialog }
    }

    pub fn show(&self, parent: &impl IsA<gtk::Widget>) {
        self.dialog.present(Some(parent));
    }
}
