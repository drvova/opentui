use crate::text_buffer::{
    Rgba, TextBufferState, char_weight, copy_bytes_to_out, line_start_offset, next_offset,
    text_width,
};

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct LineInfoOut {
    pub start_cols: *const u32,
    pub start_cols_len: u32,
    pub width_cols: *const u32,
    pub width_cols_len: u32,
    pub sources: *const u32,
    pub sources_len: u32,
    pub wraps: *const u32,
    pub wraps_len: u32,
    pub width_cols_max: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct MeasureResultOut {
    pub line_count: u32,
    pub width_cols_max: u32,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct VirtualLine {
    pub(crate) start_offset: u32,
    pub(crate) width_cols: u32,
    pub(crate) source_line: u32,
    pub(crate) wrap_index: u32,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct VisibleLine {
    pub(crate) viewport_row: u32,
    pub(crate) start_offset: u32,
    pub(crate) end_offset: u32,
    pub(crate) selection_start: Option<u32>,
    pub(crate) selection_end: Option<u32>,
}

pub const NO_SELECTION: u64 = 0xffff_ffff_ffff_ffff;

#[derive(Debug)]
pub struct TextBufferViewState {
    text_buffer: *mut TextBufferState,
    selection: Option<(u32, u32)>,
    selection_bg: Option<Rgba>,
    selection_fg: Option<Rgba>,
    local_selection_anchor: Option<u32>,
    wrap_width: Option<u32>,
    wrap_mode: u8,
    viewport_x: u32,
    viewport_y: u32,
    viewport_width: u32,
    viewport_height: u32,
    tab_indicator: Option<u32>,
    tab_indicator_color: Option<[f32; 4]>,
    truncate: bool,
    scratch_start_cols: Vec<u32>,
    scratch_width_cols: Vec<u32>,
    scratch_sources: Vec<u32>,
    scratch_wraps: Vec<u32>,
}

impl TextBufferViewState {
    pub fn new(text_buffer: *mut TextBufferState) -> Self {
        Self {
            text_buffer,
            selection: None,
            selection_bg: None,
            selection_fg: None,
            local_selection_anchor: None,
            wrap_width: None,
            wrap_mode: 0,
            viewport_x: 0,
            viewport_y: 0,
            viewport_width: 0,
            viewport_height: 0,
            tab_indicator: None,
            tab_indicator_color: None,
            truncate: false,
            scratch_start_cols: Vec::new(),
            scratch_width_cols: Vec::new(),
            scratch_sources: Vec::new(),
            scratch_wraps: Vec::new(),
        }
    }

    pub fn set_selection(&mut self, start: u32, end: u32, bg: Option<Rgba>, fg: Option<Rgba>) {
        self.selection = normalize_selection(start, end);
        self.selection_bg = bg;
        self.selection_fg = fg;
        self.local_selection_anchor = Some(start);
    }

    pub fn update_selection(&mut self, end: u32, bg: Option<Rgba>, fg: Option<Rgba>) {
        let anchor = self
            .local_selection_anchor
            .or_else(|| self.selection.map(|(start, _)| start));
        self.selection = anchor.and_then(|start| normalize_selection(start, end));
        self.selection_bg = bg;
        self.selection_fg = fg;
    }

    pub fn reset_selection(&mut self) {
        self.selection = None;
        self.selection_bg = None;
        self.selection_fg = None;
        self.local_selection_anchor = None;
    }

    pub fn set_local_selection(
        &mut self,
        anchor_x: i32,
        anchor_y: i32,
        focus_x: i32,
        focus_y: i32,
        bg: Option<Rgba>,
        fg: Option<Rgba>,
    ) -> bool {
        let lines = self.compute_virtual_lines(false);
        let max_y = i32::try_from(lines.len()).unwrap_or(i32::MAX) - 1;
        let anchor_above = anchor_y < 0;
        let focus_above = focus_y < 0;
        let anchor_below = anchor_y > max_y;
        let focus_below = focus_y > max_y;

        if (anchor_above && focus_above) || (anchor_below && focus_below) {
            let had_selection = self.selection.is_some();
            self.selection = None;
            self.local_selection_anchor = None;
            return had_selection;
        }

        let text_end_offset = self.text_end_offset();
        let Some(anchor) = self.selection_offset_for_coords(anchor_x, anchor_y, text_end_offset)
        else {
            let had_selection = self.selection.is_some();
            self.selection = None;
            self.local_selection_anchor = None;
            return had_selection;
        };
        let Some(focus) = self.selection_offset_for_coords(focus_x, focus_y, text_end_offset)
        else {
            let had_selection = self.selection.is_some();
            self.selection = None;
            self.local_selection_anchor = None;
            return had_selection;
        };
        self.local_selection_anchor = Some(anchor);
        self.selection = normalize_selection(anchor, focus);
        self.selection_bg = bg;
        self.selection_fg = fg;
        true
    }

    pub fn update_local_selection(
        &mut self,
        anchor_x: i32,
        anchor_y: i32,
        focus_x: i32,
        focus_y: i32,
        bg: Option<Rgba>,
        fg: Option<Rgba>,
    ) -> bool {
        let text_end_offset = self.text_end_offset();
        let anchor = self
            .local_selection_anchor
            .or_else(|| self.selection_offset_for_coords(anchor_x, anchor_y, text_end_offset));
        let Some(anchor) = anchor else {
            return false;
        };
        let Some(focus) = self.selection_offset_for_coords(focus_x, focus_y, text_end_offset)
        else {
            return false;
        };
        self.local_selection_anchor = Some(anchor);
        let start = anchor.min(focus);
        let mut end = anchor.max(focus);
        if focus < anchor {
            end = end.saturating_add(1).min(text_end_offset);
        }
        self.selection = if start == end {
            None
        } else {
            Some((start, end))
        };
        self.selection_bg = bg;
        self.selection_fg = fg;
        true
    }

    pub fn reset_local_selection(&mut self) {
        self.reset_selection();
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

    pub fn set_tab_indicator(&mut self, indicator: u32) {
        self.tab_indicator = Some(indicator);
    }

    pub fn set_tab_indicator_color(&mut self, color: [f32; 4]) {
        self.tab_indicator_color = Some(color);
    }

    pub fn set_truncate(&mut self, truncate: bool) {
        self.truncate = truncate;
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

    pub(crate) fn visible_lines(&self) -> Vec<VisibleLine> {
        let lines = self.compute_virtual_lines(false);
        if lines.is_empty() {
            return Vec::new();
        }

        let start = usize::try_from(self.viewport_y).unwrap_or(0);
        let height = if self.viewport_height == 0 {
            lines.len().saturating_sub(start)
        } else {
            usize::try_from(self.viewport_height).unwrap_or(0)
        };
        let end = lines.len().min(start.saturating_add(height.max(1)));

        (start..end)
            .map(|index| {
                let line = lines[index];
                let next_start = lines
                    .get(index + 1)
                    .map(|next| next.start_offset)
                    .unwrap_or(line.start_offset.saturating_add(line.width_cols));
                let (selection_start, selection_end) = match self.selection {
                    Some((start_offset, end_offset)) => {
                        let start_offset = start_offset.max(line.start_offset);
                        let end_offset = end_offset.min(next_start);
                        if start_offset < end_offset {
                            (Some(start_offset), Some(end_offset))
                        } else {
                            (None, None)
                        }
                    }
                    None => (None, None),
                };

                VisibleLine {
                    viewport_row: u32::try_from(index.saturating_sub(start)).unwrap_or(u32::MAX),
                    start_offset: line.start_offset,
                    end_offset: next_start,
                    selection_start,
                    selection_end,
                }
            })
            .collect()
    }

    pub(crate) fn text_for_offsets(&self, start: u32, end: u32) -> String {
        self.buffer().text_range(start, end)
    }

    pub(crate) fn selection_colors(&self) -> (Option<Rgba>, Option<Rgba>) {
        (self.selection_bg, self.selection_fg)
    }

    pub(crate) fn default_fg(&self) -> Option<Rgba> {
        self.buffer().default_fg()
    }

    pub(crate) fn default_bg(&self) -> Option<Rgba> {
        self.buffer().default_bg()
    }

    pub(crate) fn tab_width(&self) -> u8 {
        self.buffer().tab_width()
    }

    pub fn virtual_line_count(&self) -> u32 {
        u32::try_from(self.compute_virtual_lines(false).len())
            .unwrap_or(u32::MAX)
            .max(1)
    }

    pub fn measure_for_dimensions(&self, width: u32, _height: u32) -> MeasureResultOut {
        let lines = self.compute_virtual_lines_with_width(false, Some(width));
        MeasureResultOut {
            line_count: u32::try_from(lines.len()).unwrap_or(u32::MAX).max(1),
            width_cols_max: lines.iter().map(|line| line.width_cols).max().unwrap_or(0),
        }
    }

    pub fn line_info(&mut self) -> LineInfoOut {
        self.populate_line_info(false)
    }

    pub fn logical_line_info(&mut self) -> LineInfoOut {
        self.populate_line_info(true)
    }

    pub(crate) fn visual_lines(&self) -> Vec<VirtualLine> {
        self.compute_virtual_lines(false)
    }

    pub(crate) fn visual_cursor_for_offset(
        &self,
        offset: u32,
        logical_row: u32,
        logical_col: u32,
    ) -> (u32, u32, u32, u32, u32) {
        let lines = self.compute_virtual_lines(false);
        for (index, line) in lines.iter().enumerate() {
            let next_start = lines
                .get(index + 1)
                .map(|next| next.start_offset)
                .unwrap_or(
                    line.start_offset
                        .saturating_add(line.width_cols)
                        .saturating_add(1),
                );
            if offset >= line.start_offset && offset < next_start {
                let visual_col = offset
                    .saturating_sub(line.start_offset)
                    .min(line.width_cols);
                return (
                    u32::try_from(index).unwrap_or(u32::MAX),
                    visual_col,
                    logical_row,
                    logical_col,
                    offset,
                );
            }
        }

        (0, 0, logical_row, logical_col, offset)
    }

    pub(crate) fn offset_for_visual_position(
        &self,
        visual_row: u32,
        visual_col: u32,
    ) -> Option<u32> {
        let lines = self.compute_virtual_lines(false);
        let visual_row = usize::try_from(visual_row).ok()?;
        let line = lines.get(visual_row)?;
        Some(self.snap_visual_offset(*line, visual_col.min(line.width_cols)))
    }

    fn buffer(&self) -> &TextBufferState {
        assert!(
            !self.text_buffer.is_null(),
            "TextBufferViewState requires a valid TextBufferState"
        );
        unsafe { &*self.text_buffer }
    }

    fn populate_line_info(&mut self, logical_only: bool) -> LineInfoOut {
        let lines = self.compute_virtual_lines(logical_only);

        self.scratch_start_cols.clear();
        self.scratch_width_cols.clear();
        self.scratch_sources.clear();
        self.scratch_wraps.clear();

        let mut width_cols_max = 0_u32;
        for line in lines {
            self.scratch_start_cols.push(line.start_offset);
            self.scratch_width_cols.push(line.width_cols);
            self.scratch_sources.push(line.source_line);
            self.scratch_wraps.push(line.wrap_index);
            width_cols_max = width_cols_max.max(line.width_cols);
        }

        if self.scratch_start_cols.is_empty() {
            self.scratch_start_cols.push(0);
            self.scratch_width_cols.push(0);
            self.scratch_sources.push(0);
            self.scratch_wraps.push(0);
        }

        LineInfoOut {
            start_cols: self.scratch_start_cols.as_ptr(),
            start_cols_len: u32::try_from(self.scratch_start_cols.len()).unwrap_or(u32::MAX),
            width_cols: self.scratch_width_cols.as_ptr(),
            width_cols_len: u32::try_from(self.scratch_width_cols.len()).unwrap_or(u32::MAX),
            sources: self.scratch_sources.as_ptr(),
            sources_len: u32::try_from(self.scratch_sources.len()).unwrap_or(u32::MAX),
            wraps: self.scratch_wraps.as_ptr(),
            wraps_len: u32::try_from(self.scratch_wraps.len()).unwrap_or(u32::MAX),
            width_cols_max,
        }
    }

    fn visual_coords_to_offset(&self, x: i32, y: i32) -> Option<u32> {
        if x < 0 || y < 0 {
            return None;
        }

        let visual_row = usize::try_from(self.viewport_y.saturating_add(y as u32)).ok()?;
        let visual_col = self.viewport_x.saturating_add(x as u32);
        let lines = self.compute_virtual_lines(false);
        let line = lines.get(visual_row)?;
        Some(self.snap_visual_offset(*line, visual_col.min(line.width_cols)))
    }

    fn selection_offset_for_coords(&self, x: i32, y: i32, text_end_offset: u32) -> Option<u32> {
        let lines = self.compute_virtual_lines(false);
        if lines.is_empty() {
            return Some(0);
        }

        let max_y = i32::try_from(lines.len()).unwrap_or(i32::MAX) - 1;
        if y < 0 || x < 0 {
            return Some(0);
        }
        if y > max_y {
            return Some(text_end_offset);
        }
        self.visual_coords_to_offset(x, y)
    }

    fn compute_virtual_lines(&self, logical_only: bool) -> Vec<VirtualLine> {
        self.compute_virtual_lines_with_width(logical_only, self.wrap_width)
    }

    fn text_end_offset(&self) -> u32 {
        let lines = self.compute_virtual_lines(false);
        lines
            .last()
            .map(|line| line.start_offset.saturating_add(line.width_cols))
            .unwrap_or(0)
    }

    fn snap_visual_offset(&self, line: VirtualLine, visual_col: u32) -> u32 {
        let text = std::str::from_utf8(self.buffer().plain_text_bytes()).unwrap_or("");
        let tab_width = self.buffer().tab_width();
        let target = line
            .start_offset
            .saturating_add(visual_col.min(line.width_cols));

        let mut offset = line.start_offset;
        while offset < target {
            let next = next_offset(text, tab_width, offset);
            if next <= offset || next > target {
                break;
            }
            offset = next;
        }
        offset
    }

    fn compute_virtual_lines_with_width(
        &self,
        logical_only: bool,
        override_width: Option<u32>,
    ) -> Vec<VirtualLine> {
        let text = std::str::from_utf8(self.buffer().plain_text_bytes()).unwrap_or("");
        if text.is_empty() {
            return vec![VirtualLine {
                start_offset: 0,
                width_cols: 0,
                source_line: 0,
                wrap_index: 0,
            }];
        }

        let lines: Vec<&str> = text.split('\n').collect();
        let wrap_width = if logical_only || self.wrap_mode == 0 {
            None
        } else {
            override_width.filter(|width| *width > 0)
        };

        let tab_width = self.buffer().tab_width();
        let mut virtual_lines = Vec::new();

        for (source_line, line) in lines.iter().enumerate() {
            let line_start = line_start_offset(text, tab_width, source_line as u32).unwrap_or(0);
            let line_width = text_width(line, tab_width);

            if line.is_empty() {
                virtual_lines.push(VirtualLine {
                    start_offset: line_start,
                    width_cols: 0,
                    source_line: source_line as u32,
                    wrap_index: 0,
                });
                continue;
            }

            let Some(wrap_width) = wrap_width else {
                virtual_lines.push(VirtualLine {
                    start_offset: line_start,
                    width_cols: line_width,
                    source_line: source_line as u32,
                    wrap_index: 0,
                });
                continue;
            };

            match self.wrap_mode {
                2 => {
                    let mut segment_start_col = 0_u32;
                    let mut current_col = 0_u32;
                    let mut last_break_col = None;
                    let mut wrap_index = 0_u32;

                    for token in line.split_inclusive(char::is_whitespace) {
                        let token_width = text_width(token, tab_width);
                        if token_width > wrap_width {
                            if current_col > segment_start_col {
                                let end_col = last_break_col.unwrap_or(current_col);
                                virtual_lines.push(VirtualLine {
                                    start_offset: line_start.saturating_add(segment_start_col),
                                    width_cols: end_col.saturating_sub(segment_start_col),
                                    source_line: source_line as u32,
                                    wrap_index,
                                });
                                wrap_index = wrap_index.saturating_add(1);
                                segment_start_col = end_col;
                                current_col = end_col;
                                last_break_col = None;
                            }

                            for ch in token.chars() {
                                let width = char_weight(ch, tab_width);
                                if current_col > segment_start_col
                                    && current_col
                                        .saturating_sub(segment_start_col)
                                        .saturating_add(width)
                                        > wrap_width
                                {
                                    virtual_lines.push(VirtualLine {
                                        start_offset: line_start.saturating_add(segment_start_col),
                                        width_cols: current_col.saturating_sub(segment_start_col),
                                        source_line: source_line as u32,
                                        wrap_index,
                                    });
                                    wrap_index = wrap_index.saturating_add(1);
                                    segment_start_col = current_col;
                                }

                                current_col = current_col.saturating_add(width);
                                if ch.is_whitespace() {
                                    last_break_col = Some(current_col);
                                }
                            }
                            continue;
                        }

                        if current_col > 0 && current_col.saturating_add(token_width) > wrap_width {
                            let end_col = last_break_col.unwrap_or(current_col);
                            virtual_lines.push(VirtualLine {
                                start_offset: line_start.saturating_add(segment_start_col),
                                width_cols: end_col.saturating_sub(segment_start_col),
                                source_line: source_line as u32,
                                wrap_index,
                            });
                            wrap_index = wrap_index.saturating_add(1);
                            segment_start_col = end_col;
                            current_col = end_col;
                            last_break_col = None;
                        }

                        current_col = current_col.saturating_add(token_width);
                        if token.ends_with(char::is_whitespace) {
                            last_break_col = Some(current_col);
                        }
                    }

                    virtual_lines.push(VirtualLine {
                        start_offset: line_start.saturating_add(segment_start_col),
                        width_cols: line_width.saturating_sub(segment_start_col),
                        source_line: source_line as u32,
                        wrap_index,
                    });
                }
                _ => {
                    let segments = if line_width == 0 {
                        1
                    } else {
                        (line_width + wrap_width - 1) / wrap_width
                    };

                    for wrap_index in 0..segments {
                        let start_col = wrap_index * wrap_width;
                        let width_cols = (line_width.saturating_sub(start_col)).min(wrap_width);
                        virtual_lines.push(VirtualLine {
                            start_offset: line_start.saturating_add(start_col),
                            width_cols,
                            source_line: source_line as u32,
                            wrap_index,
                        });
                    }
                }
            }
        }

        virtual_lines
    }
}

fn normalize_selection(start: u32, end: u32) -> Option<(u32, u32)> {
    if start == end {
        None
    } else {
        Some((start.min(end), start.max(end)))
    }
}

pub fn copy_selected_text(view: &TextBufferViewState, out_ptr: *mut u8, max_len: usize) -> usize {
    let data = view.selected_text_bytes();
    copy_bytes_to_out(&data, out_ptr, max_len)
}

#[cfg(test)]
mod tests {
    use super::{MeasureResultOut, NO_SELECTION, TextBufferViewState};
    use crate::text_buffer::TextBufferState;

    #[test]
    fn selection_info_packs_and_resets() {
        let mut buffer = TextBufferState::new(0);
        let id = buffer.register_mem_buffer(b"Hello\nWorld").unwrap();
        buffer.set_text_from_mem(id);

        let mut view = TextBufferViewState::new(&mut buffer);
        assert_eq!(view.selection_info(), NO_SELECTION);

        view.set_selection(6, 11, None, None);
        assert_eq!(view.selection_info(), ((6_u64) << 32) | 11_u64);
        assert_eq!(view.selected_text_bytes(), b"World");

        view.reset_selection();
        assert_eq!(view.selection_info(), NO_SELECTION);
    }

    #[test]
    fn line_info_and_measure_support_wrapping_and_local_selection() {
        let mut buffer = TextBufferState::new(0);
        let id = buffer.register_mem_buffer(b"Hello World").unwrap();
        buffer.set_text_from_mem(id);

        let mut view = TextBufferViewState::new(&mut buffer);
        view.set_wrap_mode(1);
        view.set_wrap_width(5);

        let line_info = view.line_info();
        assert_eq!(line_info.start_cols_len, 3);
        assert_eq!(unsafe { *line_info.width_cols.add(0) }, 5);

        let logical = view.logical_line_info();
        assert_eq!(logical.start_cols_len, 1);
        assert_eq!(unsafe { *logical.width_cols.add(0) }, 11);

        let measure: MeasureResultOut = view.measure_for_dimensions(4, 10);
        assert_eq!(measure.line_count, 3);
        assert_eq!(measure.width_cols_max, 4);

        assert!(view.set_local_selection(0, 0, 5, 1, None, None));
        assert!(!view.selected_text_bytes().is_empty());
        view.reset_local_selection();
        assert_eq!(view.selection_info(), NO_SELECTION);
    }
}
