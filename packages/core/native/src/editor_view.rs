use crate::{
    edit_buffer::{EditBufferState, LogicalCursor},
    text_buffer::StyledChunk,
    text_buffer_view::{LineInfoOut, TextBufferViewState},
};

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VisualCursor {
    pub visual_row: u32,
    pub visual_col: u32,
    pub logical_row: u32,
    pub logical_col: u32,
    pub offset: u32,
}

#[derive(Debug)]
pub struct EditorViewState {
    edit_buffer: *mut EditBufferState,
    text_buffer_view: Box<TextBufferViewState>,
    viewport_x: u32,
    viewport_y: u32,
    viewport_width: u32,
    viewport_height: u32,
    scroll_margin: f32,
    placeholder_text: String,
}

impl EditorViewState {
    pub fn new(
        edit_buffer: *mut EditBufferState,
        viewport_width: u32,
        viewport_height: u32,
    ) -> Self {
        let text_buffer = unsafe { (&mut *edit_buffer).text_buffer_ptr() };
        let mut text_buffer_view = Box::new(TextBufferViewState::new(text_buffer));
        text_buffer_view.set_viewport_size(viewport_width, viewport_height);

        Self {
            edit_buffer,
            text_buffer_view,
            viewport_x: 0,
            viewport_y: 0,
            viewport_width,
            viewport_height,
            scroll_margin: 0.0,
            placeholder_text: String::new(),
        }
    }

    pub fn text_buffer_view_ptr(&mut self) -> *mut TextBufferViewState {
        self.text_buffer_view.as_mut() as *mut TextBufferViewState
    }

    pub fn line_info(&mut self) -> LineInfoOut {
        self.text_buffer_view.line_info()
    }

    pub fn logical_line_info(&mut self) -> LineInfoOut {
        self.text_buffer_view.logical_line_info()
    }

    pub fn set_viewport_size(&mut self, width: u32, height: u32) {
        self.viewport_width = width;
        self.viewport_height = height;
        self.text_buffer_view.set_viewport_size(width, height);
        if width > 0 {
            self.text_buffer_view.set_wrap_width(width);
        }
    }

    pub fn set_viewport(&mut self, x: u32, y: u32, width: u32, height: u32) {
        self.viewport_x = x;
        self.viewport_y = y;
        self.viewport_width = width;
        self.viewport_height = height;
        self.text_buffer_view.set_viewport(x, y, width, height);
        if width > 0 {
            self.text_buffer_view.set_wrap_width(width);
        }
    }

    pub fn viewport(&self) -> (u32, u32, u32, u32) {
        (
            self.viewport_x,
            self.viewport_y,
            self.viewport_width,
            self.viewport_height,
        )
    }

    pub fn set_scroll_margin(&mut self, margin: f32) {
        self.scroll_margin = margin;
    }

    pub fn set_wrap_mode(&mut self, mode: u8) {
        self.text_buffer_view.set_wrap_mode(mode);
        self.text_buffer_view
            .set_wrap_width(if mode == 0 { 0 } else { self.viewport_width });
    }

    pub fn virtual_line_count(&self) -> u32 {
        self.text_buffer_view.virtual_line_count()
    }

    pub fn total_virtual_line_count(&self) -> u32 {
        self.virtual_line_count()
    }

    pub fn set_selection(&mut self, start: u32, end: u32) {
        self.text_buffer_view.set_selection(start, end);
    }

    pub fn set_local_selection(
        &mut self,
        anchor_x: i32,
        anchor_y: i32,
        focus_x: i32,
        focus_y: i32,
    ) -> bool {
        self.text_buffer_view
            .set_local_selection(anchor_x, anchor_y, focus_x, focus_y)
    }

    pub fn update_selection(&mut self, end: u32) {
        self.text_buffer_view.update_selection(end);
    }

    pub fn update_local_selection(
        &mut self,
        anchor_x: i32,
        anchor_y: i32,
        focus_x: i32,
        focus_y: i32,
    ) -> bool {
        self.text_buffer_view
            .update_local_selection(anchor_x, anchor_y, focus_x, focus_y)
    }

