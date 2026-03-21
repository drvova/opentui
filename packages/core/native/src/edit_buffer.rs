use std::sync::atomic::{AtomicU16, Ordering};

use crate::text_buffer::{
    TextBufferState, line_start_offset, next_offset, offset_to_position, position_to_offset,
    previous_offset, text_weight, text_width,
};

static NEXT_EDIT_BUFFER_ID: AtomicU16 = AtomicU16::new(1);

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LogicalCursor {
    pub row: u32,
    pub col: u32,
    pub offset: u32,
}

#[derive(Debug)]
pub struct EditBufferState {
    id: u16,
    text_buffer: Box<TextBufferState>,
    cursor_offset: u32,
}

impl EditBufferState {
    pub fn new(width_method: u8) -> Self {
        Self {
            id: NEXT_EDIT_BUFFER_ID.fetch_add(1, Ordering::Relaxed),
            text_buffer: Box::new(TextBufferState::new(width_method)),
            cursor_offset: 0,
        }
    }

    pub fn id(&self) -> u16 {
        self.id
    }

    pub fn text_buffer_ptr(&mut self) -> *mut TextBufferState {
        self.text_buffer.as_mut() as *mut TextBufferState
    }

    pub fn set_text_bytes(&mut self, data: &[u8]) {
        self.text_buffer.set_text_bytes(data);
        self.cursor_offset = 0;
    }

    pub fn set_text_from_mem(&mut self, mem_id: u8) {
        self.text_buffer.set_text_from_mem(mem_id);
        self.cursor_offset = 0;
    }

    pub fn replace_text_bytes(&mut self, data: &[u8]) {
        self.set_text_bytes(data);
    }

    pub fn replace_text_from_mem(&mut self, mem_id: u8) {
        self.set_text_from_mem(mem_id);
    }

    pub fn text_bytes(&self) -> &[u8] {
        self.text_buffer.plain_text_bytes()
    }

    pub fn clear(&mut self) {
        self.text_buffer.clear();
        self.cursor_offset = 0;
    }

    pub fn cursor(&self) -> LogicalCursor {
        let (row, col) = offset_to_position(
            self.text_buffer.text_str(),
            self.text_buffer.tab_width(),
            self.cursor_offset,
        )
        .unwrap_or((0, 0));

        LogicalCursor {
            row,
            col,
            offset: self.cursor_offset,
        }
    }

    pub fn set_cursor_to_line_col(&mut self, row: u32, col: u32) {
        if let Some(offset) = position_to_offset(
            self.text_buffer.text_str(),
            self.text_buffer.tab_width(),
            row,
            col,
        ) {
            self.cursor_offset = offset;
        }
    }

    pub fn set_cursor_by_offset(&mut self, offset: u32) {
        let max = text_weight(self.text_buffer.text_str(), self.text_buffer.tab_width());
        self.cursor_offset = offset.min(max);
    }

    pub fn move_cursor_left(&mut self) {
        self.cursor_offset = previous_offset(
            self.text_buffer.text_str(),
            self.text_buffer.tab_width(),
            self.cursor_offset,
        );
    }

    pub fn move_cursor_right(&mut self) {
        self.cursor_offset = next_offset(
            self.text_buffer.text_str(),
            self.text_buffer.tab_width(),
            self.cursor_offset,
        );
    }

    pub fn move_cursor_up(&mut self) {
        let cursor = self.cursor();
        if cursor.row == 0 {
            return;
        }

        let target_row = cursor.row - 1;
        let max_col = line_width_at(
            self.text_buffer.text_str(),
            self.text_buffer.tab_width(),
            target_row,
        );
        self.set_cursor_to_line_col(target_row, cursor.col.min(max_col));
    }

    pub fn move_cursor_down(&mut self) {
        let cursor = self.cursor();
        let line_count = self.text_buffer.line_count();
        if line_count == 0 || cursor.row + 1 >= line_count {
            return;
        }

        let target_row = cursor.row + 1;
        let max_col = line_width_at(
            self.text_buffer.text_str(),
            self.text_buffer.tab_width(),
            target_row,
        );
        self.set_cursor_to_line_col(target_row, cursor.col.min(max_col));
    }

    pub fn goto_line(&mut self, line: u32) {
        if line == 0
            || line_start_offset(
                self.text_buffer.text_str(),
                self.text_buffer.tab_width(),
                line,
            )
            .is_some()
        {
            self.set_cursor_to_line_col(line, 0);
        }
    }

    pub fn insert_text(&mut self, data: &[u8]) {
        self.cursor_offset = self
            .text_buffer
            .insert_text_at_offset(self.cursor_offset, data);
    }

    pub fn insert_char(&mut self, data: &[u8]) {
        self.insert_text(data);
    }

