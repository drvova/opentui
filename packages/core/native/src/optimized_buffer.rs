use std::collections::HashMap;

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::{
    alloc_grapheme_bytes, encoded_char_width, grapheme_bytes, grapheme_id_from_char,
    is_continuation_char, is_grapheme_char, pack_continuation, pack_grapheme_start,
    release_grapheme_id, retain_grapheme_id,
};

pub type Rgba = [f32; 4];

const DEFAULT_SPACE_CHAR: u32 = ' ' as u32;
const DEFAULT_BLOCK_CHAR: u32 = '█' as u32;
const ATTRIBUTE_BASE_MASK: u32 = 0xff;
const DEFAULT_BG: Rgba = [0.0, 0.0, 0.0, 1.0];
const DEFAULT_FG: Rgba = [1.0, 1.0, 1.0, 1.0];
const GRAYSCALE_CHARS: &[u8] =
    b" .'^\",:;Il!i><~+_-?][}{1)(|\\/tfjrxnuvczXYUJCLQ0OZmwqpdbkhao*#MW&8%B@$";
const QUADRANT_CHARS: [u32; 16] = [
    32, 0x2597, 0x2596, 0x2584, 0x259D, 0x2590, 0x259E, 0x259F, 0x2598, 0x259A, 0x258C, 0x2599,
    0x2580, 0x259C, 0x259B, 0x2588,
];

#[derive(Clone, Copy, Debug, Default)]
pub struct BorderSides {
    pub top: bool,
    pub right: bool,
    pub bottom: bool,
    pub left: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct GridDrawOptions {
    pub draw_inner: bool,
    pub draw_outer: bool,
}

#[derive(Clone, Copy, Debug)]
struct ClipRect {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

#[derive(Clone, Copy, Debug)]
struct Cell {
    char: u32,
    fg: Rgba,
    bg: Rgba,
    attributes: u32,
}

#[derive(Clone, Copy, Debug)]
struct QuadrantResult {
    char: u32,
    fg: Rgba,
    bg: Rgba,
}

#[repr(C)]
#[derive(Debug, Clone)]
pub struct OptimizedBuffer {
    width: usize,
    height: usize,
    respect_alpha: bool,
    id: Vec<u8>,
    chars: Vec<u32>,
    fg: Vec<Rgba>,
    bg: Vec<Rgba>,
    attributes: Vec<u32>,
    opacity_stack: Vec<f32>,
    scissor_stack: Vec<ClipRect>,
    used_graphemes: HashMap<u32, u32>,
}

fn clamp01(value: f32) -> f32 {
    value.clamp(0.0, 1.0)
}

fn color_has_alpha(color: Rgba) -> bool {
    color[3] < 1.0
}

fn is_fully_opaque(opacity: f32, fg: Rgba, bg: Rgba) -> bool {
    opacity == 1.0 && !color_has_alpha(fg) && !color_has_alpha(bg)
}

fn blend_color(overlay: Rgba, dest: Rgba) -> Rgba {
    if overlay[3] >= 1.0 {
        return overlay;
    }

    if dest[3] <= 0.0 {
        let alpha = clamp01(overlay[3]);
        return [
            overlay[0] * alpha,
            overlay[1] * alpha,
            overlay[2] * alpha,
            alpha,
        ];
    }

    let alpha = clamp01(overlay[3]);
    let perceptual_alpha = if alpha > 0.8 {
        0.8 + ((alpha - 0.8) * 5.0).powf(0.2) * 0.2
    } else {
        alpha.powf(0.9)
    };

    [
        overlay[0] * perceptual_alpha + dest[0] * (1.0 - perceptual_alpha),
        overlay[1] * perceptual_alpha + dest[1] * (1.0 - perceptual_alpha),
        overlay[2] * perceptual_alpha + dest[2] * (1.0 - perceptual_alpha),
        alpha + dest[3] * (1.0 - alpha),
    ]
}

fn get_base_attributes(attributes: u32) -> u32 {
    attributes & ATTRIBUTE_BASE_MASK
}

fn get_link_id(attributes: u32) -> u32 {
    attributes >> 8
}

fn attributes_with_link(base_attributes: u32, link_id: u32) -> u32 {
    (base_attributes & ATTRIBUTE_BASE_MASK) | (link_id << 8)
}

fn color_distance(a: Rgba, b: Rgba) -> f32 {
    let dr = a[0] - b[0];
    let dg = a[1] - b[1];
    let db = a[2] - b[2];
    dr * dr + dg * dg + db * db
}

fn average_color(colors: &[Rgba]) -> Rgba {
    if colors.is_empty() {
        return [0.0; 4];
    }

    let mut sum = [0.0; 4];
    for color in colors {
        sum[0] += color[0];
        sum[1] += color[1];
        sum[2] += color[2];
        sum[3] += color[3];
    }

    let len = colors.len() as f32;
    [sum[0] / len, sum[1] / len, sum[2] / len, sum[3] / len]
}

fn luminance(color: Rgba) -> f32 {
    0.2126 * color[0] + 0.7152 * color[1] + 0.0722 * color[2]
}

fn closest_color_index(pixel: Rgba, candidates: [Rgba; 2]) -> usize {
    if color_distance(pixel, candidates[0]) <= color_distance(pixel, candidates[1]) {
        0
    } else {
        1
    }
}

fn render_quadrant_block(pixels: [Rgba; 4]) -> QuadrantResult {
    let mut a = 0;
    let mut b = 1;
    let mut max_distance = color_distance(pixels[0], pixels[1]);

    for left in 0..4 {
        for right in (left + 1)..4 {
            let distance = color_distance(pixels[left], pixels[right]);
            if distance > max_distance {
                a = left;
                b = right;
                max_distance = distance;
            }
        }
    }

    let (dark, light) = if luminance(pixels[a]) <= luminance(pixels[b]) {
        (pixels[a], pixels[b])
    } else {
        (pixels[b], pixels[a])
    };

    let bit_values = [8_u8, 4_u8, 2_u8, 1_u8];
    let mut quadrant_bits = 0_u8;
    for (index, pixel) in pixels.iter().enumerate() {
        if closest_color_index(*pixel, [dark, light]) == 0 {
            quadrant_bits |= bit_values[index];
        }
    }

    if quadrant_bits == 0 {
        QuadrantResult {
            char: DEFAULT_SPACE_CHAR,
            fg: dark,
            bg: average_color(&pixels),
        }
    } else if quadrant_bits == 15 {
        QuadrantResult {
            char: QUADRANT_CHARS[15],
            fg: average_color(&pixels),
            bg: light,
        }
    } else {
        QuadrantResult {
            char: QUADRANT_CHARS[usize::from(quadrant_bits)],
            fg: dark,
            bg: light,
        }
    }
}

fn pixel_color(data: &[u8], offset: usize, bgra: bool) -> Rgba {
    if offset + 3 >= data.len() {
        return [1.0, 0.0, 1.0, 0.0];
    }

    let (r, g, b, a) = if bgra {
        (
            data[offset + 2],
            data[offset + 1],
            data[offset],
            data[offset + 3],
        )
    } else {
        (
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        )
    };

    [
        r as f32 / 255.0,
        g as f32 / 255.0,
        b as f32 / 255.0,
        a as f32 / 255.0,
    ]
}

fn grayscale_char(intensity: f32) -> u32 {
    if intensity < 0.01 {
        return DEFAULT_SPACE_CHAR;
    }

    let clamped = clamp01(intensity);
    let index = (clamped * (GRAYSCALE_CHARS.len() - 1) as f32) as usize;
    GRAYSCALE_CHARS[index] as u32
}

fn normalize_terminal_color(color: Rgba, fallback: Rgba) -> Rgba {
    let alpha = clamp01(color[3]);
    if alpha >= 1.0 {
        return [clamp01(color[0]), clamp01(color[1]), clamp01(color[2]), 1.0];
    }

    [
        clamp01(color[0] * alpha + fallback[0] * (1.0 - alpha)),
        clamp01(color[1] * alpha + fallback[1] * (1.0 - alpha)),
        clamp01(color[2] * alpha + fallback[2] * (1.0 - alpha)),
        1.0,
    ]
}

fn color_to_rgb(color: Rgba) -> [u8; 3] {
    [
        (clamp01(color[0]) * 255.0).round() as u8,
        (clamp01(color[1]) * 255.0).round() as u8,
        (clamp01(color[2]) * 255.0).round() as u8,
    ]
}

fn append_move_cursor(out: &mut Vec<u8>, row: u32, col: u32) {
    out.extend_from_slice(format!("\x1b[{};{}H", row.max(1), col.max(1)).as_bytes());
}

fn append_sgr(out: &mut Vec<u8>, fg: Rgba, bg: Rgba, attributes: u32) {
    let [fg_r, fg_g, fg_b] = color_to_rgb(fg);
    let [bg_r, bg_g, bg_b] = color_to_rgb(bg);
    let mut codes = Vec::with_capacity(12);
    codes.push(String::from("0"));

    if attributes & (1 << 0) != 0 {
        codes.push(String::from("1"));
    }
    if attributes & (1 << 1) != 0 {
        codes.push(String::from("2"));
    }
    if attributes & (1 << 2) != 0 {
        codes.push(String::from("3"));
    }
    if attributes & (1 << 3) != 0 {
        codes.push(String::from("4"));
    }
    if attributes & (1 << 4) != 0 {
        codes.push(String::from("5"));
    }
    if attributes & (1 << 5) != 0 {
        codes.push(String::from("7"));
    }
    if attributes & (1 << 6) != 0 {
        codes.push(String::from("8"));
    }
    if attributes & (1 << 7) != 0 {
        codes.push(String::from("9"));
    }

    codes.push(format!("38;2;{fg_r};{fg_g};{fg_b}"));
    codes.push(format!("48;2;{bg_r};{bg_g};{bg_b}"));

    out.extend_from_slice(b"\x1b[");
    out.extend_from_slice(codes.join(";").as_bytes());
    out.push(b'm');
}

fn append_terminal_cell(out: &mut Vec<u8>, codepoint: u32) {
    if is_grapheme_char(codepoint) {
        if let Some(bytes) = grapheme_bytes(grapheme_id_from_char(codepoint)) {
            out.extend_from_slice(&bytes);
        } else {
            out.push(b' ');
        }
        return;
    }

    if codepoint == 0 {
        out.push(b' ');
        return;
    }

    if let Some(chr) = char::from_u32(codepoint) {
        let mut buf = [0_u8; 4];
        out.extend_from_slice(chr.encode_utf8(&mut buf).as_bytes());
    } else {
        out.push(b' ');
    }
}

fn apply_matrix(matrix: &[f32; 16], color: Rgba, strength: f32) -> Rgba {
    let r = color[0];
    let g = color[1];
    let b = color[2];
    let a = color[3];

    let next_r = matrix[0] * r + matrix[1] * g + matrix[2] * b + matrix[3] * a;
    let next_g = matrix[4] * r + matrix[5] * g + matrix[6] * b + matrix[7] * a;
    let next_b = matrix[8] * r + matrix[9] * g + matrix[10] * b + matrix[11] * a;
    let next_a = matrix[12] * r + matrix[13] * g + matrix[14] * b + matrix[15] * a;

    [
        r + (next_r - r) * strength,
        g + (next_g - g) * strength,
        b + (next_b - b) * strength,
        a + (next_a - a) * strength,
    ]
}

impl OptimizedBuffer {
    pub fn new(width: usize, height: usize, respect_alpha: bool) -> Self {
        Self::with_id(width, height, respect_alpha, b"unnamed".to_vec())
    }

