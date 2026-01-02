mod gl;
mod imp;

use std::{path::PathBuf, rc::Rc};

use adw::subclass::prelude::ObjectSubclassIsExt;
use gtk::{
    DropTarget, EventControllerKey, EventControllerMotion, EventControllerScroll,
    EventControllerScrollFlags, GestureClick,
    gdk::{Display, DragAction, FileList, Key, ModifierType, ScrollUnit, prelude::DisplayExt},
    gio::{Cancellable, prelude::FileExt},
    glib::{
        self, Object, Priority, Propagation,
        object::{Cast, IsA},
        types::StaticType,
    },
    prelude::*,
};

use crate::shared::{
    Frame,
    states::{KeyboardState, PointerState},
};

glib::wrapper! {
    pub struct WebView(ObjectSubclass<imp::WebView>)
        @extends gtk::GLArea, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Default for WebView {
    fn default() -> Self {
        glib::Object::builder()
            .property("hexpand", true)
            .property("vexpand", true)
            .property("focusable", true)
            .property("can-focus", true)
            .build()
    }
}

impl WebView {
    pub fn render(&self, frame: Frame) {
        self.imp().frames.push(frame);
    }

    pub fn connect_resized<T: Fn(i32, i32) + 'static>(&self, callback: T) {
        self.connect_resize(move |_, width, height| {
            callback(width, height);
        });
    }

    pub fn connect_motion<T: Fn(Rc<PointerState>) + 'static>(&self, callback: T) {
        let callback = Rc::new(callback);

        let event_controller_motion = EventControllerMotion::new();

        let motion_callback = callback.clone();
        let pointer_state = self.imp().pointer_state.clone();
        event_controller_motion.connect_motion(move |_, x, y| {
            pointer_state.set_over(true);
            pointer_state.set_position(x, y);

            motion_callback(pointer_state.clone());
        });

        let leave_callback = callback.clone();
        let pointer_state = self.imp().pointer_state.clone();
        event_controller_motion.connect_leave(move |_| {
            pointer_state.set_over(false);

            leave_callback(pointer_state.clone())
        });

        self.add_controller(event_controller_motion);
    }

    pub fn connect_scroll<T: Fn(Rc<PointerState>, f64, f64) + 'static>(&self, callback: T) {
        let pointer_state = self.imp().pointer_state.clone();

        let flags = EventControllerScrollFlags::BOTH_AXES | EventControllerScrollFlags::KINETIC;
        let event_controller_motion = EventControllerScroll::new(flags);

        event_controller_motion.connect_scroll(move |controller, delta_x, delta_y| {
            match controller.unit() {
                ScrollUnit::Wheel => {
                    callback(pointer_state.clone(), delta_x * -100.0, delta_y * -100.0);
                }
                ScrollUnit::Surface => {
                    callback(pointer_state.clone(), delta_x - 2.0, delta_y - 2.0);
                }
                _ => {}
            }

            Propagation::Proceed
        });

        self.add_controller(event_controller_motion);
    }

    pub fn connect_click<T: Fn(Rc<PointerState>, i32) + 'static>(&self, callback: T) {
        let callback = Rc::new(callback);
        let gesture_click = GestureClick::builder().button(0).build();

        let pressed_callback = callback.clone();
        let pressed_pointer_state = self.imp().pointer_state.clone();

        gesture_click.connect_pressed(move |gesture, count, x, y| {
            pressed_pointer_state.set_position(x, y);
            pressed_pointer_state.set_pressed(true);
            pressed_pointer_state.set_button(gesture.current_button());

            pressed_callback(pressed_pointer_state.clone(), count);
        });

        let released_callback = callback.clone();
        let released_pointer_state = self.imp().pointer_state.clone();

        gesture_click.connect_released(move |gesture, count, x, y| {
            released_pointer_state.set_position(x, y);
            released_pointer_state.set_pressed(false);
            released_pointer_state.set_button(gesture.current_button());

            released_callback(released_pointer_state.clone(), count);
        });

        self.add_controller(gesture_click);
    }

    pub fn connect_keys<T: Fn(Rc<KeyboardState>) + 'static>(&self, callback: T) {
        let callback = Rc::new(callback);
        let event_controller_key = EventControllerKey::new();

        let pressed_callback = callback.clone();
        let kayboard_state_pressed = self.imp().keyboard_state.clone();
        event_controller_key.connect_key_pressed(move |_, key, code, modifiers| {
            let character = key.to_unicode();
            kayboard_state_pressed.set_character(character);
            kayboard_state_pressed.set_pressed(true);
            kayboard_state_pressed.set_code(code);
            kayboard_state_pressed.set_modifiers(modifiers);

            pressed_callback(kayboard_state_pressed.clone());

            Propagation::Proceed
        });

        let released_callback = callback.clone();
        let kayboard_state_released = self.imp().keyboard_state.clone();
        event_controller_key.connect_key_released(move |_, key, code, modifiers| {
            let character = key.to_unicode();
            kayboard_state_released.set_character(character);
            kayboard_state_released.set_pressed(false);
            kayboard_state_released.set_code(code);
            kayboard_state_released.set_modifiers(modifiers);

            released_callback(kayboard_state_released.clone());
        });

        self.add_controller(event_controller_key);
    }

    pub fn connect_clipboard<T: Fn(String) + 'static>(&self, callback: T) {
        let callback = Rc::new(callback);
        let event_controller_key = EventControllerKey::new();

        event_controller_key.connect_key_pressed(move |_, key, _, modifiers| {
            let ctrl_modifier = modifiers.contains(ModifierType::CONTROL_MASK);

            if ctrl_modifier && key == Key::v {
                if let Some(display) = Display::default() {
                    let clipboard = display.clipboard();

                    let callback = callback.clone();
                    clipboard.read_text_async(None::<&Cancellable>, move |result| {
                        if let Ok(Some(text)) = result {
                            callback(text.to_string());
                        }
                    });
                }

                return Propagation::Stop;
            }

            Propagation::Proceed
        });

        self.add_controller(event_controller_key);
    }

    pub fn connect_file_enter<F: Fn(Rc<PointerState>, PathBuf) + 'static>(&self, callback: F) {
        if let Some(drop_target) = self.controller::<DropTarget>() {
            let callback = Rc::new(callback);
            let pointer_state = self.imp().pointer_state.clone();

            let enter_callback = callback.clone();
            drop_target.connect_enter(move |target, _, _| {
                if let Some(drop) = target.current_drop() {
                    let pointer_state = pointer_state.clone();
                    let enter_callback = enter_callback.clone();

                    drop.read_value_async(
                        FileList::static_type(),
                        Priority::DEFAULT,
                        None::<&Cancellable>,
                        move |result| {
                            if let Ok(value) = result
                                && let Ok(file_list) = value.get::<FileList>()
                            {
                                for file in file_list.files() {
                                    if let Some(path) = file.path() {
                                        enter_callback(pointer_state.clone(), path);
                                    }
                                }
                            }
                        },
                    );
                }

                DragAction::COPY
            });
        }
    }

    pub fn connect_file_leave<F: Fn() + 'static>(&self, callback: F) {
        if let Some(drop_target) = self.controller::<DropTarget>() {
            drop_target.connect_leave(move |_| {
                callback();
            });
        }
    }

    pub fn connect_file_motion<F: Fn(Rc<PointerState>) + 'static>(&self, callback: F) {
        if let Some(drop_target) = self.controller::<DropTarget>() {
            let pointer_state = self.imp().pointer_state.clone();

            drop_target.connect_motion(move |_, _, _| {
                callback(pointer_state.clone());
                DragAction::COPY
            });
        }
    }

    pub fn connect_file_drop<F: Fn(Rc<PointerState>) + 'static>(&self, callback: F) {
        if let Some(drop_target) = self.controller::<DropTarget>() {
            let pointer_state = self.imp().pointer_state.clone();

            drop_target.connect_drop(move |_, _, _, _| {
                callback(pointer_state.clone());
                true
            });
        }
    }

    fn controller<T: IsA<Object>>(&self) -> Option<T> {
        for controller in &self.observe_controllers() {
            if let Ok(controller) = controller
                && let Ok(contoller) = controller.downcast::<T>()
            {
                return Some(contoller);
            }
        }

        None
    }
}
