mod config;
mod imp;

use adw::subclass::prelude::ObjectSubclassIsExt;
use gtk::glib::{self, Variant, closure_local, object::ObjectExt};
use itertools::Itertools;
use libmpv2::Format;
use serde_json::{Number, Value};
use tracing::warn;

use crate::app::video::config::{BOOL_PROPERTIES, FLOAT_PROPERTIES, STRING_PROPERTIES};

/// The `hwdec` mode to request. Zero-copy keeps decoded frames on the GPU — a
/// large CPU/bandwidth win (measured ~100% -> ~23% of a core on 4K HDR); copy-back
/// (`*-copy`) round-trips every frame through system RAM.
///
/// - **Mesa (AMD/Intel):** `auto-safe` selects VAAPI dmabuf zero-copy.
/// - **Nvidia proprietary:** `auto-safe` only offers copy-back (`vulkan-copy`),
///   so request `nvdec` explicitly for CUDA-interop zero-copy. Verified clean and
///   ~4x cheaper than copy-back on 4K HDR. (Earlier reports of `nvdec` artifacts
///   were a host-only libmpv build issue; see BENCHMARKS.md / DEVLOG.)
///
/// REQUIRES libmpv built with the CUDA interop, else `nvdec` falls back to
/// copy-back/software: mpv `--enable-cuda-hwaccel --enable-cuda-interop`, ffmpeg
/// with `--enable-ffnvcodec` (nvdec), and `libplacebo`. Verified with libmpv
/// 0.41 / ffmpeg 7.1 (org.gnome.Platform 50 runtime). Distro `libmpv` packages
/// (e.g. for the .deb) must ship these features for the Nvidia win to apply.
///
/// `STREMIO_HWDEC` overrides everything (e.g. `nvdec`, `auto-safe`, `auto-copy`,
/// `no`) for testing decode modes without a rebuild.
fn default_hwdec() -> String {
    std::env::var("STREMIO_HWDEC").unwrap_or_else(|_| {
        if std::path::Path::new("/dev/nvidia0").exists() {
            "nvdec"
        } else {
            "auto-safe"
        }
        .to_string()
    })
}

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
    pub fn connect_mpv_property_change<T: Fn(&str, Value) + 'static>(&self, callback: T) {
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

    pub fn connect_playback_ended<T: Fn(&str) + 'static>(&self, callback: T) {
        self.connect_closure(
            "playback-ended",
            false,
            closure_local!(move |_: Video, reason: &str| {
                callback(reason);
            }),
        );
    }

    pub fn send_mpv_command(&self, name: String, args: Vec<String>) {
        let widget = self.imp();

        let args = args.iter().map(String::as_ref).collect_vec();
        widget.send_command(&name, &args);
    }

    pub fn observe_mpv_property(&self, name: String) {
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
            _ => warn!("Failed to observe property {name}: Unsupported"),
        };
    }

    pub fn set_mpv_property(&self, name: String, value: Value) {
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
                    // The web UI enables hardware decoding by sending
                    // `hwdec=auto-copy` (copy-back: decode on the GPU, copy every
                    // frame back to system RAM and re-upload it — CPU/bandwidth
                    // heavy, ~30x costlier than zero-copy on 4K HDR). Remap it to
                    // the platform's zero-copy mode (`super::default_hwdec`).
                    let hwdec = (name == "hwdec" && value == "auto-copy").then(default_hwdec);
                    let value = hwdec.as_deref().unwrap_or(value);

                    widget.set_property(name, value);
                }
            }
            name => warn!("Failed to set property {name}: Unsupported"),
        };
    }
}