    pub fn with_id(width: usize, height: usize, respect_alpha: bool, id: Vec<u8>) -> Self {
        let cells = width
            .checked_mul(height)
            .expect("OptimizedBuffer dimensions overflow");
        Self {
            width,
            height,
            respect_alpha,
            id,
            chars: vec![0; cells],
            fg: vec![[0.0; 4]; cells],
            bg: vec![DEFAULT_BG; cells],
            attributes: vec![0; cells],
            opacity_stack: Vec::new(),
            scissor_stack: Vec::new(),
            used_graphemes: HashMap::new(),
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

    pub fn id_bytes(&self) -> &[u8] {
        &self.id
    }

    pub fn clear(&mut self) {
        self.clear_with_bg(DEFAULT_BG);
    }

    pub fn clear_with_bg(&mut self, bg: Rgba) {
        self.release_all_graphemes();
        self.chars.fill(0);
        self.fg.fill([0.0; 4]);
        self.bg.fill(bg);
        self.attributes.fill(0);
    }

    pub fn respect_alpha(&self) -> bool {
        self.respect_alpha
    }

    pub fn set_respect_alpha(&mut self, respect_alpha: bool) {
        self.respect_alpha = respect_alpha;
    }

    pub fn real_char_size(&self) -> usize {
        self.width
            .saturating_mul(self.height)
            .saturating_mul(4)
            .saturating_add(self.height)
            .saturating_add(self.used_graphemes.values().copied().sum::<u32>() as usize)
    }

    pub fn draw_text(
        &mut self,
        x: usize,
        y: usize,
        text: &str,
        fg: Rgba,
        bg: Rgba,
        attributes: u32,
    ) -> usize {
        if x >= self.width || y >= self.height || text.is_empty() {
            return 0;
        }

        let mut row = y;
        let mut col = x;
        let mut written = 0;

        for grapheme in text.graphemes(true) {
            if grapheme == "\n" {
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

            let width = UnicodeWidthStr::width(grapheme).max(1);
            let allocated_id = if grapheme.chars().count() == 1 && width == 1 {
                None
            } else {
                Some(alloc_grapheme_bytes(grapheme.as_bytes()))
            };
            let codepoint = if let Some(id) = allocated_id {
                pack_grapheme_start(id, width as u32)
            } else {
                grapheme
                    .chars()
                    .next()
                    .map(u32::from)
                    .unwrap_or(DEFAULT_SPACE_CHAR)
            };
            self.draw_char(codepoint, col, row, fg, bg, attributes);
            if let Some(id) = allocated_id {
                release_grapheme_id(id);
            }
            written += 1;

            col += width;
            if col >= self.width {
                row += 1;
                col = 0;
            }
        }

        written
    }

    pub fn draw_char(
        &mut self,
        codepoint: u32,
        x: usize,
        y: usize,
        fg: Rgba,
        bg: Rgba,
        attributes: u32,
    ) {
        if x >= self.width || y >= self.height || !self.is_point_in_scissor(x as i32, y as i32) {
            return;
        }

        if color_has_alpha(fg) || color_has_alpha(bg) || self.current_opacity() < 1.0 {
            self.set_cell_with_alpha_blending(x, y, codepoint, fg, bg, attributes);
            return;
        }

        self.write_cell(x, y, codepoint, fg, bg, attributes);
    }

    pub fn set_cell_with_alpha_blending(
        &mut self,
        x: usize,
        y: usize,
        codepoint: u32,
        fg: Rgba,
        bg: Rgba,
        attributes: u32,
    ) {
        if x >= self.width || y >= self.height || !self.is_point_in_scissor(x as i32, y as i32) {
            return;
        }

        let opacity = self.current_opacity();
        if is_fully_opaque(opacity, fg, bg) {
            self.write_cell(x, y, codepoint, fg, bg, attributes);
            return;
        }

        let index = self.cell_index(x, y);
        let effective_fg = [fg[0], fg[1], fg[2], fg[3] * opacity];
        let effective_bg = [bg[0], bg[1], bg[2], bg[3] * opacity];
        let overlay = Cell {
            char: codepoint,
            fg: effective_fg,
            bg: effective_bg,
            attributes,
        };
        let existing = Cell {
            char: self.chars[index],
            fg: self.fg[index],
            bg: self.bg[index],
            attributes: self.attributes[index],
        };

        let preserve_char = codepoint == DEFAULT_SPACE_CHAR
            && existing.char != 0
            && existing.char != DEFAULT_SPACE_CHAR;
        let final_char = if preserve_char {
            existing.char
        } else {
            codepoint
        };
        let final_fg = if preserve_char {
            blend_color(effective_bg, existing.fg)
        } else if effective_fg[3] < 1.0 {
            blend_color(effective_fg, existing.bg)
        } else {
            effective_fg
        };
        let mut final_bg = if self.respect_alpha {
            effective_bg
        } else if effective_bg[3] < 1.0 {
            blend_color(effective_bg, existing.bg)
        } else {
            effective_bg
        };
        if effective_bg[3] == 0.0 {
            final_bg[3] = existing.bg[3];
        }
        let base_attributes = if preserve_char {
            get_base_attributes(existing.attributes)
        } else {
            get_base_attributes(overlay.attributes)
        };
        let final_attributes =
            attributes_with_link(base_attributes, get_link_id(overlay.attributes));

        self.write_cell(x, y, final_char, final_fg, final_bg, final_attributes);
    }

    pub fn fill_rect(&mut self, x: usize, y: usize, width: usize, height: usize, bg: Rgba) {
        if width == 0 || height == 0 || x >= self.width || y >= self.height {
            return;
        }

        let Some(clipped) = self.clip_rect(x as i32, y as i32, width as u32, height as u32) else {
            return;
        };

        let opacity = self.current_opacity();
        if color_has_alpha(bg) || opacity < 1.0 {
            for row in
                clipped.y.max(0) as usize..(clipped.y + clipped.height as i32).max(0) as usize
            {
                for col in
                    clipped.x.max(0) as usize..(clipped.x + clipped.width as i32).max(0) as usize
                {
                    self.set_cell_with_alpha_blending(
                        col,
                        row,
                        DEFAULT_SPACE_CHAR,
                        DEFAULT_FG,
                        bg,
                        0,
                    );
                }
            }
            return;
        }

        for row in clipped.y.max(0) as usize..(clipped.y + clipped.height as i32).max(0) as usize {
            for col in clipped.x.max(0) as usize..(clipped.x + clipped.width as i32).max(0) as usize
            {
                self.write_cell(col, row, DEFAULT_SPACE_CHAR, DEFAULT_FG, bg, 0);
            }
        }
    }

    pub fn resize(&mut self, width: usize, height: usize) {
        self.release_all_graphemes();
        let cells = width
            .checked_mul(height)
            .expect("OptimizedBuffer dimensions overflow");
        self.width = width;
        self.height = height;
        self.chars.resize(cells, 0);
        self.fg.resize(cells, [0.0; 4]);
        self.bg.resize(cells, DEFAULT_BG);
        self.attributes.resize(cells, 0);
    }

    pub fn write_resolved_chars_to_vec(&self, add_line_breaks: bool) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.real_char_size());
        for row in 0..self.height {
            for col in 0..self.width {
                let codepoint = self.chars[self.cell_index(col, row)];
                if is_continuation_char(codepoint) {
                    continue;
                }

                if is_grapheme_char(codepoint) {
                    if let Some(bytes) = grapheme_bytes(grapheme_id_from_char(codepoint)) {
                        out.extend_from_slice(&bytes);
                    } else {
                        out.push(b' ');
                    }
                    continue;
                }

                if codepoint == 0 {
                    out.push(b' ');
                    continue;
                }

                if let Some(chr) = char::from_u32(codepoint) {
                    let mut buf = [0_u8; 4];
                    out.extend_from_slice(chr.encode_utf8(&mut buf).as_bytes());
                } else {
                    out.push(b' ');
                }
            }
            if add_line_breaks {
                out.push(b'\n');
            }
        }
        out
    }

    pub fn write_ansi_frame(&self, out: &mut Vec<u8>, start_row: u32) {
        let mut last_style: Option<(Rgba, Rgba, u32)> = None;

        for row in 0..self.height {
            append_move_cursor(out, start_row.saturating_add(row as u32), 1);

            for col in 0..self.width {
                let index = self.cell_index(col, row);
                let codepoint = self.chars[index];
                if is_continuation_char(codepoint) {
                    continue;
                }

                let style = (
                    normalize_terminal_color(self.fg[index], DEFAULT_FG),
                    normalize_terminal_color(self.bg[index], DEFAULT_BG),
                    get_base_attributes(self.attributes[index]),
                );

                if last_style != Some(style) {
                    append_sgr(out, style.0, style.1, style.2);
                    last_style = Some(style);
                }

                append_terminal_cell(out, codepoint);
            }
        }

        out.extend_from_slice(b"\x1b[0m");
    }

    pub fn draw_frame_buffer(
        &mut self,
        dest_x: i32,
        dest_y: i32,
        other: &OptimizedBuffer,
        source_x: usize,
        source_y: usize,
        source_width: Option<usize>,
        source_height: Option<usize>,
    ) {
        if self.width == 0
            || self.height == 0
            || other.width == 0
            || other.height == 0
            || source_x >= other.width
            || source_y >= other.height
        {
            return;
        }

        let source_width = source_width.unwrap_or(other.width.saturating_sub(source_x));
        let source_height = source_height.unwrap_or(other.height.saturating_sub(source_y));
        if source_width == 0 || source_height == 0 {
            return;
        }

        let clamped_source_width = source_width.min(other.width.saturating_sub(source_x));
        let clamped_source_height = source_height.min(other.height.saturating_sub(source_y));
        let start_dest_x = dest_x.max(0);
        let start_dest_y = dest_y.max(0);
        let end_dest_x = (dest_x + clamped_source_width as i32 - 1).min(self.width as i32 - 1);
        let end_dest_y = (dest_y + clamped_source_height as i32 - 1).min(self.height as i32 - 1);

        if start_dest_x > end_dest_x || start_dest_y > end_dest_y {
            return;
        }

        let dest_width = (end_dest_x - start_dest_x + 1) as u32;
        let dest_height = (end_dest_y - start_dest_y + 1) as u32;
        let Some(clipped) = self.clip_rect(start_dest_x, start_dest_y, dest_width, dest_height)
        else {
            return;
        };

        let clipped_start_x = start_dest_x.max(clipped.x);
        let clipped_start_y = start_dest_y.max(clipped.y);
        let clipped_end_x = end_dest_x.min(clipped.x + clipped.width as i32 - 1);
        let clipped_end_y = end_dest_y.min(clipped.y + clipped.height as i32 - 1);

        let grapheme_aware = !self.used_graphemes.is_empty() || !other.used_graphemes.is_empty();

        for dest_row in clipped_start_y..=clipped_end_y {
            let mut last_drawn_grapheme_id = 0_u32;

            for dest_col in clipped_start_x..=clipped_end_x {
                let relative_x = (dest_col - dest_x) as usize;
                let relative_y = (dest_row - dest_y) as usize;
                let src_x = source_x + relative_x;
                let src_y = source_y + relative_y;

                if src_x >= other.width || src_y >= other.height {
                    continue;
                }

                let src_index = other.cell_index(src_x, src_y);
                let codepoint = other.chars[src_index];
                let fg = other.fg[src_index];
                let bg = other.bg[src_index];
                let attributes = other.attributes[src_index];

                if bg[3] == 0.0 && fg[3] == 0.0 {
                    continue;
                }

                if grapheme_aware {
                    if is_continuation_char(codepoint) {
                        let grapheme_id = grapheme_id_from_char(codepoint);
                        if grapheme_id != last_drawn_grapheme_id {
                            self.set_cell_with_alpha_blending(
                                dest_col as usize,
                                dest_row as usize,
                                DEFAULT_SPACE_CHAR,
                                fg,
                                bg,
                                attributes,
                            );
                        }
                        continue;
                    }

                    if is_grapheme_char(codepoint) {
                        last_drawn_grapheme_id = grapheme_id_from_char(codepoint);
                    }
                }

                if color_has_alpha(fg) || color_has_alpha(bg) || self.current_opacity() < 1.0 {
                    self.set_cell_with_alpha_blending(
                        dest_col as usize,
                        dest_row as usize,
                        codepoint,
                        fg,
                        bg,
                        attributes,
                    );
                } else {
                    self.write_cell(
                        dest_col as usize,
                        dest_row as usize,
                        codepoint,
                        fg,
                        bg,
                        attributes,
                    );
                }
            }
        }
    }

    pub fn copy_from(&mut self, other: &OptimizedBuffer) {
        if self.width != other.width || self.height != other.height {
            self.resize(other.width, other.height);
        }

        self.respect_alpha = other.respect_alpha;
        self.id = other.id.clone();
        self.chars.clone_from(&other.chars);
        self.fg.clone_from(&other.fg);
        self.bg.clone_from(&other.bg);
        self.attributes.clone_from(&other.attributes);
    }

    pub fn color_matrix(
        &mut self,
        matrix: &[f32; 16],
        cell_mask: &[f32],
        strength: f32,
        target: u8,
    ) {
        if !strength.is_finite() || cell_mask.len() < 3 || target == 0 {
            return;
        }

        let len = cell_mask.len() - (cell_mask.len() % 3);
        for triplet in cell_mask[..len].chunks_exact(3) {
            let x = triplet[0];
            let y = triplet[1];
            let cell_strength = triplet[2] * strength;
            if !x.is_finite()
                || !y.is_finite()
                || !cell_strength.is_finite()
                || x < 0.0
                || y < 0.0
                || cell_strength == 0.0
            {
                continue;
            }

            let x = x as usize;
            let y = y as usize;
            if x >= self.width || y >= self.height {
                continue;
            }

            let index = self.cell_index(x, y);
            if target & 1 != 0 {
                self.fg[index] = apply_matrix(matrix, self.fg[index], cell_strength);
            }
            if target & 2 != 0 {
                self.bg[index] = apply_matrix(matrix, self.bg[index], cell_strength);
            }
        }
    }

    pub fn color_matrix_uniform(&mut self, matrix: &[f32; 16], strength: f32, target: u8) {
        if !strength.is_finite() || strength == 0.0 || target == 0 {
            return;
        }

        for index in 0..self.chars.len() {
            if target & 1 != 0 {
                self.fg[index] = apply_matrix(matrix, self.fg[index], strength);
            }
            if target & 2 != 0 {
                self.bg[index] = apply_matrix(matrix, self.bg[index], strength);
            }
        }
    }

    pub fn push_opacity(&mut self, opacity: f32) {
        let current = self.current_opacity();
        self.opacity_stack.push(current * opacity.clamp(0.0, 1.0));
    }

    pub fn pop_opacity(&mut self) {
        let _ = self.opacity_stack.pop();
    }

    pub fn current_opacity(&self) -> f32 {
        *self.opacity_stack.last().unwrap_or(&1.0)
    }

    pub fn clear_opacity(&mut self) {
        self.opacity_stack.clear();
    }

    pub fn push_scissor_rect(&mut self, x: i32, y: i32, width: u32, height: u32) {
        let next = if let Some(current) = self.current_scissor_rect() {
            intersect_clip_rects(
                current,
                ClipRect {
                    x,
                    y,
                    width,
                    height,
                },
            )
        } else {
            Some(ClipRect {
                x,
                y,
                width,
                height,
            })
        };

        self.scissor_stack.push(next.unwrap_or(ClipRect {
            x: 0,
            y: 0,
            width: 0,
            height: 0,
        }));
    }

    pub fn pop_scissor_rect(&mut self) {
        let _ = self.scissor_stack.pop();
    }

    pub fn clear_scissor_rects(&mut self) {
        self.scissor_stack.clear();
    }

    pub fn draw_grid(
        &mut self,
        column_offsets: &[i32],
        row_offsets: &[i32],
        border_chars: &[u32],
        fg: Rgba,
        bg: Rgba,
        options: GridDrawOptions,
    ) {
        if column_offsets.len() < 2
            || row_offsets.len() < 2
            || (!options.draw_inner && !options.draw_outer)
        {
            return;
        }

        let column_count = column_offsets.len() - 1;
        let row_count = row_offsets.len() - 1;
        let h_char = *border_chars.get(4).unwrap_or(&('-' as u32));
        let v_char = *border_chars.get(5).unwrap_or(&('|' as u32));

        for row_idx in 0..=row_count {
            let is_outer_row = row_idx == 0 || row_idx == row_count;
            let should_draw_horizontal = if is_outer_row {
                options.draw_outer
            } else {
                options.draw_inner
            };
            let border_y = row_offsets[row_idx];
            if border_y >= self.height as i32 {
                break;
            }

            if should_draw_horizontal && border_y >= 0 {
                for col_border_idx in 0..=column_count {
                    let is_outer_col = col_border_idx == 0 || col_border_idx == column_count;
                    let should_draw_vertical = if is_outer_col {
                        options.draw_outer
                    } else {
                        options.draw_inner
                    };
                    if !should_draw_vertical {
                        continue;
                    }

                    let border_x = column_offsets[col_border_idx];
                    if border_x < 0 || border_x >= self.width as i32 {
                        continue;
                    }

                    let has_up = row_idx > 0 && should_draw_vertical;
                    let has_down = row_idx < row_count && should_draw_vertical;
                    let has_left = col_border_idx > 0;
                    let has_right = col_border_idx < column_count;
                    let intersection = table_border_intersection(
                        border_chars,
                        has_up,
                        has_down,
                        has_left,
                        has_right,
                    );
                    self.set_cell_with_alpha_blending(
                        border_x as usize,
                        border_y as usize,
                        intersection,
                        fg,
                        bg,
                        0,
                    );
                }

                for col_idx in 0..column_count {
                    let has_boundary_after = if col_idx < column_count - 1 {
                        options.draw_inner
                    } else {
                        options.draw_outer
                    };
                    let boundary_padding = if has_boundary_after { 0 } else { 1 };
                    let start_x = column_offsets[col_idx] + 1;
                    let end_x = column_offsets[col_idx + 1] + boundary_padding;
                    for draw_x in start_x.max(0)..end_x.min(self.width as i32) {
                        if border_y >= 0 {
                            self.set_cell_with_alpha_blending(
                                draw_x as usize,
                                border_y as usize,
                                h_char,
                                fg,
                                bg,
                                0,
                            );
                        }
                    }
                }
            }

            if row_idx >= row_count {
                break;
            }

            let has_row_boundary_after = if row_idx < row_count - 1 {
                options.draw_inner
            } else {
                options.draw_outer
            };
            let row_boundary_padding = if has_row_boundary_after { 0 } else { 1 };
            let content_start_y = border_y + 1;
            let content_end_y = row_offsets[row_idx + 1] + row_boundary_padding;
            for draw_y in content_start_y.max(0)..content_end_y.min(self.height as i32) {
                for col_border_idx in 0..=column_count {
                    let is_outer_col = col_border_idx == 0 || col_border_idx == column_count;
                    let should_draw_vertical = if is_outer_col {
                        options.draw_outer
                    } else {
                        options.draw_inner
                    };
                    if !should_draw_vertical {
                        continue;
                    }

                    let border_x = column_offsets[col_border_idx];
                    if border_x < 0 || border_x >= self.width as i32 {
                        continue;
                    }

                    self.set_cell_with_alpha_blending(
                        border_x as usize,
                        draw_y as usize,
                        v_char,
                        fg,
                        bg,
                        0,
                    );
                }
            }
        }
    }

    pub fn draw_box(
        &mut self,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        border_chars: &[u32],
        border_sides: BorderSides,
        border_color: Rgba,
        background_color: Rgba,
        should_fill: bool,
        title: Option<&str>,
        title_alignment: u8,
    ) {
        if width == 0 || height == 0 {
            return;
        }

        let start_x = x.max(0);
        let start_y = y.max(0);
        let end_x = (self.width as i32 - 1).min(x + width as i32 - 1);
        let end_y = (self.height as i32 - 1).min(y + height as i32 - 1);
        if start_x > end_x || start_y > end_y {
            return;
        }

        let box_width = (end_x - start_x + 1) as u32;
        let box_height = (end_y - start_y + 1) as u32;
        let Some(clipped) = self.clip_rect(start_x, start_y, box_width, box_height) else {
            return;
        };
        let start_x = start_x.max(clipped.x);
        let start_y = start_y.max(clipped.y);
        let end_x = end_x.min(clipped.x + clipped.width as i32 - 1);
        let end_y = end_y.min(clipped.y + clipped.height as i32 - 1);

        let is_at_actual_left = start_x == x;
        let is_at_actual_right = end_x == x + width as i32 - 1;
        let is_at_actual_top = start_y == y;
        let is_at_actual_bottom = end_y == y + height as i32 - 1;

        if should_fill {
            if !border_sides.top
                && !border_sides.right
                && !border_sides.bottom
                && !border_sides.left
            {
                self.fill_rect(
                    start_x as usize,
                    start_y as usize,
                    (end_x - start_x + 1) as usize,
                    (end_y - start_y + 1) as usize,
                    background_color,
                );
            } else {
                let inner_start_x = start_x
                    + if border_sides.left && is_at_actual_left {
                        1
                    } else {
                        0
                    };
                let inner_start_y = start_y
                    + if border_sides.top && is_at_actual_top {
                        1
                    } else {
                        0
                    };
                let inner_end_x = end_x
                    - if border_sides.right && is_at_actual_right {
                        1
                    } else {
                        0
                    };
                let inner_end_y = end_y
                    - if border_sides.bottom && is_at_actual_bottom {
                        1
                    } else {
                        0
                    };

                if inner_end_x >= inner_start_x && inner_end_y >= inner_start_y {
                    self.fill_rect(
                        inner_start_x as usize,
                        inner_start_y as usize,
                        (inner_end_x - inner_start_x + 1) as usize,
                        (inner_end_y - inner_start_y + 1) as usize,
                        background_color,
                    );
                }
            }
        }

        let top_left = *border_chars.first().unwrap_or(&('+' as u32));
        let top_right = *border_chars.get(1).unwrap_or(&('+' as u32));
        let bottom_left = *border_chars.get(2).unwrap_or(&('+' as u32));
        let bottom_right = *border_chars.get(3).unwrap_or(&('+' as u32));
        let horizontal = *border_chars.get(4).unwrap_or(&('-' as u32));
        let vertical = *border_chars.get(5).unwrap_or(&('|' as u32));

        if border_sides.top && is_at_actual_top {
            for draw_x in start_x..=end_x {
                let mut ch = horizontal;
                if draw_x == start_x && border_sides.left && is_at_actual_left {
                    ch = top_left;
                } else if draw_x == end_x && border_sides.right && is_at_actual_right {
                    ch = top_right;
                }
                self.set_cell_with_alpha_blending(
                    draw_x as usize,
                    start_y as usize,
                    ch,
                    border_color,
                    background_color,
                    0,
                );
            }
        }

        if border_sides.bottom && is_at_actual_bottom {
            for draw_x in start_x..=end_x {
                let mut ch = horizontal;
                if draw_x == start_x && border_sides.left && is_at_actual_left {
                    ch = bottom_left;
                } else if draw_x == end_x && border_sides.right && is_at_actual_right {
                    ch = bottom_right;
                }
                self.set_cell_with_alpha_blending(
                    draw_x as usize,
                    end_y as usize,
                    ch,
                    border_color,
                    background_color,
                    0,
                );
            }
        }

        let left_border_only =
            border_sides.left && is_at_actual_left && !border_sides.top && !border_sides.bottom;
        let right_border_only =
            border_sides.right && is_at_actual_right && !border_sides.top && !border_sides.bottom;
        let bottom_only_with_verticals = border_sides.bottom
            && is_at_actual_bottom
            && !border_sides.top
            && (border_sides.left || border_sides.right);
        let top_only_with_verticals = border_sides.top
            && is_at_actual_top
            && !border_sides.bottom
            && (border_sides.left || border_sides.right);
        let extend_verticals_to_top =
            left_border_only || right_border_only || bottom_only_with_verticals;
        let extend_verticals_to_bottom =
            left_border_only || right_border_only || top_only_with_verticals;

        let vertical_start_y = if extend_verticals_to_top {
            start_y
        } else {
            start_y
                + if border_sides.top && is_at_actual_top {
                    1
                } else {
                    0
                }
        };
        let vertical_end_y = if extend_verticals_to_bottom {
            end_y
        } else {
            end_y
                - if border_sides.bottom && is_at_actual_bottom {
                    1
                } else {
                    0
                }
        };
        for draw_y in vertical_start_y..=vertical_end_y {
            if border_sides.left && is_at_actual_left {
                self.set_cell_with_alpha_blending(
                    start_x as usize,
                    draw_y as usize,
                    vertical,
                    border_color,
                    background_color,
                    0,
                );
            }
            if border_sides.right && is_at_actual_right {
                self.set_cell_with_alpha_blending(
                    end_x as usize,
                    draw_y as usize,
                    vertical,
                    border_color,
                    background_color,
                    0,
                );
            }
        }

        if let Some(title) =
            title.filter(|value| !value.is_empty() && border_sides.top && is_at_actual_top)
        {
            let title_width = title
                .chars()
                .map(|ch| UnicodeWidthChar::width(ch).unwrap_or(1).max(1))
                .sum::<usize>() as i32;
            if width as i32 >= title_width + 4 {
                let padding = 2;
                let unclamped_title_x = match title_alignment {
                    1 => start_x + ((width as i32 - title_width) / 2).max(padding),
                    2 => start_x + width as i32 - padding - title_width,
                    _ => start_x + padding,
                };
                let min_title_x = start_x + 1;
                let max_title_x = end_x.saturating_sub(title_width).max(min_title_x);
                let title_x = unclamped_title_x.clamp(min_title_x, max_title_x);
                self.draw_text(
                    title_x as usize,
                    start_y as usize,
                    title,
                    border_color,
                    background_color,
                    0,
                );
            }
        }
    }

    pub fn draw_packed_buffer(
        &mut self,
        data: &[u8],
        pos_x: u32,
        pos_y: u32,
        terminal_width_cells: u32,
        terminal_height_cells: u32,
    ) {
        const CELL_RESULT_SIZE: usize = 48;
        if data.len() < CELL_RESULT_SIZE || terminal_width_cells == 0 || terminal_height_cells == 0
        {
            return;
        }

        for (index, chunk) in data.chunks_exact(CELL_RESULT_SIZE).enumerate() {
            let cell_x = pos_x + (index as u32 % terminal_width_cells);
            let cell_y = pos_y + (index as u32 / terminal_width_cells);
            if cell_x >= terminal_width_cells
                || cell_y >= terminal_height_cells
                || cell_x as usize >= self.width
                || cell_y as usize >= self.height
            {
                continue;
            }

            let bg = read_rgba(chunk, 0);
            let fg = read_rgba(chunk, 16);
            let mut codepoint = read_u32(chunk, 32);
            if codepoint == 0 || codepoint > 0x10ffff {
                codepoint = DEFAULT_SPACE_CHAR;
            } else if codepoint < 32 || (codepoint > 126 && codepoint < 0x2580) {
                codepoint = DEFAULT_BLOCK_CHAR;
            }

            self.set_cell_with_alpha_blending(
                cell_x as usize,
                cell_y as usize,
                codepoint,
                fg,
                bg,
                0,
            );
        }
    }

    pub fn draw_super_sample_buffer(
        &mut self,
        pos_x: usize,
        pos_y: usize,
        pixel_data: &[u8],
        format: u8,
        aligned_bytes_per_row: usize,
    ) {
        if aligned_bytes_per_row == 0 || pixel_data.is_empty() {
            return;
        }

        let bgra = format == 0;
        for cell_y in pos_y..self.height {
            for cell_x in pos_x..self.width {
                if !self.is_point_in_scissor(cell_x as i32, cell_y as i32) {
                    continue;
                }

                let render_x = (cell_x - pos_x) * 2;
                let render_y = (cell_y - pos_y) * 2;
                let tl = pixel_color(
                    pixel_data,
                    render_y * aligned_bytes_per_row + render_x * 4,
                    bgra,
                );
                let tr = pixel_color(
                    pixel_data,
                    render_y * aligned_bytes_per_row + (render_x + 1) * 4,
                    bgra,
                );
                let bl = pixel_color(
                    pixel_data,
                    (render_y + 1) * aligned_bytes_per_row + render_x * 4,
                    bgra,
                );
                let br = pixel_color(
                    pixel_data,
                    (render_y + 1) * aligned_bytes_per_row + (render_x + 1) * 4,
                    bgra,
                );
                let cell = render_quadrant_block([tl, tr, bl, br]);
                self.set_cell_with_alpha_blending(cell_x, cell_y, cell.char, cell.fg, cell.bg, 0);
            }
        }
    }

    pub fn draw_grayscale(
        &mut self,
        pos_x: usize,
        pos_y: usize,
        intensities: &[f32],
        src_width: usize,
        src_height: usize,
        fg: Rgba,
        bg: Rgba,
    ) {
        for row in 0..src_height {
            for col in 0..src_width {
                let dest_x = pos_x + col;
                let dest_y = pos_y + row;
                if dest_x >= self.width
                    || dest_y >= self.height
                    || !self.is_point_in_scissor(dest_x as i32, dest_y as i32)
                {
                    continue;
                }

                let value = intensities
                    .get(row * src_width + col)
                    .copied()
                    .unwrap_or(0.0);
                if value < 0.01 {
                    continue;
                }

                let alpha = clamp01(value) * fg[3] * self.current_opacity();
                let grayscale_fg = [fg[0], fg[1], fg[2], alpha];
                self.set_cell_with_alpha_blending(
                    dest_x,
                    dest_y,
                    grayscale_char(value),
                    grayscale_fg,
                    bg,
                    0,
                );
            }
        }
    }

    pub fn draw_grayscale_supersampled(
        &mut self,
        pos_x: usize,
        pos_y: usize,
        intensities: &[f32],
        src_width: usize,
        src_height: usize,
        fg: Rgba,
        bg: Rgba,
    ) {
        if src_width < 2 || src_height < 2 {
            return;
        }

        for cell_y in 0..(src_height / 2) {
            for cell_x in 0..(src_width / 2) {
                let dest_x = pos_x + cell_x;
                let dest_y = pos_y + cell_y;
                if dest_x >= self.width
                    || dest_y >= self.height
                    || !self.is_point_in_scissor(dest_x as i32, dest_y as i32)
                {
                    continue;
                }

                let qx = cell_x * 2;
                let qy = cell_y * 2;
                let tl = intensities.get(qy * src_width + qx).copied().unwrap_or(0.0);
                let tr = intensities
                    .get(qy * src_width + qx + 1)
                    .copied()
                    .unwrap_or(0.0);
                let bl = intensities
                    .get((qy + 1) * src_width + qx)
                    .copied()
                    .unwrap_or(0.0);
                let br = intensities
                    .get((qy + 1) * src_width + qx + 1)
                    .copied()
                    .unwrap_or(0.0);
                let average = (tl + tr + bl + br) / 4.0;
                if average < 0.01 {
                    continue;
                }

                let alpha = clamp01(average) * fg[3] * self.current_opacity();
                let grayscale_fg = [fg[0], fg[1], fg[2], alpha];
                self.set_cell_with_alpha_blending(
                    dest_x,
                    dest_y,
                    grayscale_char(average),
                    grayscale_fg,
                    bg,
                    0,
                );
            }
        }
    }

    fn current_scissor_rect(&self) -> Option<ClipRect> {
        self.scissor_stack.last().copied()
    }

    fn is_point_in_scissor(&self, x: i32, y: i32) -> bool {
        match self.current_scissor_rect() {
            None => true,
            Some(rect) if rect.width == 0 || rect.height == 0 => false,
            Some(rect) => {
                x >= rect.x
                    && x < rect.x + rect.width as i32
                    && y >= rect.y
                    && y < rect.y + rect.height as i32
            }
        }
    }

    fn clip_rect(&self, x: i32, y: i32, width: u32, height: u32) -> Option<ClipRect> {
        let rect = ClipRect {
            x,
            y,
            width,
            height,
        };
        let rect = intersect_clip_rects(
            rect,
            ClipRect {
                x: 0,
                y: 0,
                width: self.width as u32,
                height: self.height as u32,
            },
        )?;

        match self.current_scissor_rect() {
            Some(current) => intersect_clip_rects(rect, current),
            None => Some(rect),
        }
    }

    fn write_cell(
        &mut self,
        x: usize,
        y: usize,
        codepoint: u32,
        fg: Rgba,
        bg: Rgba,
        attributes: u32,
    ) {
        let index = self.cell_index(x, y);
        self.untrack_grapheme_cell(self.chars[index]);
        if is_grapheme_char(codepoint) || is_continuation_char(codepoint) {
            let _ = retain_grapheme_id(grapheme_id_from_char(codepoint));
        }
        self.chars[index] = codepoint;
        self.fg[index] = fg;
        self.bg[index] = bg;
        self.attributes[index] = attributes;
        self.track_grapheme_cell(codepoint);

        let width = encoded_char_width(codepoint) as usize;
        if width > 1 {
            let grapheme_id = grapheme_id_from_char(codepoint);
            for step in 1..width {
                let continuation_x = x + step;
                if continuation_x >= self.width {
                    break;
                }
                let continuation_index = self.cell_index(continuation_x, y);
                self.untrack_grapheme_cell(self.chars[continuation_index]);
                let _ = retain_grapheme_id(grapheme_id);
                self.chars[continuation_index] =
                    pack_continuation(step as u32, (width - step - 1) as u32, grapheme_id);
                self.fg[continuation_index] = fg;
                self.bg[continuation_index] = bg;
                self.attributes[continuation_index] = attributes;
                self.track_grapheme_cell(self.chars[continuation_index]);
            }
        }
    }

    fn track_grapheme_cell(&mut self, codepoint: u32) {
        if !(is_grapheme_char(codepoint) || is_continuation_char(codepoint)) {
            return;
        }
        *self
            .used_graphemes
            .entry(grapheme_id_from_char(codepoint))
            .or_insert(0) += 1;
    }

    fn untrack_grapheme_cell(&mut self, codepoint: u32) {
        if !(is_grapheme_char(codepoint) || is_continuation_char(codepoint)) {
            return;
        }
        let id = grapheme_id_from_char(codepoint);
        let Some(count) = self.used_graphemes.get_mut(&id) else {
            return;
        };
        if *count <= 1 {
            self.used_graphemes.remove(&id);
        } else {
            *count -= 1;
        }
        release_grapheme_id(id);
    }

    fn release_all_graphemes(&mut self) {
        for (&id, &count) in &self.used_graphemes {
            for _ in 0..count {
                release_grapheme_id(id);
            }
        }
        self.used_graphemes.clear();
    }

    fn cell_index(&self, x: usize, y: usize) -> usize {
        assert!(
            x < self.width && y < self.height,
            "cell index out of bounds"
        );
        y * self.width + x
    }
}

impl Drop for OptimizedBuffer {
    fn drop(&mut self) {
        self.release_all_graphemes();
    }
}

fn intersect_clip_rects(left: ClipRect, right: ClipRect) -> Option<ClipRect> {
    let x1 = left.x.max(right.x);
    let y1 = left.y.max(right.y);
    let x2 = (left.x + left.width as i32).min(right.x + right.width as i32);
    let y2 = (left.y + left.height as i32).min(right.y + right.height as i32);
    if x2 <= x1 || y2 <= y1 {
        return None;
    }

    Some(ClipRect {
        x: x1,
        y: y1,
        width: (x2 - x1) as u32,
        height: (y2 - y1) as u32,
    })
}

fn table_border_intersection(
    border_chars: &[u32],
    has_up: bool,
    has_down: bool,
    has_left: bool,
    has_right: bool,
) -> u32 {
    if has_up && has_down && has_left && has_right {
        return border_chars.get(10).copied().unwrap_or('+' as u32);
    }
    if !has_up && has_down && !has_left && has_right {
        return border_chars.first().copied().unwrap_or('+' as u32);
    }
    if !has_up && has_down && has_left && !has_right {
        return border_chars.get(1).copied().unwrap_or('+' as u32);
    }
    if has_up && !has_down && !has_left && has_right {
        return border_chars.get(2).copied().unwrap_or('+' as u32);
    }
    if has_up && !has_down && has_left && !has_right {
        return border_chars.get(3).copied().unwrap_or('+' as u32);
    }
    if has_up && has_down && !has_left && has_right {
        return border_chars.get(8).copied().unwrap_or('+' as u32);
    }
    if has_up && has_down && has_left && !has_right {
        return border_chars.get(9).copied().unwrap_or('+' as u32);
    }
    if !has_up && has_down && has_left && has_right {
        return border_chars.get(6).copied().unwrap_or('+' as u32);
    }
    if has_up && !has_down && has_left && has_right {
        return border_chars.get(7).copied().unwrap_or('+' as u32);
    }
    if (has_left || has_right) && !has_up && !has_down {
        return border_chars.get(4).copied().unwrap_or('-' as u32);
    }
    if (has_up || has_down) && !has_left && !has_right {
        return border_chars.get(5).copied().unwrap_or('|' as u32);
    }
    border_chars.get(10).copied().unwrap_or('+' as u32)
}

fn read_u32(data: &[u8], offset: usize) -> u32 {
    let bytes = [
        *data.get(offset).unwrap_or(&0),
        *data.get(offset + 1).unwrap_or(&0),
        *data.get(offset + 2).unwrap_or(&0),
        *data.get(offset + 3).unwrap_or(&0),
    ];
    u32::from_le_bytes(bytes)
}

fn read_f32(data: &[u8], offset: usize) -> f32 {
    f32::from_bits(read_u32(data, offset))
}

fn read_rgba(data: &[u8], offset: usize) -> Rgba {
    [
        read_f32(data, offset),
        read_f32(data, offset + 4),
        read_f32(data, offset + 8),
        read_f32(data, offset + 12),
    ]
}

#[cfg(test)]
mod tests {
    use super::{DEFAULT_BG, OptimizedBuffer};

