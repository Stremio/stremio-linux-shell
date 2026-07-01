use adw::subclass::prelude::*;
use gtk::{
    gio::{Settings, prelude::SettingsExt},
    glib::{self, subclass::InitializingObject},
    prelude::WidgetExt,
};

use crate::{app::config::APP_ID, utils::IS_DESKTOP_KDE};

#[derive(Default, gtk::CompositeTemplate)]
#[template(file = "preferences.xml")]
pub struct PreferencesDialog {
    #[template_child]
    remember_window_state: TemplateChild<adw::SwitchRow>,
    #[template_child]
    kde_theme: TemplateChild<adw::SwitchRow>,
}

#[gtk::template_callbacks]
impl PreferencesDialog {
    #[template_callback]
    fn on_remember_window_state_changed(&self) {
        let settings = Settings::new(APP_ID);
        let active = self.remember_window_state.is_active();
        settings.set_boolean("remember-window-state", active).ok();
    }

    #[template_callback]
    fn on_kde_theme_changed(&self) {
        let settings = Settings::new(APP_ID);
        settings
            .set_boolean("kde-theme", self.kde_theme.is_active())
            .ok();
    }
}

#[glib::object_subclass]
impl ObjectSubclass for PreferencesDialog {
    const NAME: &'static str = "PreferencesDialog";
    type Type = super::PreferencesDialog;
    type ParentType = adw::PreferencesDialog;

    fn class_init(klass: &mut Self::Class) {
        klass.bind_template();
        klass.bind_template_callbacks();
    }

    fn instance_init(obj: &InitializingObject<Self>) {
        obj.init_template();
    }
}

impl ObjectImpl for PreferencesDialog {
    fn constructed(&self) {
        self.parent_constructed();

        let settings = Settings::new(APP_ID);

        if *IS_DESKTOP_KDE {
            let kde_theme = settings.boolean("kde-theme");
            self.kde_theme.set_active(kde_theme);
        } else {
            self.kde_theme.set_visible(false);
        }

        let remember_window_state = settings.boolean("remember-window-state");
        self.remember_window_state.set_active(remember_window_state);
    }
}

impl WidgetImpl for PreferencesDialog {}
impl AdwDialogImpl for PreferencesDialog {}
impl PreferencesDialogImpl for PreferencesDialog {}
