use std::{collections::HashMap, fs};

use unicode_width::UnicodeWidthChar;

pub type Rgba = [f32; 4];

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct StyledChunk {
    pub text_ptr: *const u8,
    pub text_len: usize,
    pub fg_ptr: *const f32,
    pub bg_ptr: *const f32,
    pub attributes: u32,
    pub link_ptr: *const u8,
    pub link_len: usize,
}

#[derive(Debug)]
pub struct TextBufferState {
    width_method: u8,
    text: String,
    mem_registry: HashMap<u8, Vec<u8>>,
    next_mem_id: u8,
    default_fg: Option<Rgba>,
    default_bg: Option<Rgba>,
    default_attributes: Option<u32>,
    tab_width: u8,
}

impl TextBufferState {
    pub fn new(width_method: u8) -> Self {
        Self {
            width_method,
            text: String::new(),
            mem_registry: HashMap::new(),
            next_mem_id: 0,
            default_fg: None,
            default_bg: None,
            default_attributes: None,
            tab_width: 4,
        }
    }

    pub fn length(&self) -> u32 {
        text_width(&self.text, self.tab_width)
    }

    pub fn byte_size(&self) -> u32 {
        u32::try_from(self.text.len()).unwrap_or(u32::MAX)
    }

    pub fn line_count(&self) -> u32 {
        if self.text.is_empty() {
            return 0;
        }
        u32::try_from(self.text.split('\n').count()).unwrap_or(u32::MAX)
    }

    pub fn clear(&mut self) {
        self.text.clear();
    }

    pub fn reset(&mut self) {
        self.clear();
        self.mem_registry.clear();
        self.next_mem_id = 0;
        self.default_fg = None;
        self.default_bg = None;
        self.default_attributes = None;
    }

    pub fn set_default_fg(&mut self, fg: Option<Rgba>) {
        self.default_fg = fg;
    }

    pub fn set_default_bg(&mut self, bg: Option<Rgba>) {
        self.default_bg = bg;
    }

    pub fn set_default_attributes(&mut self, attributes: Option<u32>) {
        self.default_attributes = attributes;
    }

    pub fn reset_defaults(&mut self) {
        self.default_fg = None;
        self.default_bg = None;
        self.default_attributes = None;
    }

    pub fn tab_width(&self) -> u8 {
        self.tab_width
    }

    pub fn set_tab_width(&mut self, width: u8) {
        self.tab_width = width.max(1);
    }

    pub fn register_mem_buffer(&mut self, data: &[u8]) -> Result<u8, ()> {
        if self.mem_registry.len() >= 255 {
            return Err(());
        }

        for _ in 0..=u8::MAX {
            let candidate = self.next_mem_id;
            self.next_mem_id = self.next_mem_id.wrapping_add(1);
            if let std::collections::hash_map::Entry::Vacant(entry) =
                self.mem_registry.entry(candidate)
            {
                entry.insert(data.to_vec());
                return Ok(candidate);
            }
        }

        Err(())
    }

    pub fn replace_mem_buffer(&mut self, mem_id: u8, data: &[u8]) -> bool {
        if let Some(slot) = self.mem_registry.get_mut(&mem_id) {
            *slot = data.to_vec();
            return true;
        }
        false
    }

    pub fn clear_mem_registry(&mut self) {
        self.mem_registry.clear();
        self.next_mem_id = 0;
    }

    pub fn set_text_from_mem(&mut self, mem_id: u8) {
        if let Some(data) = self.mem_registry.get(&mem_id) {
            self.text = normalize_text_bytes(data);
        }
    }

    pub fn append_bytes(&mut self, data: &[u8]) {
        if data.is_empty() {
            return;
        }
        self.text.push_str(&normalize_text_bytes(data));
    }

    pub fn append_from_mem(&mut self, mem_id: u8) {
        if let Some(data) = self.mem_registry.get(&mem_id).cloned() {
            self.append_bytes(&data);
        }
    }

    pub fn load_file(&mut self, path: &[u8]) -> bool {
        let Ok(path) = std::str::from_utf8(path) else {
            return false;
        };
        let Ok(bytes) = fs::read(path) else {
            return false;
        };
        self.text = normalize_text_bytes(&bytes);
        true
    }

    pub fn set_styled_text(&mut self, chunks: &[StyledChunk]) {
        let mut text = String::new();
        for chunk in chunks {
            if chunk.text_ptr.is_null() || chunk.text_len == 0 {
                continue;
            }
            let bytes = unsafe { std::slice::from_raw_parts(chunk.text_ptr, chunk.text_len) };
            text.push_str(&normalize_text_bytes(bytes));
        }
        self.text = text;
    }

    pub fn plain_text_bytes(&self) -> &[u8] {
        self.text.as_bytes()
    }

    pub fn text_range(&self, start_offset: u32, end_offset: u32) -> String {
        if start_offset >= end_offset {
            return String::new();
        }
        slice_by_weight_offsets(&self.text, self.tab_width, start_offset, end_offset)
    }