    fn read_chars(buffer: &OptimizedBuffer) -> Vec<u32> {
        unsafe {
            std::slice::from_raw_parts(buffer.chars_ptr(), buffer.width() * buffer.height())
                .to_vec()
        }
    }

    fn read_attrs(buffer: &OptimizedBuffer) -> Vec<u32> {
        unsafe {
            std::slice::from_raw_parts(buffer.attributes_ptr(), buffer.width() * buffer.height())
                .to_vec()
        }
    }

    fn read_rgba(ptr: *const f32, cells: usize) -> Vec<[f32; 4]> {
        let slice = unsafe { std::slice::from_raw_parts(ptr, cells * 4) };
        slice
            .chunks_exact(4)
            .map(|chunk| [chunk[0], chunk[1], chunk[2], chunk[3]])
            .collect()
    }

    #[test]
    fn clear_resets_all_grids() {
        let mut buffer = OptimizedBuffer::new(3, 2, false);
        buffer.draw_text(0, 0, "abc", [1.0, 0.0, 0.0, 1.0], [0.0, 1.0, 0.0, 1.0], 7);
        buffer.clear();

        assert_eq!(read_chars(&buffer), vec![0; 6]);
        assert_eq!(read_attrs(&buffer), vec![0; 6]);
        assert_eq!(read_rgba(buffer.fg_ptr(), 6), vec![[0.0; 4]; 6]);
        assert_eq!(read_rgba(buffer.bg_ptr(), 6), vec![DEFAULT_BG; 6]);
    }

