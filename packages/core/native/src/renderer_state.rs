use crate::{Rgba, optimized_buffer::OptimizedBuffer, terminal_state::TerminalState};

#[derive(Clone, Copy, Debug)]
struct ClipRect {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct DebugOverlayState {
    pub enabled: bool,
    pub corner: u8,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct MemoryStats {
    pub heap_used: u32,
    pub heap_total: u32,
    pub array_buffers: u32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct RenderStats {
    pub time: f64,
    pub fps: u32,
    pub frame_callback_time: f64,
}

#[derive(Debug)]
pub struct RendererState {
    width: u32,
    height: u32,
    current_buffer: Box<OptimizedBuffer>,
    next_buffer: Box<OptimizedBuffer>,
    pub terminal: TerminalState,
    pub background_color: Rgba,
    pub render_offset: u32,
    pub use_thread: bool,
    pub debug_overlay: DebugOverlayState,
    pub render_stats: RenderStats,
    pub memory_stats: MemoryStats,
    current_hit_grid: Vec<u32>,
    hit_grid_width: u32,
    hit_grid_height: u32,
    hit_grid_dirty: bool,
    hit_grid_scissor_stack: Vec<ClipRect>,
}

impl RendererState {
    pub fn new(width: u32, height: u32) -> Self {
        let current_buffer = Box::new(OptimizedBuffer::new(width as usize, height as usize, false));
        let next_buffer = Box::new(OptimizedBuffer::new(width as usize, height as usize, false));
        let hit_cells = usize::try_from(width.saturating_mul(height)).unwrap_or(0);

        Self {
            width,
            height,
            current_buffer,
            next_buffer,
            terminal: TerminalState::default(),
            background_color: [0.0, 0.0, 0.0, 1.0],
            render_offset: 0,
            use_thread: false,
            debug_overlay: DebugOverlayState::default(),
            render_stats: RenderStats::default(),
            memory_stats: MemoryStats::default(),
            current_hit_grid: vec![0; hit_cells],
            hit_grid_width: width,
            hit_grid_height: height,
            hit_grid_dirty: false,
            hit_grid_scissor_stack: Vec::new(),
        }
    }

    pub fn current_buffer_ptr(&mut self) -> *mut OptimizedBuffer {
        self.current_buffer.as_mut() as *mut OptimizedBuffer
    }

    pub fn next_buffer_ptr(&mut self) -> *mut OptimizedBuffer {
        self.next_buffer.as_mut() as *mut OptimizedBuffer
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        self.current_buffer =
            Box::new(OptimizedBuffer::new(width as usize, height as usize, false));
        self.next_buffer = Box::new(OptimizedBuffer::new(width as usize, height as usize, false));
        self.hit_grid_width = width;
        self.hit_grid_height = height;
        self.current_hit_grid = vec![0; usize::try_from(width.saturating_mul(height)).unwrap_or(0)];
        self.hit_grid_dirty = true;
        self.hit_grid_scissor_stack.clear();
    }

    pub fn render(&mut self) {
        std::mem::swap(&mut self.current_buffer, &mut self.next_buffer);
    }

    pub fn add_to_hit_grid(&mut self, x: i32, y: i32, width: u32, height: u32, id: u32) {
        if width == 0 || height == 0 {
            return;
        }

        for row in 0..height {
            for col in 0..width {
                let gx = x + col as i32;
                let gy = y + row as i32;
                if gx < 0 || gy < 0 {
                    continue;
                }
                let gx = gx as u32;
                let gy = gy as u32;
                if gx >= self.hit_grid_width || gy >= self.hit_grid_height {
                    continue;
                }
                let index = usize::try_from(gy * self.hit_grid_width + gx).unwrap_or(0);
                self.current_hit_grid[index] = id;
            }
        }
        self.hit_grid_dirty = true;
    }

    pub fn add_to_current_hit_grid_clipped(
        &mut self,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        id: u32,
    ) {
        let Some(rect) = self.clip_rect(x, y, width, height) else {
            return;
        };
        self.add_to_hit_grid(rect.x, rect.y, rect.width, rect.height, id);
    }

    pub fn check_hit(&self, x: u32, y: u32) -> u32 {
        if x >= self.hit_grid_width || y >= self.hit_grid_height {
            return 0;
        }
        let index = usize::try_from(y * self.hit_grid_width + x).unwrap_or(0);
        self.current_hit_grid[index]
    }

    pub fn clear_current_hit_grid(&mut self) {
        self.current_hit_grid.fill(0);
        self.hit_grid_dirty = true;
    }

    pub fn hit_grid_dirty(&self) -> bool {
        self.hit_grid_dirty
    }

    pub fn clear_global_link_pool(&mut self) {}

    pub fn push_hit_grid_scissor_rect(&mut self, x: i32, y: i32, width: u32, height: u32) {
        let next = if let Some(current) = self.hit_grid_scissor_stack.last().copied() {
            intersect_rects(
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

        self.hit_grid_scissor_stack.push(next.unwrap_or(ClipRect {
            x: 0,
            y: 0,
            width: 0,
            height: 0,
        }));
    }

    pub fn pop_hit_grid_scissor_rect(&mut self) {
        let _ = self.hit_grid_scissor_stack.pop();
    }

    pub fn clear_hit_grid_scissor_rects(&mut self) {
        self.hit_grid_scissor_stack.clear();
    }

    fn clip_rect(&self, x: i32, y: i32, width: u32, height: u32) -> Option<ClipRect> {
        let rect = ClipRect {
            x,
            y,
            width,
            height,
        };
        let bounds = ClipRect {
            x: 0,
            y: 0,
            width: self.hit_grid_width,
            height: self.hit_grid_height,
        };
        let clipped = intersect_rects(rect, bounds)?;
        match self.hit_grid_scissor_stack.last().copied() {
            Some(scissor) => intersect_rects(clipped, scissor),
            None => Some(clipped),
        }
    }
}

fn intersect_rects(left: ClipRect, right: ClipRect) -> Option<ClipRect> {
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

#[cfg(test)]
mod tests {
    use super::RendererState;

    #[test]
    fn renderer_buffers_and_hit_grid_round_trip() {
        let mut renderer = RendererState::new(4, 2);
        assert_eq!(renderer.check_hit(1, 1), 0);
        renderer.add_to_hit_grid(1, 0, 2, 2, 42);
        assert_eq!(renderer.check_hit(1, 0), 42);
        assert_eq!(renderer.check_hit(2, 1), 42);
        assert!(renderer.hit_grid_dirty());
        renderer.clear_current_hit_grid();
        assert_eq!(renderer.check_hit(1, 0), 0);

        let current = renderer.current_buffer_ptr();
        let next = renderer.next_buffer_ptr();
        renderer.render();
        assert_eq!(renderer.current_buffer_ptr(), next);
        assert_eq!(renderer.next_buffer_ptr(), current);
        renderer.resize(8, 4);
        assert_eq!(renderer.hit_grid_width, 8);
        assert_eq!(renderer.hit_grid_height, 4);
    }
}
