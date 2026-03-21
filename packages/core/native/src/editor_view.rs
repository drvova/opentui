use crate::{edit_buffer::EditBufferState, text_buffer_view::TextBufferViewState};

#[derive(Debug)]
pub struct EditorViewState {
    edit_buffer: *mut EditBufferState,
    text_buffer_view: Box<TextBufferViewState>,
    viewport_x: u32,
    viewport_y: u32,
    viewport_width: u32,
    viewport_height: u32,
    scroll_margin: f32,
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
        }
    }

    pub fn text_buffer_view_ptr(&mut self) -> *mut TextBufferViewState {
        self.text_buffer_view.as_mut() as *mut TextBufferViewState
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

    pub fn update_selection(&mut self, end: u32) {
        self.text_buffer_view.update_selection(end);
    }

    pub fn reset_selection(&mut self) {
        self.text_buffer_view.reset_selection();
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

    pub fn cursor(&self) -> (u32, u32) {
        let cursor = self.edit_buffer().cursor();
        (cursor.row, cursor.col)
    }

    fn edit_buffer(&self) -> &EditBufferState {
        assert!(
            !self.edit_buffer.is_null(),
            "EditorViewState requires a valid EditBufferState"
        );
        unsafe { &*self.edit_buffer }
    }
}

#[cfg(test)]
mod tests {
    use super::EditorViewState;
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
}
