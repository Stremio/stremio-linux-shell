use adw::subclass::prelude::*;
use gtk::{
    GestureClick,
    gdk::RGBA,
    glib::{self, clone},
    prelude::*,
};
use webkit::{WebView as WebKitWebView, prelude::*};

use super::ext::SettingsExt;
use crate::utils;

#[derive(Default)]
pub struct WebView {
    pub webview: WebKitWebView,
}

#[glib::object_subclass]
impl ObjectSubclass for WebView {
    const NAME: &'static str = "WebView";
    type Type = super::WebView;
    type ParentType = gtk::Box;
}

impl ObjectImpl for WebView {
    fn constructed(&self) {
        self.parent_constructed();

        let object = self.obj();

        self.webview.set_vexpand(true);
        self.webview.set_hexpand(true);
        self.webview
            .set_background_color(&RGBA::new(0.0, 0.0, 0.0, 0.0));

        if let Some(settings) = WebViewExt::settings(&self.webview) {
            settings.set_enable_media(false);
            settings.set_enable_media_capabilities(false);
            settings.set_enable_media_stream(false);
            settings.set_enable_webaudio(false);
            settings.set_javascript_can_access_clipboard(true);

            // https://github.com/Stremio/stremio-linux-shell/issues/170
            //
            // Not applied on the NVIDIA proprietary driver: with TextureMapper, the web process
            // leaks one sync_file fd per composited frame until it hits RLIMIT_NOFILE (1024),
            // at which point GPU fence creation fails and the UI freezes.
            if webkit::functions::major_version() == 2
                && webkit::functions::minor_version() == 54
                && !utils::is_nvidia_gl_driver()
            {
                // Use TextureMapper instead of Skia for layer composition
                if let Some(compositor) = settings.find_feature("UseSkiaForComposition") {
                    settings.set_feature_enabled(&compositor, false);
                }

                // Disable damage tracking for compositing to repaint the entire view.
                if let Some(damage_tracking) =
                    settings.find_feature("UseDamagingInformationForCompositing")
                {
                    settings.set_feature_enabled(&damage_tracking, false);
                }
            }
        }

        let gesture = GestureClick::new();
        gesture.set_button(0);
        gesture.connect_pressed(clone!(
            #[weak(rename_to = webview)]
            self.webview,
            move |gesture, _, _, _| {
                let button = gesture.current_button();
                match button {
                    8 => webview.go_back(),
                    9 => webview.go_forward(),
                    _ => {}
                }
            }
        ));
        self.webview.add_controller(gesture);

        object.append(&self.webview);
    }
}

impl WidgetImpl for WebView {}
impl BoxImpl for WebView {}