    pub fn reset_selection(&mut self) {
        self.text_buffer_view.reset_selection();
    }

    pub fn reset_local_selection(&mut self) {
        self.text_buffer_view.reset_local_selection();
    }

    pub fn selection_info(&self) -> u64 {
        self.text_buffer_view.selection_info()
    }

    pub fn selected_text_bytes(&self) -> Vec<u8> {
        self.text_buffer_view.selected_text_bytes()
    }

    pub fn text_bytes(&self) -> &[u8] {
        self.edit_buffer().text_bytes()
    }

    pub fn set_placeholder_styled_text(&mut self, chunks: &[StyledChunk]) {
        self.placeholder_text.clear();
        for chunk in chunks {
            if chunk.text_ptr.is_null() || chunk.text_len == 0 {
                continue;
            }
            let bytes = unsafe { std::slice::from_raw_parts(chunk.text_ptr, chunk.text_len) };
            self.placeholder_text
                .push_str(&String::from_utf8_lossy(bytes));
        }
    }

    pub fn set_tab_indicator(&mut self, indicator: u32) {
        self.text_buffer_view.set_tab_indicator(indicator);
    }

    pub fn set_tab_indicator_color(&mut self, color: [f32; 4]) {
        self.text_buffer_view.set_tab_indicator_color(color);
    }

    pub fn cursor(&self) -> (u32, u32) {
        let cursor = self.edit_buffer().cursor();
        (cursor.row, cursor.col)
    }

    pub fn visual_cursor(&self) -> VisualCursor {
        let cursor = self.edit_buffer().cursor();
        let (visual_row, visual_col, logical_row, logical_col, offset) = self
            .text_buffer_view
            .visual_cursor_for_offset(cursor.offset, cursor.row, cursor.col);

        VisualCursor {
            visual_row,
            visual_col,
            logical_row,
            logical_col,
            offset,
        }
    }

    pub fn move_up_visual(&mut self) {
        let cursor = self.visual_cursor();
        if cursor.visual_row == 0 {
            return;
        }
        if let Some(offset) = self
            .text_buffer_view
            .offset_for_visual_position(cursor.visual_row - 1, cursor.visual_col)
        {
            self.edit_buffer_mut().set_cursor_by_offset(offset);
        }
    }

    pub fn move_down_visual(&mut self) {
        let cursor = self.visual_cursor();
        let next_row = cursor.visual_row.saturating_add(1);
        if let Some(offset) = self
            .text_buffer_view
            .offset_for_visual_position(next_row, cursor.visual_col)
        {
            self.edit_buffer_mut().set_cursor_by_offset(offset);
        }
    }

    pub fn delete_selected_text(&mut self) {
        if let Some((start, end)) = self.selection_range() {
            self.edit_buffer_mut().delete_range_by_offsets(start, end);
            self.reset_selection();
        }
    }

    pub fn set_cursor_by_offset(&mut self, offset: u32) {
        self.edit_buffer_mut().set_cursor_by_offset(offset);
    }

    pub fn next_word_boundary(&self) -> VisualCursor {
        let logical = self.edit_buffer().next_word_boundary();
        self.visual_cursor_from_logical(logical)
    }

    pub fn prev_word_boundary(&self) -> VisualCursor {
        let logical = self.edit_buffer().prev_word_boundary();
        self.visual_cursor_from_logical(logical)
    }

    pub fn eol(&self) -> VisualCursor {
        let logical = self.edit_buffer().eol();
        self.visual_cursor_from_logical(logical)
    }

    pub fn visual_sol(&self) -> VisualCursor {
        let cursor = self.visual_cursor();
        let offset = self
            .text_buffer_view
            .offset_for_visual_position(cursor.visual_row, 0)
            .unwrap_or(cursor.offset);
        let logical = self
            .edit_buffer()
            .offset_to_position(offset)
            .unwrap_or(LogicalCursor {
                row: cursor.logical_row,
                col: 0,
                offset,
            });
        self.visual_cursor_from_logical(logical)
    }

