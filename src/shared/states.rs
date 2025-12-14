use std::cell::Cell;

use cef::sys::{
    cef_event_flags_t as EventFlags, cef_key_event_type_t::KEYEVENT_KEYDOWN,
    cef_key_event_type_t::KEYEVENT_KEYUP,
};
use cef::{KeyEvent, MouseEvent};
use gtk::gdk::ModifierType;

#[derive(Default, Debug)]
pub struct PointerState {
    position: Cell<(f64, f64)>,
    pressed: Cell<bool>,
    button: Cell<u32>,
    over: Cell<bool>,
}

impl PointerState {
    pub fn position(&self) -> (f64, f64) {
        self.position.get()
    }

    pub fn set_position(&self, x: f64, y: f64) {
        self.position.set((x, y));
    }

    pub fn pressed(&self) -> bool {
        self.pressed.get()
    }

    pub fn set_pressed(&self, pressed: bool) {
        self.pressed.set(pressed);
    }

    pub fn button(&self) -> u32 {
        self.button.get()
    }

    pub fn set_button(&self, r#type: u32) {
        self.button.set(r#type);
    }

    pub fn over(&self) -> bool {
        self.over.get()
    }

    pub fn set_over(&self, state: bool) {
        self.over.set(state);
    }
}

impl From<&PointerState> for MouseEvent {
    fn from(pointer_state: &PointerState) -> Self {
        let (x, y) = pointer_state.position();
        let pressed = pointer_state.pressed();
        let button = pointer_state.button();

        MouseEvent {
            x: x as i32,
            y: y as i32,
            modifiers: match button {
                1 if pressed => EventFlags::EVENTFLAG_LEFT_MOUSE_BUTTON.0,
                3 if pressed => EventFlags::EVENTFLAG_MIDDLE_MOUSE_BUTTON.0,
                2 if pressed => EventFlags::EVENTFLAG_RIGHT_MOUSE_BUTTON.0,
                _ => 0,
            },
        }
    }
}

enum KeyCode {
    None,
    Backspace,
    Tab,
    Enter,
    Control,
    Escape,
    Space,
    PageUp,
    PageDown,
    End,
    Home,
    ArrowLeft,
    ArrowUp,
    ArrowRight,
    ArrowDown,
    Equal,
    Minus,
    A,
    C,
    V,
    X,
}

impl KeyCode {
    fn windows(&self) -> u32 {
        match self {
            KeyCode::Backspace => 8,
            KeyCode::Tab => 9,
            KeyCode::Enter => 13,
            KeyCode::Control => 17,
            KeyCode::Escape => 27,
            KeyCode::Space => 32,
            KeyCode::PageUp => 33,
            KeyCode::PageDown => 34,
            KeyCode::End => 35,
            KeyCode::Home => 36,
            KeyCode::ArrowLeft => 37,
            KeyCode::ArrowUp => 38,
            KeyCode::ArrowRight => 39,
            KeyCode::ArrowDown => 40,
            KeyCode::Equal => 61,
            KeyCode::A => 65,
            KeyCode::C => 67,
            KeyCode::V => 86,
            KeyCode::X => 88,
            KeyCode::Minus => 173,
            KeyCode::None => 0,
        }
    }
}

impl From<u32> for KeyCode {
    fn from(value: u32) -> Self {
        match value {
            22 => KeyCode::Backspace,
            23 => KeyCode::Tab,
            36 => KeyCode::Enter,
            37 => KeyCode::Control,
            9 => KeyCode::Escape,
            65 => KeyCode::Space,
            112 => KeyCode::PageUp,
            117 => KeyCode::PageDown,
            115 => KeyCode::End,
            110 => KeyCode::Home,
            113 => KeyCode::ArrowLeft,
            111 => KeyCode::ArrowUp,
            114 => KeyCode::ArrowRight,
            116 => KeyCode::ArrowDown,
            21 => KeyCode::Equal,
            38 => KeyCode::A,
            54 => KeyCode::C,
            55 => KeyCode::V,
            53 => KeyCode::X,
            20 => KeyCode::Minus,
            _ => KeyCode::None,
        }
    }
}

#[derive(Default, Debug)]
pub struct KeyboardState {
    character: Cell<Option<char>>,
    pressed: Cell<bool>,
    code: Cell<u32>,
    control_modifier: Cell<bool>,
    shift_modifier: Cell<bool>,
}

impl KeyboardState {
    pub fn character(&self) -> Option<char> {
        self.character.get()
    }

    pub fn set_character(&self, character: Option<char>) {
        self.character.set(character);
    }

    pub fn pressed(&self) -> bool {
        self.pressed.get()
    }

    pub fn set_pressed(&self, pressed: bool) {
        self.pressed.set(pressed);
    }

    pub fn code(&self) -> u32 {
        self.code.get()
    }

    pub fn windows_code(&self) -> u32 {
        let code = self.code.get();
        let native_code = KeyCode::from(code);
        native_code.windows()
    }

    pub fn set_code(&self, code: u32) {
        self.code.set(code);
    }

    pub fn control_modifier(&self) -> bool {
        self.control_modifier.get()
    }

    pub fn shift_modifier(&self) -> bool {
        self.shift_modifier.get()
    }

    pub fn set_modifiers(&self, modifiers: ModifierType) {
        let control_modifier = modifiers.contains(ModifierType::CONTROL_MASK);
        self.control_modifier.set(control_modifier);

        let shift_modifier = modifiers.contains(ModifierType::SHIFT_MASK);
        self.shift_modifier.set(shift_modifier);
    }
}

impl From<&KeyboardState> for KeyEvent {
    fn from(keyboard_state: &KeyboardState) -> Self {
        let pressed = keyboard_state.pressed();
        let code = keyboard_state.code();
        let windows_code = keyboard_state.windows_code();

        let event_type = match pressed {
            true => KEYEVENT_KEYDOWN.into(),
            false => KEYEVENT_KEYUP.into(),
        };

        let mut modifiers = EventFlags::EVENTFLAG_NONE.0;

        if keyboard_state.shift_modifier() {
            modifiers |= EventFlags::EVENTFLAG_SHIFT_DOWN.0;
        }

        if keyboard_state.control_modifier() {
            modifiers |= EventFlags::EVENTFLAG_CONTROL_DOWN.0;
        }

        KeyEvent {
            type_: event_type,
            native_key_code: code as i32,
            windows_key_code: windows_code as i32,
            modifiers,
            ..Default::default()
        }
    }
}
