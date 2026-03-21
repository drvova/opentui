use std::collections::HashMap;
use std::io::{self, Write};

use crate::crossterm_backend::CrosstermBackend;
use crate::optimized_buffer::OptimizedBuffer;

pub type Rgba = [f32; 4];

#[repr(C)]
#[derive(Clone, Copy, Debug)]
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

impl Default for CursorState {
    fn default() -> Self {
        Self {
            x: 0,
            y: 0,
            visible: false,
            style: 3,
            blinking: false,
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.0,
        }
    }
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
            osc52: false,
            explicit_cursor_positioning: true,
            term_name: b"rust-terminal".to_vec(),
            term_version: b"0.1".to_vec(),
            term_from_xtversion: false,
        }
    }
}

fn parse_xtversion(text: &str) -> Option<(&str, &str)> {
    let prefix = "\x1bP>|";
    let suffix = "\x1b\\";
    let body = text.strip_prefix(prefix)?.strip_suffix(suffix)?;
    let open = body.find('(')?;
    let close = body.rfind(')')?;
    (open < close).then_some((&body[..open], &body[open + 1..close]))
}

fn parse_decrpm(text: &str) -> Option<(u32, u32)> {
    let body = text.strip_prefix("\x1b[?")?.strip_suffix("$y")?;
    let mut parts = body.split(';');
    let mode = parts.next()?.parse().ok()?;
    let value = parts.next()?.parse().ok()?;
    Some((mode, value))
}

fn parse_cpr(text: &str) -> Option<u32> {
    let body = text.strip_prefix("\x1b[1;")?.strip_suffix('R')?;
    body.parse().ok()
}

#[derive(Debug)]
pub struct TerminalState {
    backend: CrosstermBackend,
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
    terminal_ready: bool,
    stdout_passthrough: bool,
}

impl Default for TerminalState {
    fn default() -> Self {
        Self {
            backend: CrosstermBackend::default(),
            cursor: CursorState::default(),
            capabilities: TerminalCapabilities::default(),
            kitty_keyboard_flags: 0,
            mouse_enabled: false,
            mouse_movement: false,
            title: String::new(),
            env: HashMap::new(),
            clipboard: HashMap::new(),
            writes: Vec::new(),
            alternate_screen: false,
            suspended: false,
            terminal_ready: false,
            stdout_passthrough: false,
        }
    }
}