    pub fn visual_eol(&self) -> VisualCursor {
        let lines = self.text_buffer_view.visual_lines();
        let cursor = self.visual_cursor();
        let line = lines
            .get(usize::try_from(cursor.visual_row).unwrap_or(usize::MAX))
            .copied();
        let offset = line
            .map(|line| line.start_offset.saturating_add(line.width_cols))
            .unwrap_or(cursor.offset);
        let logical = self
            .edit_buffer()
            .offset_to_position(offset)
            .unwrap_or(LogicalCursor {
                row: cursor.logical_row,
                col: cursor.logical_col,
                offset,
            });
        self.visual_cursor_from_logical(logical)
    }

    fn edit_buffer(&self) -> &EditBufferState {
        assert!(
            !self.edit_buffer.is_null(),
            "EditorViewState requires a valid EditBufferState"
        );
        unsafe { &*self.edit_buffer }
    }

    fn edit_buffer_mut(&mut self) -> &mut EditBufferState {
        assert!(
            !self.edit_buffer.is_null(),
            "EditorViewState requires a valid EditBufferState"
        );
        unsafe { &mut *self.edit_buffer }
    }

    fn selection_range(&self) -> Option<(u32, u32)> {
        match self.selection_info() {
            0xffff_ffff_ffff_ffff => None,
            packed => Some(((packed >> 32) as u32, packed as u32)),
        }
    }

    fn visual_cursor_from_logical(&self, logical: LogicalCursor) -> VisualCursor {
        let (visual_row, visual_col, logical_row, logical_col, offset) = self
            .text_buffer_view
            .visual_cursor_for_offset(logical.offset, logical.row, logical.col);
        VisualCursor {
            visual_row,
            visual_col,
            logical_row,
            logical_col,
            offset,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{EditorViewState, VisualCursor};
    use crate::edit_buffer::EditBufferState;

    #[test]
    fn viewport_and_wrap_count_round_trip() {
        let mut edit = EditBufferState::new(0);
        edit.set_text_bytes(b"ABCDEFGHIJKLMNOPQRSTUVWXYZ");

        let mut view = EditorViewState::new(&mut edit, 10, 5);
        assert_eq!(view.viewport(), (0, 0, 10, 5));
        assert_eq!(view.virtual_line_count(), 1);

        view.set_wrap_mode(1);
        assert_eq!(view.virtual_line_count(), 3);

        view.set_viewport(2, 4, 8, 3);
        assert_eq!(view.viewport(), (2, 4, 8, 3));
        assert_eq!(view.virtual_line_count(), 4);
    }

    #[test]
    fn selection_and_text_passthrough_round_trip() {
        let mut edit = EditBufferState::new(0);
        edit.set_text_bytes(b"Hello World");

        let mut view = EditorViewState::new(&mut edit, 40, 10);
        view.set_selection(6, 11);
        assert_eq!(view.selected_text_bytes(), b"World");
        assert_eq!(String::from_utf8_lossy(view.text_bytes()), "Hello World");
        assert_eq!(view.cursor(), (0, 0));
        view.update_selection(8);
        assert_eq!(view.selected_text_bytes(), b"Wo");
        view.reset_selection();
        assert!(view.selected_text_bytes().is_empty());
    }

    #[test]
    fn visual_cursor_and_selection_actions_round_trip() {
        let mut edit = EditBufferState::new(0);
        edit.set_text_bytes(b"Hello World");

        let mut view = EditorViewState::new(&mut edit, 5, 10);
        view.set_wrap_mode(1);

        let cursor = view.visual_cursor();
        assert_eq!(
            cursor,
            VisualCursor {
                visual_row: 0,
                visual_col: 0,
                logical_row: 0,
                logical_col: 0,
                offset: 0
            }
        );

        view.set_local_selection(0, 0, 5, 1);
        assert!(!view.selected_text_bytes().is_empty());
        view.delete_selected_text();
        assert_ne!(String::from_utf8_lossy(view.text_bytes()), "Hello World");

        view.set_cursor_by_offset(0);
        view.move_down_visual();
        assert!(view.visual_cursor().visual_row <= view.total_virtual_line_count());
        let _ = view.next_word_boundary();
        let _ = view.prev_word_boundary();
        let _ = view.eol();
        let _ = view.visual_sol();
        let _ = view.visual_eol();
    }
}
