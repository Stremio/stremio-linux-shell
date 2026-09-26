mod imp;

use std::{cell::Cell, rc::Rc, time::Duration};

use adw::subclass::prelude::ObjectSubclassIsExt;
use gtk::{
    gio::{Cancellable, IOErrorEnum},
    glib::{self, clone, object::Cast},
};
use tracing::{error, warn};
use webkit::{
    NavigationPolicyDecision, PolicyDecisionType, UserContentInjectedFrames, UserScript,
    UserScriptInjectionTime, prelude::WebViewExt,
};

const LOAD_RETRY_INTERVAL: Duration = Duration::from_millis(500);
const LOAD_RETRY_ATTEMPTS: u32 = 60;

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

    pub fn retry_failed_loads(&self) {
        let widget = self.imp();
        let attempts = Cell::new(0);

        widget
            .webview
            .connect_load_failed(move |webview, _, uri, e| {
                if !e.matches(IOErrorEnum::ConnectionRefused)
                    || attempts.get() >= LOAD_RETRY_ATTEMPTS
                {
                    return false;
                }

                attempts.set(attempts.get() + 1);
                warn!("Failed to load {uri}, retrying: {e}");

                let uri = uri.to_owned();
                glib::timeout_add_local_once(
                    LOAD_RETRY_INTERVAL,
                    clone!(
                        #[weak]
                        webview,
                        move || webview.load_uri(&uri)
                    ),
                );

                true
            });
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

    pub fn connect_open_external<T: Fn(String) + 'static>(&self, callback: T) {
        let widget = self.imp();
        let cb = Rc::new(callback);

        let callback = cb.clone();
        widget
            .webview
            .connect_decide_policy(move |_, decision, decision_type| {
                if let Some(uri) = decision
                    .downcast_ref::<NavigationPolicyDecision>()
                    .and_then(|decision| decision.navigation_action())
                    .and_then(|action| action.request())
                    .and_then(|request| request.uri())
                {
                    match decision_type {
                        PolicyDecisionType::NavigationAction if uri.starts_with("data:") => {
                            callback(uri.replace("data:", ""));
                        }
                        PolicyDecisionType::NewWindowAction => {
                            callback(uri.to_string());
                        }
                        _ => {}
                    }
                }

                true
            });

        let callback = cb.clone();
        widget.webview.connect_create(move |_, navigation_action| {
            if let Some(uri) = navigation_action
                .request()
                .and_then(|request| request.uri())
            {
                callback(uri.to_string());
            }

            None
        });
    }
}