impl TerminalState {
    pub fn set_stdout_passthrough(&mut self, enabled: bool) {
        self.stdout_passthrough = enabled;
    }

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
        if self.terminal_ready && !self.suspended {
            let mut out = Vec::new();
            self.backend.append_active_modes(
                &mut out,
                false,
                false,
                flags,
                None,
                false,
                CursorState::default(),
            );
            self.emit(&out);
        }
    }

    pub fn disable_kitty_keyboard(&mut self) {
        if self.terminal_ready && !self.suspended && self.kitty_keyboard_flags != 0 {
            let mut out = Vec::new();
            self.backend
                .append_teardown(&mut out, false, false, self.kitty_keyboard_flags);
            self.emit(&out);
        }
        self.set_kitty_keyboard_flags(0);
    }

    pub fn enable_mouse(&mut self, enable_movement: bool) {
        self.mouse_enabled = true;
        self.mouse_movement = enable_movement;
        if self.terminal_ready && !self.suspended {
            let mut out = Vec::new();
            self.backend.append_active_modes(
                &mut out,
                true,
                enable_movement,
                0,
                None,
                false,
                CursorState::default(),
            );
            self.emit(&out);
        }
    }

    pub fn disable_mouse(&mut self) {
        self.mouse_enabled = false;
        self.mouse_movement = false;
        if self.terminal_ready && !self.suspended {
            let mut out = Vec::new();
            self.backend.append_teardown(&mut out, false, false, 0);
            self.emit(&out);
        }
    }

    pub fn set_terminal_title(&mut self, title: &[u8]) {
        self.title = String::from_utf8_lossy(title).into_owned();
        if self.terminal_ready && !self.suspended {
            let mut out = Vec::new();
            self.backend.append_title(&mut out, self.title.as_str());
            self.emit(&out);
        }
    }

    pub fn set_terminal_env_var(&mut self, key: &[u8], value: &[u8]) -> bool {
        self.env.insert(key.to_vec(), value.to_vec());
        true
    }

    pub fn setup_terminal(&mut self, use_alternate_screen: bool) {
        self.alternate_screen = use_alternate_screen;
        self.suspended = false;
        self.terminal_ready = true;
        self.emit_setup_sequence(true);
    }

    pub fn suspend(&mut self) {
        if self.terminal_ready && !self.suspended {
            self.emit_teardown_sequence(false);
        }
        self.suspended = true;
    }

    pub fn resume(&mut self) {
        if self.terminal_ready && self.suspended {
            self.emit_setup_sequence(false);
        }
        self.suspended = false;
    }

    pub fn restore_terminal_modes(&mut self) {
        if !self.terminal_ready || self.suspended {
            return;
        }
        self.emit_active_modes(false);
    }

    pub fn clear_terminal(&mut self) {
        self.writes.clear();
        if self.terminal_ready && !self.suspended {
            let mut out = Vec::new();
            self.backend.append_clear(&mut out);
            self.emit(&out);
        }
    }

    pub fn write_out(&mut self, data: &[u8]) {
        self.emit(data);
    }

    pub fn teardown_terminal(&mut self) {
        if !self.terminal_ready {
            return;
        }
        self.emit_teardown_sequence(true);
        self.terminal_ready = false;
        self.suspended = false;
    }

    pub fn copy_to_clipboard_osc52(&mut self, target: u8, payload: &[u8]) -> bool {
        self.clipboard.insert(target, payload.to_vec());
        if self.terminal_ready && !self.suspended {
            let mut out = Vec::new();
            append_osc52_sequence(&mut out, target, payload);
            self.emit(&out);
        }
        true
    }

    pub fn clear_clipboard_osc52(&mut self, target: u8) -> bool {
        self.clipboard.remove(&target);
        if self.terminal_ready && !self.suspended {
            let mut out = Vec::new();
            append_osc52_sequence(&mut out, target, b"");
            self.emit(&out);
        }
        true
    }

    pub fn process_capability_response(&mut self, response: &[u8]) {
        let text = String::from_utf8_lossy(response);
        if let Some((name, version)) = parse_xtversion(&text) {
            self.capabilities.term_name = name.as_bytes().to_vec();
            self.capabilities.term_version = version.as_bytes().to_vec();
            self.capabilities.term_from_xtversion = true;
        }

        if text.contains("kitty") {
            self.capabilities.kitty_keyboard = true;
            self.capabilities.kitty_graphics = true;
            self.capabilities.osc52 = true;
        }
        if text.contains("sixel") {
            self.capabilities.sixel = true;
        }
        if text.contains("sync") {
            self.capabilities.sync = true;
        }
        if let Some((mode, value)) = parse_decrpm(&text) {
            let enabled = value == 1 || value == 2;
            match mode {
                1004 => self.capabilities.focus_tracking = enabled,
                1016 => self.capabilities.sgr_pixels = enabled,
                2004 => self.capabilities.bracketed_paste = enabled,
                2026 => self.capabilities.sync = enabled,
                2027 => {
                    if enabled {
                        self.capabilities.unicode = 1;
                    }
                }
                2031 => self.capabilities.color_scheme_updates = enabled,
                _ => {}
            }
        }

        if let Some(column) = parse_cpr(&text) {
            if column >= 2 {
                if self.capabilities.explicit_width {
                    self.capabilities.scaled_text = column >= 3;
                } else {
                    self.capabilities.explicit_width = true;
                }
            }
        }

        if text.contains("\x1b[?0u") || text.contains("\x1b[?1u") {
            self.capabilities.kitty_keyboard = true;
        }
    }

    pub fn query_pixel_resolution(&mut self) {
        self.capabilities.sgr_pixels = true;
        if self.terminal_ready && !self.suspended {
            self.emit(b"\x1b[14t");
        }
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

    pub fn render_frame(&mut self, buffer: &OptimizedBuffer, render_offset: u32) {
        if !self.terminal_ready || self.suspended {
            return;
        }

        let mut out = Vec::new();
        self.backend.append_frame_prefix(&mut out);
        buffer.write_ansi_frame(&mut out, render_offset.saturating_add(1), &self.backend);
        self.append_cursor_sequence(&mut out, render_offset);
        self.emit(&out);
    }

    #[cfg(test)]
    pub(crate) fn captured_writes(&self) -> &[u8] {
        &self.writes
    }

    fn emit_setup_sequence(&mut self, clear_screen: bool) {
        let mut out = Vec::new();
        self.backend.append_setup(
            &mut out,
            self.alternate_screen,
            self.mouse_enabled,
            self.mouse_movement,
            self.kitty_keyboard_flags,
            (!self.title.is_empty()).then_some(self.title.as_str()),
            clear_screen,
            self.cursor,
        );
        append_cursor_color_sequence(
            &mut out,
            [self.cursor.r, self.cursor.g, self.cursor.b, self.cursor.a],
        );
        self.emit(&out);
    }

    fn emit_active_modes(&mut self, clear_screen: bool) {
        let mut out = Vec::new();
        self.emit_active_modes_into(&mut out, clear_screen);
        self.emit(&out);
    }

    fn emit_active_modes_into(&self, out: &mut Vec<u8>, clear_screen: bool) {
        self.backend.append_active_modes(
            out,
            self.mouse_enabled,
            self.mouse_movement,
            self.kitty_keyboard_flags,
            (!self.title.is_empty()).then_some(self.title.as_str()),
            clear_screen,
            self.cursor,
        );
        append_cursor_color_sequence(
            out,
            [self.cursor.r, self.cursor.g, self.cursor.b, self.cursor.a],
        );
    }

    fn emit_teardown_sequence(&mut self, leave_alt_screen: bool) {
        let mut out = Vec::new();
        self.backend.append_teardown(
            &mut out,
            leave_alt_screen,
            self.alternate_screen,
            self.kitty_keyboard_flags,
        );
        self.emit(&out);
    }

    fn append_cursor_sequence(&self, out: &mut Vec<u8>, render_offset: u32) {
        self.backend.append_cursor(out, self.cursor, render_offset);
        if self.cursor.visible {
            append_cursor_color_sequence(
                out,
                [self.cursor.r, self.cursor.g, self.cursor.b, self.cursor.a],
            );
        }
    }

    fn emit(&mut self, data: &[u8]) {
        if data.is_empty() {
            return;
        }

        self.writes.extend_from_slice(data);
        if !self.stdout_passthrough {
            return;
        }

        let mut stdout = io::stdout().lock();
        if let Err(error) = stdout.write_all(data).and_then(|_| stdout.flush()) {
            eprintln!("opentui native stdout flush failed: {error}");
            self.stdout_passthrough = false;
        }
    }
}

