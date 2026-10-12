use adw::subclass::prelude::*;
use ashpd::desktop::file_chooser::FileFilter;
use gtk::{
    gio::{Settings, prelude::SettingsExt},
    glib::{self, clone, subclass::InitializingObject},
    prelude::{EditableExt, WidgetExt},
};

use crate::{
    app::{config::APP_ID, ext::ASHPDExt},
    spawn_local,
    utils::IS_DESKTOP_KDE,
};

#[derive(Default, gtk::CompositeTemplate)]
#[template(file = "preferences.xml")]
pub struct PreferencesDialog {
    #[template_child]
    remember_window_state: TemplateChild<adw::SwitchRow>,
    #[template_child]
    kde_theme: TemplateChild<adw::SwitchRow>,
    #[template_child]
    no_window_decorations: TemplateChild<adw::SwitchRow>,
    #[template_child]
    mpv_config_location: TemplateChild<adw::EntryRow>,
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

    #[template_callback]
    fn on_no_window_decorations_changed(&self) {
        let settings = Settings::new(APP_ID);
        settings
            .set_boolean(
                "no-window-decorations",
                self.no_window_decorations.is_active(),
            )
            .ok();
    }

    #[template_callback]
    fn on_mpv_config_location_changed(&self) {
        let settings = Settings::new(APP_ID);
        let value = self.mpv_config_location.text();
        settings.set_string("mpv-config-location", &value).ok();
    }

    #[template_callback]
    fn browse_mpv_config_file(&self, _: &gtk::Button) {
        let filter = FileFilter::new("MPV Config")
            .mimetype("text/plain")
            .glob("*.conf");

        let choose_file = self.choose_file(filter);

        spawn_local!(clone!(
            #[weak(rename_to = entry)]
            self.mpv_config_location,
            async move {
                if let Some(path) = choose_file.await {
                    let value = path.to_string_lossy();
                    entry.set_text(&value);

                    let settings = Settings::new(APP_ID);
                    settings.set_string("mpv-config-location", &value).ok();
                }
            }
        ));
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

        let no_window_decorations = settings.boolean("no-window-decorations");
        self.no_window_decorations.set_active(no_window_decorations);

        let mpv_config_location = settings.string("mpv-config-location");
        self.mpv_config_location.set_text(&mpv_config_location);
    }
}

impl WidgetImpl for PreferencesDialog {}
impl AdwDialogImpl for PreferencesDialog {}
impl PreferencesDialogImpl for PreferencesDialog {}
