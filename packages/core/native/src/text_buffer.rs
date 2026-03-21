use std::{collections::HashMap, fs};

use crate::syntax_style::SyntaxStyleState;
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

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct ExternalHighlight {
    pub start: u32,
    pub end: u32,
    pub style_id: u32,
    pub priority: u8,
    pub hl_ref: u16,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct StyleSpan {
    pub col: u32,
    pub style_id: u32,
    pub next_col: u32,
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
    syntax_style: Option<*const SyntaxStyleState>,
    line_highlights: Vec<Vec<ExternalHighlight>>,
    line_spans: Vec<Vec<StyleSpan>>,
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
            syntax_style: None,
            line_highlights: Vec::new(),
            line_spans: Vec::new(),
            tab_width: 2,
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
        self.syntax_style = None;
        self.clear_all_highlights();
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

    pub fn default_fg(&self) -> Option<Rgba> {
        self.default_fg
    }

    pub fn default_bg(&self) -> Option<Rgba> {
        self.default_bg
    }

    pub fn default_attributes(&self) -> Option<u32> {
        self.default_attributes
    }

    pub fn set_syntax_style(&mut self, style: Option<*const SyntaxStyleState>) {
        self.syntax_style = style;
    }

    pub fn syntax_style(&self) -> Option<&SyntaxStyleState> {
        self.syntax_style.map(|ptr| unsafe { &*ptr })
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
        let width = width.max(2);
        self.tab_width = if width % 2 == 0 {
            width
        } else {
            width.saturating_add(1)
        };
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
        if chunks.is_empty() {
            self.clear();
            self.clear_all_highlights();
            return;
        }

        let mut text = String::new();
        let mut pending_highlights = Vec::new();

        for (index, chunk) in chunks.iter().enumerate() {
            if chunk.text_ptr.is_null() || chunk.text_len == 0 {
                continue;
            }

            let bytes = unsafe { std::slice::from_raw_parts(chunk.text_ptr, chunk.text_len) };
            let normalized = normalize_text_bytes(bytes);
            if normalized.is_empty() {
                continue;
            }

            let start = text_width(&text, self.tab_width);
            let end = start.saturating_add(text_width(&normalized, self.tab_width));
            if start < end {
                if let Some(style_id) = self.resolve_chunk_style_id(index, chunk) {
                    pending_highlights.push((start, end, style_id));
                }
            }

            text.push_str(&normalized);
        }

        self.text = text;
        self.clear_all_highlights();

        for (start, end, style_id) in pending_highlights {
            self.add_highlight_by_char_range(start, end, style_id, 1, 0);
        }
    }

    pub fn plain_text_bytes(&self) -> &[u8] {
        self.text.as_bytes()
    }

    pub fn text_range(&self, start_offset: u32, end_offset: u32) -> String {
        if start_offset >= end_offset {
            return String::new();
        }

        let total_weight = text_weight(&self.text, self.tab_width);
        if start_offset >= total_weight {
            return String::new();
        }

        let clamped_end = end_offset.min(total_weight);
        let mut output = String::new();
        let mut col_offset = 0_u32;
        let lines: Vec<&str> = if self.text.is_empty() {
            Vec::new()
        } else {
            self.text.split('\n').collect()
        };

        for (line_idx, line) in lines.iter().enumerate() {
            let line_width = text_width(line, self.tab_width);
            let line_start = col_offset;
            let line_end = line_start.saturating_add(line_width);
            let mut line_had_content = false;

            if line_end > start_offset && line_start < clamped_end {
                let local_start = start_offset.saturating_sub(line_start);
                let local_end = clamped_end.saturating_sub(line_start).min(line_width);
                let selected = slice_by_display_offsets(line, self.tab_width, local_start, local_end);
                if !selected.is_empty() {
                    output.push_str(&selected);
                    line_had_content = true;
                }
            }

            if line_had_content
                && line_idx + 1 < lines.len()
                && line_end.saturating_add(1) < clamped_end
            {
                output.push('\n');
            }

            col_offset = line_end.saturating_add(1);
        }

        output
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

    pub fn add_highlight(
        &mut self,
        line_idx: usize,
        col_start: u32,
        col_end: u32,
        style_id: u32,
        priority: u8,
        hl_ref: u16,
    ) {
        if line_idx >= self.line_count() as usize || col_start >= col_end {
            return;
        }

        if self.line_highlights.len() <= line_idx {
            self.line_highlights.resize_with(line_idx + 1, Vec::new);
        }
        if self.line_spans.len() <= line_idx {
            self.line_spans.resize_with(line_idx + 1, Vec::new);
        }

        self.line_highlights[line_idx].push(ExternalHighlight {
            start: col_start,
            end: col_end,
            style_id,
            priority,
            hl_ref,
        });
        self.rebuild_line_spans(line_idx);
    }

    pub fn add_highlight_by_char_range(
        &mut self,
        char_start: u32,
        char_end: u32,
        style_id: u32,
        priority: u8,
        hl_ref: u16,
    ) {
        if char_start >= char_end || self.text.is_empty() {
            return;
        }

        let mut pending = Vec::new();
        let mut line_start = 0_u32;
        for (line_idx, line) in self.text.split('\n').enumerate() {
            let line_width = text_width(line, self.tab_width);
            let line_end = line_start.saturating_add(line_width);
            if line_end > char_start && line_start < char_end {
                pending.push((
                    line_idx,
                    char_start.saturating_sub(line_start),
                    char_end.min(line_end).saturating_sub(line_start),
                    style_id,
                    priority,
                    hl_ref,
                ));
            }
            line_start = line_end;
        }

        for (line_idx, start, end, style_id, priority, hl_ref) in pending {
            self.add_highlight(line_idx, start, end, style_id, priority, hl_ref);
        }
    }

    pub fn remove_highlights_by_ref(&mut self, hl_ref: u16) {
        for line_idx in 0..self.line_highlights.len() {
            let line = &mut self.line_highlights[line_idx];
            let before = line.len();
            line.retain(|hl| hl.hl_ref != hl_ref);
            if line.len() != before {
                self.rebuild_line_spans(line_idx);
            }
        }
    }

    pub fn clear_line_highlights(&mut self, line_idx: usize) {
        if let Some(line) = self.line_highlights.get_mut(line_idx) {
            line.clear();
        }
        if let Some(line) = self.line_spans.get_mut(line_idx) {
            line.clear();
        }
    }

    pub fn clear_all_highlights(&mut self) {
        for line in &mut self.line_highlights {
            line.clear();
        }
        for line in &mut self.line_spans {
            line.clear();
        }
    }

    pub fn get_line_highlights(&self, line_idx: usize) -> &[ExternalHighlight] {
        self.line_highlights
            .get(line_idx)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    pub fn get_line_spans(&self, line_idx: usize) -> &[StyleSpan] {
        self.line_spans
            .get(line_idx)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    pub fn get_highlight_count(&self) -> u32 {
        self.line_highlights.iter().fold(0_u32, |count, line| {
            count.saturating_add(u32::try_from(line.len()).unwrap_or(u32::MAX))
        })
    }

    pub(crate) fn set_text_bytes(&mut self, data: &[u8]) {
        self.text = normalize_text_bytes(data);
    }

    pub(crate) fn text_str(&self) -> &str {
        &self.text
    }

    pub(crate) fn insert_text_at_offset(&mut self, offset: u32, data: &[u8]) -> u32 {
        let insert = normalize_text_bytes(data);
        if insert.is_empty() {
            return offset;
        }

        let byte_index = weight_to_byte_index(&self.text, self.tab_width, offset);
        self.text.insert_str(byte_index, &insert);
        offset.saturating_add(text_weight(&insert, self.tab_width))
    }

    pub(crate) fn delete_range_by_offsets(&mut self, start_offset: u32, end_offset: u32) -> u32 {
        if start_offset == end_offset {
            return start_offset;
        }

        let start = start_offset.min(end_offset);
        let end = start_offset.max(end_offset);
        let start_byte = weight_to_byte_index(&self.text, self.tab_width, start);
        let end_byte = weight_to_byte_index(&self.text, self.tab_width, end);

        if start_byte >= end_byte || start_byte >= self.text.len() {
            return start;
        }

        self.text
            .replace_range(start_byte..end_byte.min(self.text.len()), "");
        start
    }

    fn resolve_chunk_style_id(&mut self, index: usize, chunk: &StyledChunk) -> Option<u32> {
        let style = unsafe { &mut *(self.syntax_style? as *const SyntaxStyleState as *mut SyntaxStyleState) };
        let fg = rgba_from_ptr(chunk.fg_ptr);
        let bg = rgba_from_ptr(chunk.bg_ptr);
        let attributes = chunk.attributes;

        if let Some(style_id) = style.resolve_by_definition(fg, bg, attributes) {
            return Some(style_id);
        }

        let name = format!("chunk{index}");
        Some(style.register_style(name.as_bytes(), fg, bg, attributes))
    }

    fn rebuild_line_spans(&mut self, line_idx: usize) {
        if line_idx >= self.line_spans.len() {
            self.line_spans.resize_with(line_idx + 1, Vec::new);
        }

        let spans = &mut self.line_spans[line_idx];
        spans.clear();

        let Some(highlights) = self.line_highlights.get(line_idx) else {
            return;
        };
        if highlights.is_empty() {
            return;
        }

        #[derive(Clone, Copy)]
        struct Event {
            col: u32,
            is_start: bool,
            hl_idx: usize,
        }

        let mut events = Vec::with_capacity(highlights.len().saturating_mul(2));
        for (hl_idx, highlight) in highlights.iter().enumerate() {
            events.push(Event {
                col: highlight.start,
                is_start: true,
                hl_idx,
            });
            events.push(Event {
                col: highlight.end,
                is_start: false,
                hl_idx,
            });
        }

        events.sort_by(|left, right| {
            left.col
                .cmp(&right.col)
                .then_with(|| right.is_start.cmp(&left.is_start))
                .then_with(|| left.hl_idx.cmp(&right.hl_idx))
        });

        let mut active: Vec<usize> = Vec::new();
        let mut current_col = 0_u32;

        for event in events {
            let mut current_style = 0_u32;
            let mut current_priority = -1_i16;

            for active_idx in &active {
                let highlight = highlights[*active_idx];
                if i16::from(highlight.priority) > current_priority {
                    current_priority = i16::from(highlight.priority);
                    current_style = highlight.style_id;
                }
            }

            if event.col > current_col {
                spans.push(StyleSpan {
                    col: current_col,
                    style_id: current_style,
                    next_col: event.col,
                });
                current_col = event.col;
            }

            if event.is_start {
                active.push(event.hl_idx);
            } else if let Some(position) = active.iter().position(|value| *value == event.hl_idx) {
                active.swap_remove(position);
            }
        }

        let line_width = self
            .text
            .split('\n')
            .nth(line_idx)
            .map(|line| text_width(line, self.tab_width))
            .unwrap_or(0);
        if current_col < line_width {
            spans.push(StyleSpan {
                col: current_col,
                style_id: 0,
                next_col: line_width,
            });
        }
    }
}

fn rgba_from_ptr(ptr: *const f32) -> Option<Rgba> {
    (!ptr.is_null()).then(|| unsafe { std::ptr::read_unaligned(ptr.cast::<Rgba>()) })
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

pub(crate) fn text_weight(text: &str, tab_width: u8) -> u32 {
    let mut weight = 0_u32;
    for ch in text.chars() {
        weight = weight.saturating_add(char_weight(ch, tab_width));
    }
    weight
}

pub(crate) fn weight_to_byte_index(text: &str, tab_width: u8, target: u32) -> usize {
    let mut weight = 0_u32;
    for (index, ch) in text.char_indices() {
        if weight == target {
            return index;
        }
        weight = weight.saturating_add(char_weight(ch, tab_width));
    }
    text.len()
}

pub(crate) fn previous_offset(text: &str, tab_width: u8, target: u32) -> u32 {
    let mut weight = 0_u32;
    let mut previous = 0_u32;
    for ch in text.chars() {
        if weight >= target {
            return previous;
        }
        previous = weight;
        weight = weight.saturating_add(char_weight(ch, tab_width));
    }
    previous
}

pub(crate) fn next_offset(text: &str, tab_width: u8, target: u32) -> u32 {
    let total = text_weight(text, tab_width);
    if target >= total {
        return total;
    }

    let mut weight = 0_u32;
    for ch in text.chars() {
        let next = weight.saturating_add(char_weight(ch, tab_width));
        if weight == target {
            return next;
        }
        if target < next {
            return next;
        }
        weight = next;
    }
    total
}

pub(crate) fn offset_to_position(text: &str, tab_width: u8, target: u32) -> Option<(u32, u32)> {
    let total = text_weight(text, tab_width);
    if target > total {
        return None;
    }

    let mut row = 0_u32;
    let mut col = 0_u32;
    let mut offset = 0_u32;

    for ch in text.chars() {
        if offset == target {
            return Some((row, col));
        }

        match ch {
            '\r' => {}
            '\n' => {
                offset = offset.saturating_add(1);
                row = row.saturating_add(1);
                col = 0;
            }
            '\t' => {
                let width = u32::from(tab_width.max(1));
                offset = offset.saturating_add(width);
                col = col.saturating_add(width);
            }
            _ => {
                let width = ch.width().unwrap_or(0) as u32;
                offset = offset.saturating_add(width);
                col = col.saturating_add(width);
            }
        }
    }

    (offset == target).then_some((row, col))
}

pub(crate) fn position_to_offset(text: &str, tab_width: u8, row: u32, col: u32) -> Option<u32> {
    let mut current_row = 0_u32;
    let mut current_col = 0_u32;
    let mut offset = 0_u32;

    if row == 0 && col == 0 {
        return Some(0);
    }

    for ch in text.chars() {
        if current_row == row && current_col == col {
            return Some(offset);
        }

        match ch {
            '\r' => {}
            '\n' => {
                offset = offset.saturating_add(1);
                current_row = current_row.saturating_add(1);
                current_col = 0;
            }
            '\t' => {
                let width = u32::from(tab_width.max(1));
                offset = offset.saturating_add(width);
                current_col = current_col.saturating_add(width);
            }
            _ => {
                let width = ch.width().unwrap_or(0) as u32;
                offset = offset.saturating_add(width);
                current_col = current_col.saturating_add(width);
            }
        }
    }

    (current_row == row && current_col == col).then_some(offset)
}

pub(crate) fn line_start_offset(text: &str, tab_width: u8, row: u32) -> Option<u32> {
    if row == 0 {
        return Some(0);
    }

    let mut current_row = 0_u32;
    let mut offset = 0_u32;

    for ch in text.chars() {
        match ch {
            '\r' => {}
            '\n' => {
                offset = offset.saturating_add(1);
                current_row = current_row.saturating_add(1);
                if current_row == row {
                    return Some(offset);
                }
            }
            _ => {
                offset = offset.saturating_add(char_weight(ch, tab_width));
            }
        }
    }

    None
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

pub(crate) fn char_weight(ch: char, tab_width: u8) -> u32 {
    match ch {
        '\r' => 0,
        '\n' => 1,
        '\t' => u32::from(tab_width.max(1)),
        _ => ch.width().unwrap_or(0) as u32,
    }
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
    use super::{
        StyledChunk, TextBufferState, copy_bytes_to_out, line_start_offset, next_offset,
        offset_to_position, position_to_offset, previous_offset, text_width,
    };
    use crate::syntax_style::SyntaxStyleState;

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

    #[test]
    fn offset_and_position_helpers_round_trip() {
        let text = "Hello\n世界";
        assert_eq!(offset_to_position(text, 4, 0), Some((0, 0)));
        assert_eq!(offset_to_position(text, 4, 6), Some((1, 0)));
        assert_eq!(position_to_offset(text, 4, 1, 0), Some(6));
        assert_eq!(line_start_offset(text, 4, 1), Some(6));
        assert_eq!(previous_offset(text, 4, 6), 5);
        assert_eq!(next_offset(text, 4, 6), 8);
    }

    #[test]
    fn styled_text_reuses_matching_syntax_style_ids() {
        let mut syntax_style = SyntaxStyleState::default();
        let keyword_id = syntax_style.register_style(b"keyword", Some([1.0, 0.0, 0.0, 1.0]), None, 1);

        let mut buffer = TextBufferState::new(0);
        buffer.set_syntax_style(Some(&syntax_style));

        let fg = [1.0, 0.0, 0.0, 1.0];
        let text = b"const value";
        let chunks = [StyledChunk {
            text_ptr: text.as_ptr(),
            text_len: text.len(),
            fg_ptr: fg.as_ptr(),
            bg_ptr: core::ptr::null(),
            attributes: 1,
            link_ptr: core::ptr::null(),
            link_len: 0,
        }];

        buffer.set_styled_text(&chunks);

        let highlights = buffer.get_line_highlights(0);
        assert_eq!(highlights.len(), 1);
        assert_eq!(highlights[0].style_id, keyword_id);
        assert_eq!(highlights[0].end, 11);
    }
}