fn append_cursor_color_sequence(out: &mut Vec<u8>, color: Rgba) {
    if color[3] <= 0.0 {
        return;
    }

    let [r, g, b] = color_to_rgb(color);
    out.extend_from_slice(format!("\x1b]12;#{r:02x}{g:02x}{b:02x}\x1b\\").as_bytes());
}

fn append_osc52_sequence(out: &mut Vec<u8>, target: u8, payload: &[u8]) {
    let selector = match target {
        1 => b"p".as_slice(),
        2 => b"q".as_slice(),
        3 => b"?".as_slice(),
        _ => b"c".as_slice(),
    };

    out.extend_from_slice(b"\x1b]52;");
    out.extend_from_slice(selector);
    out.extend_from_slice(b";");
    out.extend_from_slice(payload);
    out.extend_from_slice(b"\x1b\\");
}

fn color_to_rgb(color: Rgba) -> [u8; 3] {
    [
        (color[0].clamp(0.0, 1.0) * 255.0).round() as u8,
        (color[1].clamp(0.0, 1.0) * 255.0).round() as u8,
        (color[2].clamp(0.0, 1.0) * 255.0).round() as u8,
    ]
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

    #[test]
    fn setup_and_restore_emit_terminal_sequences() {
        let mut terminal = TerminalState::default();
        terminal.set_terminal_title(b"OpenTUI");
        terminal.enable_mouse(true);
        terminal.enable_kitty_keyboard(0b111);
        terminal.setup_terminal(true);
        terminal.restore_terminal_modes();
        terminal.teardown_terminal();

        let output = String::from_utf8_lossy(terminal.captured_writes());
        assert!(output.contains("\x1b[?1049h"));
        assert!(output.contains("\x1b[?1004h"));
        assert!(output.contains("\x1b[?2004h"));
        assert!(output.contains("\x1b[>7u"));
        assert!(output.contains("\x1b[?1003h"));
        assert!(output.contains("\x1b]2;OpenTUI\x1b\\"));
        assert!(output.contains("\x1b[?1049l"));
    }
}