    #[test]
    fn draw_text_writes_codepoints_and_styles() {
        let mut buffer = OptimizedBuffer::new(4, 2, false);
        let fg = [0.25, 0.5, 0.75, 1.0];
        let bg = [0.1, 0.2, 0.3, 1.0];

        let written = buffer.draw_text(1, 0, "Aé\nZ", fg, bg, 42);

        assert_eq!(written, 3);
        assert_eq!(read_chars(&buffer)[1], 'A' as u32);
        assert_eq!(read_chars(&buffer)[2], 'é' as u32);
        assert_eq!(read_attrs(&buffer)[1], 42);
        assert_eq!(read_rgba(buffer.fg_ptr(), 8)[1], fg);
        assert_eq!(read_rgba(buffer.bg_ptr(), 8)[5], bg);
    }

    #[test]
    fn draw_text_wraps_at_row_end() {
        let mut buffer = OptimizedBuffer::new(3, 2, true);
        let written = buffer.draw_text(2, 0, "abcd", [1.0; 4], [0.0; 4], 1);
        assert_eq!(written, 4);
        assert_eq!(
            read_chars(&buffer),
            vec![0, 0, 'a' as u32, 'b' as u32, 'c' as u32, 'd' as u32]
        );
        assert!(buffer.respect_alpha());
    }

