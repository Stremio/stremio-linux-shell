mod imp;

use adw::prelude::AdwDialogExt;
use gtk::glib::{self, object::IsA};

glib::wrapper! {
    pub struct PreferencesDialog(ObjectSubclass<imp::PreferencesDialog>)
    @extends gtk::Widget, gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, adw::Dialog, adw::PreferencesDialog;
}

impl PreferencesDialog {
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    pub fn show(&self, parent: &impl IsA<gtk::Widget>) {
        self.present(Some(parent));
    }
}

impl Default for PreferencesDialog {
    fn default() -> Self {
        Self::new()
    }
}