    pub fn new_line(&mut self) {
        self.insert_text(b"\n");
    }

    pub fn delete_char_backward(&mut self) {
        let previous = previous_offset(
            self.text_buffer.text_str(),
            self.text_buffer.tab_width(),
            self.cursor_offset,
        );
        self.cursor_offset = self
            .text_buffer
            .delete_range_by_offsets(previous, self.cursor_offset);
    }

    pub fn delete_char(&mut self) {
        let next = next_offset(
            self.text_buffer.text_str(),
            self.text_buffer.tab_width(),
            self.cursor_offset,
        );
        self.cursor_offset = self
            .text_buffer
            .delete_range_by_offsets(self.cursor_offset, next);
    }

    pub fn delete_range_by_coords(
        &mut self,
        start_row: u32,
        start_col: u32,
        end_row: u32,
        end_col: u32,
    ) {
        let start = position_to_offset(
            self.text_buffer.text_str(),
            self.text_buffer.tab_width(),
            start_row,
            start_col,
        )
        .unwrap_or(0);
        let end = position_to_offset(
            self.text_buffer.text_str(),
            self.text_buffer.tab_width(),
            end_row,
            end_col,
        )
        .unwrap_or(start);
        self.cursor_offset = self.text_buffer.delete_range_by_offsets(start, end);
    }

    pub fn offset_to_position(&self, offset: u32) -> Option<LogicalCursor> {
        offset_to_position(
            self.text_buffer.text_str(),
            self.text_buffer.tab_width(),
            offset,
        )
        .map(|(row, col)| LogicalCursor { row, col, offset })
    }

    pub fn position_to_offset(&self, row: u32, col: u32) -> u32 {
        position_to_offset(
            self.text_buffer.text_str(),
            self.text_buffer.tab_width(),
            row,
            col,
        )
        .unwrap_or(0)
    }

    pub fn line_start_offset(&self, row: u32) -> u32 {
        line_start_offset(
            self.text_buffer.text_str(),
            self.text_buffer.tab_width(),
            row,
        )
        .unwrap_or(0)
    }

    pub fn text_buffer_text_range(&self, start_offset: u32, end_offset: u32) -> String {
        self.text_buffer.text_range(start_offset, end_offset)
    }

    pub fn text_buffer_text_range_by_coords(
        &self,
        start_row: u32,
        start_col: u32,
        end_row: u32,
        end_col: u32,
    ) -> String {
        self.text_buffer
            .text_range_by_coords(start_row, start_col, end_row, end_col)
    }
}

fn line_width_at(text: &str, tab_width: u8, row: u32) -> u32 {
    let lines: Vec<&str> = if text.is_empty() {
        Vec::new()
    } else {
        text.split('\n').collect()
    };

    let row = usize::try_from(row).unwrap_or(usize::MAX);
    if row >= lines.len() {
        return 0;
    }

    text_width(lines[row], tab_width)
}

#[cfg(test)]
mod tests {
    use super::{EditBufferState, LogicalCursor};

    #[test]
    fn set_text_and_cursor_round_trip() {
        let mut buffer = EditBufferState::new(0);
        buffer.set_text_bytes(b"Hello\nWorld");

        assert_eq!(
            buffer.cursor(),
            LogicalCursor {
                row: 0,
                col: 0,
                offset: 0
            }
        );

        buffer.set_cursor_to_line_col(1, 0);
        assert_eq!(buffer.cursor().offset, 6);
        assert_eq!(String::from_utf8_lossy(buffer.text_bytes()), "Hello\nWorld");
    }

    #[test]
    fn insert_and_delete_basic_text() {
        let mut buffer = EditBufferState::new(0);
        buffer.set_text_bytes(b"Hello");
        buffer.set_cursor_to_line_col(0, 5);
        buffer.insert_text(b" World");
        assert_eq!(String::from_utf8_lossy(buffer.text_bytes()), "Hello World");

        buffer.delete_char_backward();
        assert_eq!(String::from_utf8_lossy(buffer.text_bytes()), "Hello Worl");

        buffer.delete_char();
        assert_eq!(String::from_utf8_lossy(buffer.text_bytes()), "Hello Worl");
    }

    #[test]
    fn move_between_lines_preserves_column_when_possible() {
        let mut buffer = EditBufferState::new(0);
        buffer.set_text_bytes(b"Line 1\nLine 22\nL3");
        buffer.set_cursor_to_line_col(1, 4);
        buffer.move_cursor_down();
        assert_eq!(buffer.cursor().row, 2);
        assert_eq!(buffer.cursor().col, 2);
        buffer.move_cursor_up();
        assert_eq!(buffer.cursor().row, 1);
    }
}
