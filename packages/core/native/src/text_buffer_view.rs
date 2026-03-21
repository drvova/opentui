use crate::text_buffer::{TextBufferState, copy_bytes_to_out, text_width};

pub const NO_SELECTION: u64 = 0xffff_ffff_ffff_ffff;

#[derive(Debug)]
pub struct TextBufferViewState {
    text_buffer: *mut TextBufferState,
    selection: Option<(u32, u32)>,
    wrap_width: Option<u32>,
    wrap_mode: u8,
    viewport_x: u32,
    viewport_y: u32,
    viewport_width: u32,
    viewport_height: u32,
}

impl TextBufferViewState {
    pub fn new(text_buffer: *mut TextBufferState) -> Self {
        Self {
            text_buffer,
            selection: None,
            wrap_width: None,
            wrap_mode: 0,
            viewport_x: 0,
            viewport_y: 0,
            viewport_width: 0,
            viewport_height: 0,
        }
    }

    pub fn set_selection(&mut self, start: u32, end: u32) {
        self.selection = normalize_selection(start, end);
    }

    pub fn update_selection(&mut self, end: u32) {
        self.selection = self
            .selection
            .and_then(|(start, _)| normalize_selection(start, end));
    }

    pub fn reset_selection(&mut self) {
        self.selection = None;
    }

    pub fn selection_info(&self) -> u64 {
        match self.selection {
            Some((start, end)) => ((start as u64) << 32) | end as u64,
            None => NO_SELECTION,
        }
    }

    pub fn set_wrap_width(&mut self, width: u32) {
        self.wrap_width = if width == 0 { None } else { Some(width) };
    }

    pub fn set_wrap_mode(&mut self, mode: u8) {
        self.wrap_mode = mode;
    }

    pub fn set_viewport_size(&mut self, width: u32, height: u32) {
        self.viewport_width = width;
        self.viewport_height = height;
    }

    pub fn set_viewport(&mut self, x: u32, y: u32, width: u32, height: u32) {
        self.viewport_x = x;
        self.viewport_y = y;
        self.viewport_width = width;
        self.viewport_height = height;
    }

    pub fn plain_text_bytes(&self) -> &[u8] {
        self.buffer().plain_text_bytes()
    }

    pub fn selected_text_bytes(&self) -> Vec<u8> {
        match self.selection {
            Some((start, end)) => self.buffer().text_range(start, end).into_bytes(),
            None => Vec::new(),
        }
    }

    pub fn virtual_line_count(&self) -> u32 {
        let text = std::str::from_utf8(self.buffer().plain_text_bytes()).unwrap_or("");
        if text.is_empty() {
            return 1;
        }

        let wrap_width = match self.wrap_width {
            Some(width) if width > 0 && self.wrap_mode != 0 => width,
            _ => return self.buffer().line_count().max(1),
        };

        let tab_width = self.buffer().tab_width();
        let mut count = 0_u32;
        for line in text.split('\n') {
            let width = text_width(line, tab_width);
            let segments = if width == 0 {
                1
            } else {
                (width + wrap_width - 1) / wrap_width
            };
            count = count.saturating_add(segments.max(1));
        }
        count.max(1)
    }

    fn buffer(&self) -> &TextBufferState {
        assert!(
            !self.text_buffer.is_null(),
            "TextBufferViewState requires a valid TextBufferState"
        );
        unsafe { &*self.text_buffer }
    }
}

fn normalize_selection(start: u32, end: u32) -> Option<(u32, u32)> {
    if start >= end {
        None
    } else {
        Some((start, end))
    }
}

pub fn copy_selected_text(view: &TextBufferViewState, out_ptr: *mut u8, max_len: usize) -> usize {
    let data = view.selected_text_bytes();
    copy_bytes_to_out(&data, out_ptr, max_len)
}

#[cfg(test)]
mod tests {
    use super::{NO_SELECTION, TextBufferViewState};
    use crate::text_buffer::TextBufferState;

    #[test]
    fn selection_info_packs_and_resets() {
        let mut buffer = TextBufferState::new(0);
        let id = buffer.register_mem_buffer(b"Hello\nWorld").unwrap();
        buffer.set_text_from_mem(id);

        let mut view = TextBufferViewState::new(&mut buffer);
        assert_eq!(view.selection_info(), NO_SELECTION);

        view.set_selection(6, 11);
        assert_eq!(view.selection_info(), ((6_u64) << 32) | 11_u64);
        assert_eq!(view.selected_text_bytes(), b"World");

        view.reset_selection();
        assert_eq!(view.selection_info(), NO_SELECTION);
    }

    #[test]
    fn virtual_line_count_respects_wrap_settings() {
        let mut buffer = TextBufferState::new(0);
        let id = buffer
            .register_mem_buffer(b"ABCDEFGHIJKLMNOPQRSTUVWXYZ")
            .unwrap();
        buffer.set_text_from_mem(id);

        let mut view = TextBufferViewState::new(&mut buffer);
        assert_eq!(view.virtual_line_count(), 1);

        view.set_wrap_mode(1);
        view.set_wrap_width(10);
        assert_eq!(view.virtual_line_count(), 3);
    }
}
