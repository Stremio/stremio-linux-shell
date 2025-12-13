use std::{thread, time::Duration};

use crossbeam_channel::{Receiver, Sender, unbounded};
use gtk::glib;
use rust_i18n::t;
use tray_icon::{
    Icon, TrayIcon, TrayIconBuilder,
    menu::{Menu, MenuEvent, MenuItem},
};

use crate::{config::TrayConfig, shared::types::UserEvent};

const ICON: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/data/icons/symbolic.png"
));

enum TrayEvent {
    Visibility(bool),
    ShowWarning(String),
}

pub struct Tray {
    receiver: Receiver<UserEvent>,
    tray_sender: Sender<TrayEvent>,
}

impl Tray {
    pub fn new(config: TrayConfig) -> Self {
        let (sender, receiver) = unbounded::<UserEvent>();
        let (tray_sender, tray_receiver) = unbounded::<TrayEvent>();

        thread::spawn(|| {
            gtk::init().expect("Failed to initialize gtk");

            let language = gtk::default_language();
            if let Some(language) = language {
                rust_i18n::set_locale(&language.to_str());
            }

            let menu = Self::create_menu();
            let tray = Self::create(menu, config);

            glib::timeout_add_local(Duration::from_millis(16), move || {
                tray_receiver.try_iter().for_each(|event| match event {
                    TrayEvent::Visibility(state) => {
                        let menu = Self::create_menu();
                        menu.remove_at(0);

                        if state {
                            let hide_item = Self::create_menu_item("hide");
                            menu.prepend(&hide_item).ok();
                        } else {
                            let show_item = Self::create_menu_item("show");
                            menu.prepend(&show_item).ok();
                        }

                        tray.set_menu(Some(menu));
                    }
                    TrayEvent::ShowWarning(message) => {
                        use gtk::prelude::*;
                        let dialog = gtk::MessageDialog::new(
                            None::<&gtk::Window>,
                            gtk::DialogFlags::MODAL,
                            gtk::MessageType::Warning,
                            gtk::ButtonsType::Ok,
                            message.as_str(),
                        );
                        dialog.set_title("Stremio - Performance Warning");
                        dialog.run();
                        unsafe {
                            dialog.destroy();
                        }
                    }
                });

                glib::ControlFlow::Continue
            });

            gtk::main();
        });

        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            if event.id == "hide" {
                sender.send(UserEvent::Hide).ok();
            }

            if event.id == "show" {
                sender.send(UserEvent::Show).ok();
            }

            if event.id == "quit" {
                sender.send(UserEvent::Quit).ok();
            }
        }));

        Self {
            receiver,
            tray_sender,
        }
    }

    fn create_menu_item(id: &str) -> MenuItem {
        MenuItem::with_id(id, t!(id), true, None)
    }

    fn create_menu() -> Box<Menu> {
        let empty_item = Self::create_menu_item("");
        let quit_item = Self::create_menu_item("quit");

        let version_label = format!("v{}", env!("CARGO_PKG_VERSION"));
        let version_item = MenuItem::new(version_label.as_str(), false, None);

        let menu = Menu::new();
        menu.append_items(&[&empty_item, &quit_item, &version_item])
            .expect("Failed to append menu items");

        Box::new(menu)
    }

    fn create(menu: Box<Menu>, config: TrayConfig) -> TrayIcon {
        let icon = load_icon(ICON);

        TrayIconBuilder::new()
            .with_menu(menu)
            .with_icon(icon)
            .with_temp_dir_path(config.icon_path)
            .build()
            .expect("Failed to build tray icon")
    }

    pub fn update(&self, visibility: bool) {
        self.tray_sender
            .send(TrayEvent::Visibility(visibility))
            .ok();
    }

    pub fn show_warning(&self, message: String) {
        self.tray_sender.send(TrayEvent::ShowWarning(message)).ok();
    }

    pub fn events<F: FnMut(UserEvent)>(&self, handler: F) {
        self.receiver.try_iter().for_each(handler);
    }
}

fn load_icon(buffer: &[u8]) -> Icon {
    let (icon_rgba, icon_width, icon_height) = {
        let image = image::load_from_memory(buffer)
            .expect("Failed to open icon path")
            .into_rgba8();

        let (width, height) = image.dimensions();
        let rgba = image.into_raw();

        (rgba, width, height)
    };

    Icon::from_rgba(icon_rgba, icon_width, icon_height).expect("Failed to open icon")
}
