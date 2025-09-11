mod imp;

use std::rc::Rc;

use adw::subclass::prelude::ObjectSubclassIsExt;
use gtk::{
    gio::Cancellable,
    glib::{self, clone, object::Cast},
};
use tracing::error;
use webkit::{
    NavigationPolicyDecision, PolicyDecisionType, UserContentInjectedFrames, UserScript,
    UserScriptInjectionTime, prelude::WebViewExt,
};

glib::wrapper! {
    pub struct WebView(ObjectSubclass<imp::WebView>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Default for WebView {
    fn default() -> Self {
        glib::Object::builder()
            .property("hexpand", true)
            .property("vexpand", true)
            .build()
    }
}

impl WebView {
    pub fn load_uri(&self, uri: &str) {
        let widget = self.imp();

        widget.webview.load_uri(uri);
    }

    pub fn inject_script(&self, script: &'static str) {
        let widget = self.imp();

        let user_script = UserScript::new(
            script,
            UserContentInjectedFrames::TopFrame,
            UserScriptInjectionTime::Start,
            &[],
            &[],
        );

        if let Some(user_content_manager) = widget.webview.user_content_manager() {
            user_content_manager.add_script(&user_script);
        }
    }

    pub fn dev_mode(&self, state: bool) {
        let widget = self.imp();

        if let Some(settings) = widget.webview.settings() {
            settings.set_enable_developer_extras(state);
        }

        if let Some(inspector) = widget.webview.inspector() {
            if state {
                inspector.show();
            } else {
                inspector.close();
            }
        }
    }

    pub fn send(&self, message: &str) {
        let widget = self.imp();

        let serialized_message =
            serde_json::to_string(&message).expect("Failed to serialize as JSON string");
        let script = format!("__postMessage({serialized_message})");

        widget
            .webview
            .evaluate_javascript(&script, None, None, Cancellable::NONE, |result| {
                if let Err(e) = result {
                    error!("Failed to send message: {e}");
                }
            });
    }

    pub fn connect_ipc<T: Fn(WebView, &str) + 'static>(&self, callback: T) {
        let widget = self.imp();
        let webview = self;

        if let Some(user_content_manager) = widget.webview.user_content_manager() {
            user_content_manager.register_script_message_handler("ipc", None);
            user_content_manager.connect_script_message_received(
                Some("ipc"),
                clone!(
                    #[weak]
                    webview,
                    move |_, value| {
                        let message = value.to_string();
                        callback(webview, &message);
                    }
                ),
            );
        }
    }

    pub fn connect_fullscreen<T: Fn(bool) + 'static>(&self, callback: T) {
        let widget = self.imp();

        let cb = Rc::new(callback);

        let callback = cb.clone();
        widget.webview.connect_enter_fullscreen(move |_| {
            callback(true);
            true
        });

        let callback = cb.clone();
        widget.webview.connect_leave_fullscreen(move |_| {
            callback(false);
            true
        });
    }

    pub fn connect_open_external<T: Fn(&str) + 'static>(&self, callback: T) {
        let widget = self.imp();

        widget
            .webview
            .connect_decide_policy(move |_, decision, decision_type| {
                if let PolicyDecisionType::NewWindowAction = decision_type
                    && let Some(decision) = decision.downcast_ref::<NavigationPolicyDecision>()
                    && let Some(mut action) = decision.navigation_action()
                    && let Some(request) = action.request()
                    && let Some(uri) = request.uri()
                {
                    callback(&uri);
                }

                true
            });
    }
}