    pub fn text_range_by_coords(
        &self,
        start_row: u32,
        start_col: u32,
        end_row: u32,
        end_col: u32,
    ) -> String {
        let lines: Vec<&str> = if self.text.is_empty() {
            Vec::new()
        } else {
            self.text.split('\n').collect()
        };

        let start_row = usize::try_from(start_row).unwrap_or(usize::MAX);
        let end_row = usize::try_from(end_row).unwrap_or(usize::MAX);
        if start_row >= lines.len() || end_row >= lines.len() || start_row > end_row {
            return String::new();
        }

        let mut parts = Vec::new();
        for row in start_row..=end_row {
            let line = lines[row];
            let line_start = if row == start_row { start_col } else { 0 };
            let line_end = if row == end_row {
                end_col
            } else {
                text_width(line, self.tab_width)
            };
            parts.push(slice_by_display_offsets(
                line,
                self.tab_width,
                line_start,
                line_end,
            ));
        }
        parts.join("\n")
    }

    pub fn width_method(&self) -> u8 {
        self.width_method
    }
}

fn normalize_text_bytes(data: &[u8]) -> String {
    let text = String::from_utf8_lossy(data);
    let mut normalized = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\r' {
            if chars.peek() == Some(&'\n') {
                let _ = chars.next();
            }
            normalized.push('\n');
        } else {
            normalized.push(ch);
        }
    }
    normalized
}

pub(crate) fn text_width(text: &str, tab_width: u8) -> u32 {
    let mut width = 0_u32;
    for ch in text.chars() {
        match ch {
            '\n' | '\r' => {}
            '\t' => width = width.saturating_add(u32::from(tab_width.max(1))),
            _ => width = width.saturating_add(ch.width().unwrap_or(0) as u32),
        }
    }
    width
}

fn slice_by_display_offsets(
    text: &str,
    tab_width: u8,
    start_offset: u32,
    end_offset: u32,
) -> String {
    if start_offset >= end_offset {
        return String::new();
    }

    let mut width = 0_u32;
    let mut start_byte = None;
    let mut end_byte = text.len();

    for (index, ch) in text.char_indices() {
        let char_width = match ch {
            '\n' | '\r' => 0,
            '\t' => u32::from(tab_width.max(1)),
            _ => ch.width().unwrap_or(0) as u32,
        };

        if start_byte.is_none() && width >= start_offset {
            start_byte = Some(index);
        }
        if width >= end_offset {
            end_byte = index;
            break;
        }

        width = width.saturating_add(char_width);
    }

    let start_byte = start_byte.unwrap_or_else(|| if start_offset == 0 { 0 } else { text.len() });
    if width < end_offset {
        end_byte = text.len();
    }

    text[start_byte..end_byte].to_string()
}

fn slice_by_weight_offsets(
    text: &str,
    tab_width: u8,
    start_offset: u32,
    end_offset: u32,
) -> String {
    if start_offset >= end_offset {
        return String::new();
    }

    let mut weight = 0_u32;
    let mut start_byte = None;
    let mut end_byte = text.len();

    for (index, ch) in text.char_indices() {
        let char_weight = match ch {
            '\r' => 0,
            '\n' => 1,
            '\t' => u32::from(tab_width.max(1)),
            _ => ch.width().unwrap_or(0) as u32,
        };

        if start_byte.is_none() && weight >= start_offset {
            start_byte = Some(index);
        }
        if weight >= end_offset {
            end_byte = index;
            break;
        }

        weight = weight.saturating_add(char_weight);
    }

    let start_byte = start_byte.unwrap_or_else(|| if start_offset == 0 { 0 } else { text.len() });
    if weight < end_offset {
        end_byte = text.len();
    }

    text[start_byte..end_byte].to_string()
}

pub fn copy_bytes_to_out(source: &[u8], out_ptr: *mut u8, max_len: usize) -> usize {
    if out_ptr.is_null() || max_len == 0 {
        return 0;
    }
    let len = source.len().min(max_len);
    unsafe {
        std::ptr::copy_nonoverlapping(source.as_ptr(), out_ptr, len);
    }
    len
}

#[cfg(test)]
mod tests {
    use super::{TextBufferState, copy_bytes_to_out, text_width};

    #[test]
    fn width_counts_unicode_cells_and_ignores_newlines() {
        assert_eq!(text_width("Hello 世界 🌟", 4), 13);
        assert_eq!(text_width("Line 1\nLine 2\nLine 3", 4), 18);
    }

    #[test]
    fn mem_buffer_set_and_append_normalizes_crlf() {
        let mut buffer = TextBufferState::new(0);
        let id = buffer.register_mem_buffer(b"Line1\r\nLine2").unwrap();
        buffer.set_text_from_mem(id);
        buffer.append_bytes(b"\r\nLine3");

        assert_eq!(buffer.plain_text_bytes(), b"Line1\nLine2\nLine3");
        assert_eq!(buffer.line_count(), 3);
    }

    #[test]
    fn text_range_uses_weight_offsets_and_coords() {
        let mut buffer = TextBufferState::new(0);
        let id = buffer
            .register_mem_buffer("Hello\n世界".as_bytes())
            .unwrap();
        buffer.set_text_from_mem(id);

        assert_eq!(buffer.text_range(0, 5), "Hello");
        assert_eq!(buffer.text_range(6, 10), "世界");
        assert_eq!(buffer.text_range_by_coords(1, 0, 1, 4), "世界");
    }

    #[test]
    fn copy_bytes_clips_to_output() {
        let mut out = [0_u8; 4];
        let written = copy_bytes_to_out(b"abcdef", out.as_mut_ptr(), out.len());
        assert_eq!(written, 4);
        assert_eq!(&out, b"abcd");
    }
}
