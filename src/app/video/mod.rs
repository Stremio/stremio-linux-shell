mod config;
mod imp;

use adw::subclass::prelude::ObjectSubclassIsExt;
use gtk::glib::{self, Variant, closure_local, object::ObjectExt};
use itertools::Itertools;
use libmpv2::Format;
use serde_json::{Number, Value};
use tracing::error;

use crate::app::video::config::{BOOL_PROPERTIES, FLOAT_PROPERTIES, STRING_PROPERTIES};

glib::wrapper! {
    pub struct Video(ObjectSubclass<imp::Video>)
        @extends gtk::GLArea, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Default for Video {
    fn default() -> Self {
        glib::Object::builder()
            .property("hexpand", true)
            .property("vexpand", true)
            .build()
    }
}

impl Video {
    pub fn connect_property_change<T: Fn(&str, Value) + 'static>(&self, callback: T) {
        self.connect_closure(
            "property-changed",
            false,
            closure_local!(move |_: Video, name: &str, value: Variant| {
                match name {
                    name if FLOAT_PROPERTIES.contains(&name) => {
                        if let Some(value) = value.get::<f64>() {
                            callback(name, Value::Number(Number::from_f64(value).unwrap()))
                        }
                    }
                    name if BOOL_PROPERTIES.contains(&name) => {
                        if let Some(value) = value.get::<bool>() {
                            callback(name, Value::Bool(value))
                        }
                    }
                    name if STRING_PROPERTIES.contains(&name) => {
                        if let Some(value) = value.get::<String>() {
                            if let Ok(json_value) = serde_json::from_str::<Value>(&value) {
                                callback(name, json_value)
                            } else {
                                callback(name, Value::String(value))
                            }
                        }
                    }
                    _ => {}
                };
            }),
        );
    }

    pub fn connect_playback_started<T: Fn() + 'static>(&self, callback: T) {
        self.connect_closure(
            "playback-started",
            false,
            closure_local!(move |_: Video| {
                callback();
            }),
        );
    }

    pub fn connect_playback_ended<T: Fn() + 'static>(&self, callback: T) {
        self.connect_closure(
            "playback-ended",
            false,
            closure_local!(move |_: Video| {
                callback();
            }),
        );
    }

    pub fn send_command(&self, name: String, args: Vec<String>) {
        let widget = self.imp();

        let args = args.iter().map(String::as_ref).collect_vec();
        widget.send_command(&name, &args);
    }

    pub fn observe_property(&self, name: String) {
        let widget = self.imp();

        match name.as_str() {
            name if FLOAT_PROPERTIES.contains(&name) => {
                widget.observe_property(name, Format::Double);
            }
            name if BOOL_PROPERTIES.contains(&name) => {
                widget.observe_property(name, Format::Flag);
            }
            name if STRING_PROPERTIES.contains(&name) => {
                widget.observe_property(name, Format::String);
            }
            _ => error!("Failed to observe property {name}: Unsupported"),
        };
    }

    pub fn set_property(&self, name: String, value: Value) {
        let widget = self.imp();

        match name.as_str() {
            name if FLOAT_PROPERTIES.contains(&name) => {
                if let Some(value) = value.as_f64() {
                    widget.set_property(name, value);
                }
            }
            name if BOOL_PROPERTIES.contains(&name) => {
                if let Some(value) = value.as_bool() {
                    widget.set_property(name, value);
                }
            }
            name if STRING_PROPERTIES.contains(&name) => {
                if let Some(value) = value.as_str() {
                    widget.set_property(name, value);
                }
            }
            name => error!("Failed to set property {name}: Unsupported"),
        };
    }
}
