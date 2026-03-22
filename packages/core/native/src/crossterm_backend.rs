use crossterm::cursor::{Hide, MoveTo, SetCursorStyle, Show};
use crossterm::event::{
    DisableBracketedPaste, DisableFocusChange, EnableBracketedPaste, EnableFocusChange,
    KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use crossterm::queue;
use crossterm::style::{
    Attribute, Print, ResetColor, SetAttribute, SetBackgroundColor, SetForegroundColor,
};
use crossterm::terminal::{
    Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};

use crate::terminal_state::CursorState;

#[derive(Debug)]
pub struct CrosstermBackend;

impl Default for CrosstermBackend {
    fn default() -> Self {
        Self
    }
}

impl CrosstermBackend {
    pub fn enable_raw_mode(&self) {
        let _ = enable_raw_mode();
    }

    pub fn disable_raw_mode(&self) {
        let _ = disable_raw_mode();
    }

    pub fn append_setup(
        &self,
        out: &mut Vec<u8>,
        use_alternate_screen: bool,
        mouse_enabled: bool,
        mouse_movement: bool,
        kitty_keyboard_flags: u8,
        title: Option<&str>,
        clear_screen: bool,
        cursor: CursorState,
    ) {
        if use_alternate_screen {
            let _ = queue!(out, EnterAlternateScreen);
        }
        self.append_active_modes(
            out,
            mouse_enabled,
            mouse_movement,
            kitty_keyboard_flags,
            title,
            clear_screen,
            cursor,
        );
    }

    pub fn append_active_modes(
        &self,
        out: &mut Vec<u8>,
        mouse_enabled: bool,
        mouse_movement: bool,
        kitty_keyboard_flags: u8,
        title: Option<&str>,
        clear_screen: bool,
        cursor: CursorState,
    ) {
        let _ = queue!(out, EnableFocusChange, EnableBracketedPaste);

        if mouse_enabled {
            out.extend_from_slice(mouse_enable_sequence(mouse_movement));
        }
        if kitty_keyboard_flags != 0 {
            self.append_keyboard_flags(out, kitty_keyboard_flags);
        }
        if let Some(title) = title {
            if !title.is_empty() {
                self.append_title(out, title);
            }
        }
        if clear_screen {
            let _ = queue!(out, Clear(ClearType::All), MoveTo(0, 0));
        }
        self.append_cursor(out, cursor, 0);
    }

    pub fn append_teardown(
        &self,
        out: &mut Vec<u8>,
        leave_alternate_screen: bool,
        alternate_screen_enabled: bool,
        kitty_keyboard_flags: u8,
    ) {
        out.extend_from_slice(mouse_disable_sequence());
        if kitty_keyboard_flags != 0 {
            let _ = queue!(out, PopKeyboardEnhancementFlags);
        }
        let _ = queue!(out, DisableFocusChange, DisableBracketedPaste, Show);
        if leave_alternate_screen && alternate_screen_enabled {
            let _ = queue!(out, LeaveAlternateScreen);
        }
    }

    pub fn append_clear(&self, out: &mut Vec<u8>) {
        let _ = queue!(out, Clear(ClearType::All), MoveTo(0, 0));
    }

    pub fn append_title(&self, out: &mut Vec<u8>, title: &str) {
        out.extend_from_slice(b"\x1b]2;");
        let _ = queue!(out, Print(title));
        out.extend_from_slice(b"\x1b\\");
    }

    pub fn append_cursor(&self, out: &mut Vec<u8>, cursor: CursorState, render_offset: u32) {
        self.append_cursor_style(out, cursor.style, cursor.blinking);
        if cursor.visible {
            let row = render_offset.saturating_add(cursor.y).saturating_sub(1);
            let col = cursor.x.saturating_sub(1);
            let _ = queue!(out, MoveTo(col as u16, row as u16), Show);
        } else {
            let _ = queue!(out, Hide);
        }
    }

    pub fn append_frame_prefix(&self, out: &mut Vec<u8>) {
        let _ = queue!(out, Hide);
    }

    pub fn append_colors(&self, out: &mut Vec<u8>, fg: [u8; 3], bg: [u8; 3], attributes: u32) {
        let _ = queue!(
            out,
            SetAttribute(Attribute::Reset),
            SetForegroundColor(crossterm::style::Color::Rgb {
                r: fg[0],
                g: fg[1],
                b: fg[2]
            }),
            SetBackgroundColor(crossterm::style::Color::Rgb {
                r: bg[0],
                g: bg[1],
                b: bg[2]
            })
        );

        if attributes & (1 << 0) != 0 {
            let _ = queue!(out, SetAttribute(Attribute::Bold));
        }
        if attributes & (1 << 1) != 0 {
            let _ = queue!(out, SetAttribute(Attribute::Dim));
        }
        if attributes & (1 << 2) != 0 {
            let _ = queue!(out, SetAttribute(Attribute::Italic));
        }
        if attributes & (1 << 3) != 0 {
            let _ = queue!(out, SetAttribute(Attribute::Underlined));
        }
        if attributes & (1 << 4) != 0 {
            let _ = queue!(out, SetAttribute(Attribute::SlowBlink));
        }
        if attributes & (1 << 5) != 0 {
            let _ = queue!(out, SetAttribute(Attribute::Reverse));
        }
        if attributes & (1 << 6) != 0 {
            let _ = queue!(out, SetAttribute(Attribute::Hidden));
        }
        if attributes & (1 << 7) != 0 {
            let _ = queue!(out, SetAttribute(Attribute::CrossedOut));
        }
    }

    pub fn append_reset(&self, out: &mut Vec<u8>) {
        let _ = queue!(out, SetAttribute(Attribute::Reset), ResetColor);
    }

    fn append_keyboard_flags(&self, out: &mut Vec<u8>, flags: u8) {
        let mut enhancements = KeyboardEnhancementFlags::empty();
        if flags & 0b00001 != 0 {
            enhancements |= KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES;
        }
        if flags & 0b00010 != 0 {
            enhancements |= KeyboardEnhancementFlags::REPORT_EVENT_TYPES;
        }
        if flags & 0b00100 != 0 {
            enhancements |= KeyboardEnhancementFlags::REPORT_ALTERNATE_KEYS;
        }
        if flags & 0b01000 != 0 {
            enhancements |= KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES;
        }
        let _ = queue!(out, PushKeyboardEnhancementFlags(enhancements));
    }

    fn append_cursor_style(&self, out: &mut Vec<u8>, style: u8, blinking: bool) {
        let cursor_style = match style {
            0 => Some(if blinking {
                SetCursorStyle::BlinkingBlock
            } else {
                SetCursorStyle::SteadyBlock
            }),
            1 => Some(if blinking {
                SetCursorStyle::BlinkingBar
            } else {
                SetCursorStyle::SteadyBar
            }),
            2 => Some(if blinking {
                SetCursorStyle::BlinkingUnderScore
            } else {
                SetCursorStyle::SteadyUnderScore
            }),
            _ => None,
        };

        if let Some(cursor_style) = cursor_style {
            let _ = queue!(out, cursor_style);
        }
    }
}

fn mouse_enable_sequence(enable_movement: bool) -> &'static [u8] {
    if enable_movement {
        b"\x1b[?1003h\x1b[?1006h"
    } else {
        b"\x1b[?1002h\x1b[?1006h"
    }
}

fn mouse_disable_sequence() -> &'static [u8] {
    b"\x1b[?1000l\x1b[?1002l\x1b[?1003l\x1b[?1006l\x1b[?1015l\x1b[?1016l"
}
