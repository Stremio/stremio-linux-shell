use std::sync::{
    Arc, LazyLock, OnceLock,
    mpsc::{Sender, channel},
};

use gettextrs::gettext;
use gtk::{
    glib::{self, object::ObjectExt, subclass::Signal},
    subclass::prelude::*,
};
use ksni::{Handle, MenuItem, TrayMethods, menu::StandardItem};
use tokio::sync::Mutex;

use crate::{
    app::{
        config::{APP_ID, APP_NAME},
        tray::config::ICON_FILE,
    },
    spawn_local,
};

#[derive(Default)]
pub struct Tray {
    handle: Arc<Mutex<Option<Handle<TrayIcon>>>>,
}

impl Tray {
    pub fn update(&self, state: bool) {
        let local_handle = self.handle.clone();
        spawn_local!(async move {
            let handle_guard = local_handle.lock().await;
            if let Some(handle) = handle_guard.as_ref() {
                handle.update(|tray| tray.window_visible = state).await;
            }
        });
    }
}

#[glib::object_subclass]
impl ObjectSubclass for Tray {
    const NAME: &'static str = "Tray";
    type Type = super::Tray;
    type ParentType = glib::Object;
}

impl ObjectImpl for Tray {
    fn signals() -> &'static [Signal] {
        static SIGNALS: OnceLock<Vec<Signal>> = OnceLock::new();
        SIGNALS.get_or_init(|| {
            vec![
                Signal::builder("show").build(),
                Signal::builder("hide").build(),
                Signal::builder("quit").build(),
            ]
        })
    }

    fn constructed(&self) {
        self.parent_constructed();

        let (sender, receiver) = channel::<TrayEvent>();

        let tray_icon = TrayIcon {
            sender,
            window_visible: true,
        };

        let local_handle = self.handle.clone();
        spawn_local!(async move {
            let mut handle_guard = local_handle.lock().await;
            let handle = tray_icon
                .spawn_without_dbus_name()
                .await
                .expect("Failed to create tray icon");
            *handle_guard = Some(handle);
        });

        let object_weak = self.obj().downgrade();
        glib::idle_add_local(move || {
            receiver.try_iter().for_each(|event| {
                if let Some(object) = object_weak.upgrade() {
                    match event {
                        TrayEvent::Show => object.emit_by_name::<()>("show", &[]),
                        TrayEvent::Hide => object.emit_by_name::<()>("hide", &[]),
                        TrayEvent::Quit => object.emit_by_name::<()>("quit", &[]),
                    }
                }
            });

            glib::ControlFlow::Continue
        });
    }
}

#[derive(Debug)]
pub enum TrayEvent {
    Show,
    Hide,
    Quit,
}

pub struct TrayIcon {
    sender: Sender<TrayEvent>,
    window_visible: bool,
}

impl ksni::Tray for TrayIcon {
    fn id(&self) -> String {
        APP_ID.into()
    }

    fn title(&self) -> String {
        APP_NAME.into()
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        static ICON: LazyLock<ksni::Icon> = LazyLock::new(|| {
            let (data, width, height) = load_image(ICON_FILE);

            ksni::Icon {
                width: width as i32,
                height: height as i32,
                data,
            }
        });

        vec![ICON.clone()]
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let sender_show = self.sender.clone();
        let sender_hide = self.sender.clone();
        let sender_quit = self.sender.clone();

        vec![
            StandardItem {
                label: gettext("Show"),
                visible: !self.window_visible,
                activate: Box::new(move |_| {
                    sender_show.send(TrayEvent::Show).ok();
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: gettext("Hide"),
                visible: self.window_visible,
                activate: Box::new(move |_| {
                    sender_hide.send(TrayEvent::Hide).ok();
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: gettext("Quit"),
                activate: Box::new(move |_| {
                    sender_quit.send(TrayEvent::Quit).ok();
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}

fn load_image(buffer: &[u8]) -> (Vec<u8>, u32, u32) {
    let image = image::load_from_memory(buffer)
        .expect("Failed to open icon path")
        .into_rgba8();

    let (width, height) = image.dimensions();
    let mut data = image.into_raw();

    for pixel in data.chunks_exact_mut(4) {
        pixel.rotate_right(1) // rgba to argb
    }

    (data, width, height)
}
