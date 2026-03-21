#[repr(C)]
#[derive(Debug, Clone)]
pub struct OptimizedBuffer {
    width: usize,
    height: usize,
    chars: Vec<u32>,
    fg: Vec<[f32; 4]>,
    bg: Vec<[f32; 4]>,
    attributes: Vec<u32>,
}

impl OptimizedBuffer {
    pub fn new(width: usize, height: usize) -> Self {
        let cells = width.checked_mul(height).expect("OptimizedBuffer dimensions overflow");
        Self {
            width,
            height,
            chars: vec![0; cells],
            fg: vec![[0.0; 4]; cells],
            bg: vec![[0.0; 4]; cells],
            attributes: vec![0; cells],
        }
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn chars_ptr(&self) -> *const u32 {
        self.chars.as_ptr()
    }

    pub fn fg_ptr(&self) -> *const f32 {
        self.fg.as_ptr().cast()
    }

    pub fn bg_ptr(&self) -> *const f32 {
        self.bg.as_ptr().cast()
    }

    pub fn attributes_ptr(&self) -> *const u32 {
        self.attributes.as_ptr()
    }

    pub fn clear(&mut self) {
        self.chars.fill(0);
        self.fg.fill([0.0; 4]);
        self.bg.fill([0.0; 4]);
        self.attributes.fill(0);
    }

    pub fn draw_text(
        &mut self,
        x: usize,
        y: usize,
        text: &str,
        fg: [f32; 4],
        bg: [f32; 4],
        attributes: u32,
    ) -> usize {
        if x >= self.width || y >= self.height || text.is_empty() {
            return 0;
        }

        let mut row = y;
        let mut col = x;
        let mut written = 0;

        for codepoint in text.chars().map(u32::from) {
            if codepoint == u32::from('\n') {
                row += 1;
                col = x;
                if row >= self.height {
                    break;
                }
                continue;
            }

            if row >= self.height {
                break;
            }

            self.set_cell(col, row, codepoint, fg, bg, attributes);
            written += 1;

            col += 1;
            if col >= self.width {
                row += 1;
                col = 0;
            }
        }

        written
    }

    fn set_cell(&mut self, x: usize, y: usize, codepoint: u32, fg: [f32; 4], bg: [f32; 4], attributes: u32) {
        let index = self.cell_index(x, y);
        self.chars[index] = codepoint;
        self.fg[index] = fg;
        self.bg[index] = bg;
        self.attributes[index] = attributes;
    }

    fn cell_index(&self, x: usize, y: usize) -> usize {
        assert!(x < self.width && y < self.height, "cell index out of bounds");
        y * self.width + x
    }
}

#[cfg(test)]
mod tests {
    use super::OptimizedBuffer;

    fn read_chars(buffer: &OptimizedBuffer) -> Vec<u32> {
        unsafe { std::slice::from_raw_parts(buffer.chars_ptr(), buffer.width() * buffer.height()).to_vec() }
    }

    fn read_attrs(buffer: &OptimizedBuffer) -> Vec<u32> {
        unsafe { std::slice::from_raw_parts(buffer.attributes_ptr(), buffer.width() * buffer.height()).to_vec() }
    }

    fn read_rgba(ptr: *const f32, cells: usize) -> Vec<[f32; 4]> {
        let slice = unsafe { std::slice::from_raw_parts(ptr, cells * 4) };
        slice.chunks_exact(4).map(|chunk| [chunk[0], chunk[1], chunk[2], chunk[3]]).collect()
    }

    #[test]
    fn clear_resets_all_grids() {
        let mut buffer = OptimizedBuffer::new(3, 2);
        buffer.draw_text(0, 0, "abc", [1.0, 0.0, 0.0, 1.0], [0.0, 1.0, 0.0, 1.0], 7);
        buffer.clear();

        assert_eq!(read_chars(&buffer), vec![0; 6]);
        assert_eq!(read_attrs(&buffer), vec![0; 6]);
        assert_eq!(read_rgba(buffer.fg_ptr(), 6), vec![[0.0; 4]; 6]);
        assert_eq!(read_rgba(buffer.bg_ptr(), 6), vec![[0.0; 4]; 6]);
    }

    #[test]
    fn draw_text_writes_codepoints_and_styles() {
        let mut buffer = OptimizedBuffer::new(4, 2);
        let fg = [0.25, 0.5, 0.75, 1.0];
        let bg = [0.1, 0.2, 0.3, 0.4];

        let written = buffer.draw_text(1, 0, "Aé\nZ", fg, bg, 42);

        assert_eq!(written, 3);
        assert_eq!(read_chars(&buffer), vec![0, 'A' as u32, 'é' as u32, 0, 0, 'Z' as u32, 0, 0]);
        assert_eq!(read_attrs(&buffer), vec![0, 42, 42, 0, 0, 42, 0, 0]);
        assert_eq!(read_rgba(buffer.fg_ptr(), 8)[1], fg);
        assert_eq!(read_rgba(buffer.bg_ptr(), 8)[5], bg);
    }

    #[test]
    fn draw_text_wraps_at_row_end() {
        let mut buffer = OptimizedBuffer::new(3, 2);

        let written = buffer.draw_text(2, 0, "abcd", [1.0; 4], [0.0; 4], 1);

        assert_eq!(written, 4);
        assert_eq!(read_chars(&buffer), vec![0, 0, 'a' as u32, 'b' as u32, 'c' as u32, 'd' as u32]);
    }
}