    #[test]
    fn resize_and_framebuffer_copy_work() {
        let mut parent = OptimizedBuffer::with_id(2, 1, false, b"parent".to_vec());
        let mut child = OptimizedBuffer::with_id(2, 1, false, b"child".to_vec());
        child.draw_text(0, 0, "Hi", [1.0; 4], DEFAULT_BG, 0);
        parent.draw_frame_buffer(0, 0, &child, 0, 0, None, None);
        assert_eq!(read_chars(&parent)[0], 'H' as u32);
        assert_eq!(read_chars(&parent)[1], 'i' as u32);

        parent.resize(3, 2);
        assert_eq!(parent.width(), 3);
        assert_eq!(parent.height(), 2);
        assert_eq!(parent.id_bytes(), b"parent");
    }

    #[test]
    fn opacity_and_scissor_stacks_round_trip() {
        let mut buffer = OptimizedBuffer::new(2, 2, false);
        assert_eq!(buffer.current_opacity(), 1.0);
        buffer.push_opacity(0.5);
        assert_eq!(buffer.current_opacity(), 0.5);
        buffer.pop_opacity();
        assert_eq!(buffer.current_opacity(), 1.0);
        buffer.push_scissor_rect(0, 0, 1, 1);
        buffer.clear_scissor_rects();
        buffer.clear_opacity();
        assert_eq!(buffer.current_opacity(), 1.0);
    }
}
