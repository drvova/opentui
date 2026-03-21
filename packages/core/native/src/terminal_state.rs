use std::collections::HashMap;

pub type Rgba = [f32; 4];

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct CursorState {
    pub x: u32,
    pub y: u32,
    pub visible: bool,
    pub style: u8,
    pub blinking: bool,
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct CursorStyleOptions {
    pub style: u8,
    pub blinking: u8,
    pub color: *const f32,
    pub cursor: u8,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct TerminalCapabilitiesOut {
    pub kitty_keyboard: bool,
    pub kitty_graphics: bool,
    pub rgb: bool,
    pub unicode: u8,
    pub sgr_pixels: bool,
    pub color_scheme_updates: bool,
    pub explicit_width: bool,
    pub scaled_text: bool,
    pub sixel: bool,
    pub focus_tracking: bool,
    pub sync: bool,
    pub bracketed_paste: bool,
    pub hyperlinks: bool,
    pub osc52: bool,
    pub explicit_cursor_positioning: bool,
    pub term_name_ptr: *const u8,
    pub term_name_len: usize,
    pub term_version_ptr: *const u8,
    pub term_version_len: usize,
    pub term_from_xtversion: bool,
}

#[derive(Clone, Debug)]
pub struct TerminalCapabilities {
    pub kitty_keyboard: bool,
    pub kitty_graphics: bool,
    pub rgb: bool,
    pub unicode: u8,
    pub sgr_pixels: bool,
    pub color_scheme_updates: bool,
    pub explicit_width: bool,
    pub scaled_text: bool,
    pub sixel: bool,
    pub focus_tracking: bool,
    pub sync: bool,
    pub bracketed_paste: bool,
    pub hyperlinks: bool,
    pub osc52: bool,
    pub explicit_cursor_positioning: bool,
    pub term_name: Vec<u8>,
    pub term_version: Vec<u8>,
    pub term_from_xtversion: bool,
}

impl Default for TerminalCapabilities {
    fn default() -> Self {
        Self {
            kitty_keyboard: false,
            kitty_graphics: false,
            rgb: true,
            unicode: 1,
            sgr_pixels: false,
            color_scheme_updates: false,
            explicit_width: false,
            scaled_text: false,
            sixel: false,
            focus_tracking: false,
            sync: false,
            bracketed_paste: false,
            hyperlinks: false,
            osc52: true,
            explicit_cursor_positioning: true,
            term_name: b"rust-terminal".to_vec(),
            term_version: b"0.1".to_vec(),
            term_from_xtversion: false,
        }
    }
}

#[derive(Debug, Default)]
pub struct TerminalState {
    cursor: CursorState,
    capabilities: TerminalCapabilities,
    kitty_keyboard_flags: u8,
    mouse_enabled: bool,
    mouse_movement: bool,
    title: String,
    env: HashMap<Vec<u8>, Vec<u8>>,
    clipboard: HashMap<u8, Vec<u8>>,
    writes: Vec<u8>,
    alternate_screen: bool,
    suspended: bool,
}

impl TerminalState {
    pub fn set_cursor_position(&mut self, x: i32, y: i32, visible: bool) {
        self.cursor.x = x.max(1) as u32;
        self.cursor.y = y.max(1) as u32;
        self.cursor.visible = visible;
    }

    pub fn set_cursor_color(&mut self, color: Rgba) {
        self.cursor.r = color[0];
        self.cursor.g = color[1];
        self.cursor.b = color[2];
        self.cursor.a = color[3];
    }

    pub fn set_cursor_style_options(&mut self, options: CursorStyleOptions) {
        if options.style != u8::MAX {
            self.cursor.style = options.style;
        }
        if options.blinking != u8::MAX {
            self.cursor.blinking = options.blinking != 0;
        }
        if !options.color.is_null() {
            let color = unsafe { std::slice::from_raw_parts(options.color, 4) };
            self.set_cursor_color([color[0], color[1], color[2], color[3]]);
        }
    }

    pub fn cursor_state(&self) -> CursorState {
        self.cursor
    }

    pub fn set_kitty_keyboard_flags(&mut self, flags: u8) {
        self.kitty_keyboard_flags = flags;
        self.capabilities.kitty_keyboard = flags != 0;
    }

    pub fn kitty_keyboard_flags(&self) -> u8 {
        self.kitty_keyboard_flags
    }

    pub fn enable_kitty_keyboard(&mut self, flags: u8) {
        self.set_kitty_keyboard_flags(flags);
    }

    pub fn disable_kitty_keyboard(&mut self) {
        self.set_kitty_keyboard_flags(0);
    }

    pub fn enable_mouse(&mut self, enable_movement: bool) {
        self.mouse_enabled = true;
        self.mouse_movement = enable_movement;
    }

    pub fn disable_mouse(&mut self) {
        self.mouse_enabled = false;
        self.mouse_movement = false;
    }

    pub fn set_terminal_title(&mut self, title: &[u8]) {
        self.title = String::from_utf8_lossy(title).into_owned();
    }

    pub fn set_terminal_env_var(&mut self, key: &[u8], value: &[u8]) -> bool {
        self.env.insert(key.to_vec(), value.to_vec());
        true
    }

    pub fn setup_terminal(&mut self, use_alternate_screen: bool) {
        self.alternate_screen = use_alternate_screen;
        self.suspended = false;
    }

    pub fn suspend(&mut self) {
        self.suspended = true;
    }

    pub fn resume(&mut self) {
        self.suspended = false;
    }

    pub fn restore_terminal_modes(&mut self) {
        self.suspended = false;
        self.mouse_enabled = false;
        self.mouse_movement = false;
        self.kitty_keyboard_flags = 0;
        self.capabilities.kitty_keyboard = false;
    }

    pub fn clear_terminal(&mut self) {
        self.writes.clear();
    }

    pub fn write_out(&mut self, data: &[u8]) {
        self.writes.extend_from_slice(data);
    }

    pub fn copy_to_clipboard_osc52(&mut self, target: u8, payload: &[u8]) -> bool {
        self.clipboard.insert(target, payload.to_vec());
        true
    }

    pub fn clear_clipboard_osc52(&mut self, target: u8) -> bool {
        self.clipboard.remove(&target);
        true
    }

    pub fn process_capability_response(&mut self, response: &[u8]) {
        let text = String::from_utf8_lossy(response);
        if text.contains("kitty") {
            self.capabilities.kitty_keyboard = true;
            self.capabilities.kitty_graphics = true;
            self.capabilities.term_name = b"kitty".to_vec();
        }
        if text.contains("sixel") {
            self.capabilities.sixel = true;
        }
        if text.contains("sync") {
            self.capabilities.sync = true;
        }
    }

    pub fn query_pixel_resolution(&mut self) {
        self.capabilities.sgr_pixels = true;
    }

    pub fn set_hyperlinks_capability(&mut self, enabled: bool) {
        self.capabilities.hyperlinks = enabled;
    }

    pub fn capabilities_out(&self) -> TerminalCapabilitiesOut {
        TerminalCapabilitiesOut {
            kitty_keyboard: self.capabilities.kitty_keyboard,
            kitty_graphics: self.capabilities.kitty_graphics,
            rgb: self.capabilities.rgb,
            unicode: self.capabilities.unicode,
            sgr_pixels: self.capabilities.sgr_pixels,
            color_scheme_updates: self.capabilities.color_scheme_updates,
            explicit_width: self.capabilities.explicit_width,
            scaled_text: self.capabilities.scaled_text,
            sixel: self.capabilities.sixel,
            focus_tracking: self.capabilities.focus_tracking,
            sync: self.capabilities.sync,
            bracketed_paste: self.capabilities.bracketed_paste,
            hyperlinks: self.capabilities.hyperlinks,
            osc52: self.capabilities.osc52,
            explicit_cursor_positioning: self.capabilities.explicit_cursor_positioning,
            term_name_ptr: self.capabilities.term_name.as_ptr(),
            term_name_len: self.capabilities.term_name.len(),
            term_version_ptr: self.capabilities.term_version.as_ptr(),
            term_version_len: self.capabilities.term_version.len(),
            term_from_xtversion: self.capabilities.term_from_xtversion,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{CursorStyleOptions, TerminalState};

    #[test]
    fn cursor_and_capability_state_round_trip() {
        let mut terminal = TerminalState::default();
        terminal.set_cursor_position(4, 2, true);
        terminal.set_cursor_color([1.0, 0.0, 0.0, 1.0]);
        terminal.set_cursor_style_options(CursorStyleOptions {
            style: 1,
            blinking: 1,
            color: core::ptr::null(),
            cursor: 0,
        });
        terminal.enable_kitty_keyboard(0b1010);
        terminal.enable_mouse(true);
        terminal.process_capability_response(b"kitty sixel sync");
        terminal.query_pixel_resolution();
        terminal.set_terminal_title(b"title");
        terminal.set_terminal_env_var(b"TERM", b"xterm");
        terminal.write_out(b"hello");
        terminal.copy_to_clipboard_osc52(1, b"clip");
        terminal.set_hyperlinks_capability(true);

        let cursor = terminal.cursor_state();
        assert_eq!(cursor.x, 4);
        assert_eq!(cursor.y, 2);
        assert_eq!(cursor.style, 1);
        assert!(cursor.blinking);
        assert_eq!(terminal.kitty_keyboard_flags(), 0b1010);

        let caps = terminal.capabilities_out();
        assert!(caps.kitty_keyboard);
        assert!(caps.kitty_graphics);
        assert!(caps.sixel);
        assert!(caps.sync);
        assert!(caps.sgr_pixels);
        assert!(caps.hyperlinks);
    }
}
