use windows_sys::Win32::{
    Foundation::HWND,
    UI::{Input::KeyboardAndMouse::GetAsyncKeyState, WindowsAndMessaging::*},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Binding {
    key: u8,
    modifiers: u8,
}

impl Binding {
    pub fn parse(value: &str) -> Option<Self> {
        let mut parts = value.rsplit('+');
        let code = parts.next()?;
        let key = match code {
            "ArrowUp" => 38,
            "ArrowDown" => 40,
            "ArrowLeft" => 37,
            "ArrowRight" => 39,
            "Space" => 32,
            "Tab" => 9,
            "Enter" | "NumpadEnter" => 13,
            "Backspace" => 8,
            "Delete" => 46,
            "Insert" => 45,
            "Home" => 36,
            "End" => 35,
            "PageUp" => 33,
            "PageDown" => 34,
            "Backquote" => 192,
            "Minus" => 189,
            "Equal" => 187,
            "BracketLeft" => 219,
            "BracketRight" => 221,
            "Backslash" => 220,
            "Semicolon" => 186,
            "Quote" => 222,
            "Comma" => 188,
            "Period" => 190,
            "Slash" => 191,
            "NumpadAdd" => 107,
            "NumpadSubtract" => 109,
            "NumpadMultiply" => 106,
            "NumpadDivide" => 111,
            "NumpadDecimal" => 110,
            _ => match code.as_bytes() {
                [b'K', b'e', b'y', k @ b'A'..=b'Z'] => *k,
                [b'D', b'i', b'g', b'i', b't', k @ b'0'..=b'9'] => *k,
                [b'N', b'u', b'm', b'p', b'a', b'd', k @ b'0'..=b'9'] => 96 + (k - b'0'),
                _ => {
                    let number = code.strip_prefix('F')?.parse::<u8>().ok()?;
                    if !(1..=24).contains(&number) || code != format!("F{number}") {
                        return None;
                    }
                    111 + number
                }
            },
        };
        let mut modifiers = 0;
        for modifier in parts {
            let bit = match modifier {
                "Shift" => 1,
                "Ctrl" => 2,
                "Alt" => 4,
                "Super" => 8,
                _ => return None,
            };
            if modifiers & bit != 0 {
                return None;
            }
            modifiers |= bit;
        }
        Some(Self { key, modifiers })
    }

    pub fn key_down(self) -> bool {
        down(self.key)
    }
    pub fn modifiers_down(self) -> bool {
        let actual = u8::from(down(16))
            | (u8::from(down(17)) << 1)
            | (u8::from(down(18)) << 2)
            | (u8::from(down(91) || down(92)) << 3);
        self.modifiers == actual
    }
    pub fn activation_word(self) -> Option<usize> {
        (self.key == 112 && self.modifiers & 8 == 0)
            .then_some((usize::from(self.modifiers) << 8) | 112)
    }
}
fn down(key: u8) -> bool {
    unsafe { GetAsyncKeyState(i32::from(key)) < 0 }
}

/// Owned by the watcher thread, which services the activation window's messages.
pub struct Activation {
    pub hwnd: HWND,
    armed: bool,
    blocked: bool,
}
impl Activation {
    pub fn new() -> Option<Self> {
        unsafe {
            let hwnd = CreateWindowExW(
                WS_EX_TOOLWINDOW,
                windows_sys::core::w!("STATIC"),
                windows_sys::core::w!(""),
                WS_POPUP,
                -32000,
                -32000,
                1,
                1,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null(),
            );
            if hwnd.is_null() {
                return None;
            }
            ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            Some(Self {
                hwnd,
                armed: false,
                blocked: false,
            })
        }
    }
    pub fn pump(&self) {
        let mut message = MSG::default();
        unsafe {
            while PeekMessageW(&mut message, self.hwnd, 0, 0, PM_REMOVE) != 0 {
                DispatchMessageW(&message);
            }
        }
    }
    pub fn arm(&mut self, word: usize) {
        if self.armed || self.blocked {
            return;
        }
        self.armed = unsafe { SendMessageW(self.hwnd, WM_SETHOTKEY, word, 0) } == 1;
        if !self.armed {
            unsafe {
                SendMessageW(self.hwnd, WM_SETHOTKEY, 0, 0);
            }
            self.blocked = true;
        }
    }
    pub fn disarm(&mut self) {
        if self.armed {
            unsafe {
                SendMessageW(self.hwnd, WM_SETHOTKEY, 0, 0);
            }
            self.armed = false;
        }
    }
}
impl Drop for Activation {
    fn drop(&mut self) {
        self.disarm();
        unsafe {
            DestroyWindow(self.hwnd);
        }
    }
}

#[test]
fn activation_words_follow_the_configured_shortcut() {
    assert_eq!(
        Binding::parse("Ctrl+Shift+F1").unwrap().activation_word(),
        Some(0x370)
    );
    assert_eq!(Binding::parse("Super+F1").unwrap().activation_word(), None);
    assert_eq!(Binding::parse("Alt+Numpad7").unwrap().key, 103);
    assert_eq!(Binding::parse("Ctrl+KeyG").unwrap().key, b'G');
    assert!(Binding::parse("F25").is_none());
    assert!(Binding::parse("Ctrl+Ctrl+F1").is_none());
}
