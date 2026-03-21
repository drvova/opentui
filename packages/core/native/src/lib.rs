#![deny(unsafe_op_in_unsafe_fn)]

use core::ffi::{c_char, c_uint};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Mutex, OnceLock};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

mod crossterm_backend;
mod edit_buffer;
mod editor_view;
mod native_span_feed;
mod optimized_buffer;
mod renderer_state;
mod syntax_style;
mod terminal_input;
mod terminal_state;
mod text_buffer;
mod text_buffer_view;

pub type NativeEditBuffer = edit_buffer::EditBufferState;
pub type NativeEditorView = editor_view::EditorViewState;
pub type NativeLogicalCursor = edit_buffer::LogicalCursor;
pub type NativeVisualCursor = editor_view::VisualCursor;
pub type NativeSpanFeedCallbackFn = native_span_feed::CallbackFn;
pub type NativeSpanFeedOptions = native_span_feed::Options;
pub type NativeSpanFeedReserveInfo = native_span_feed::ReserveInfo;
pub type NativeSpanFeedSpanInfo = native_span_feed::SpanInfo;
pub type NativeSpanFeedStats = native_span_feed::Stats;
pub type NativeSpanFeedStream = native_span_feed::Stream;
pub type NativeOptimizedBuffer = optimized_buffer::OptimizedBuffer;
pub type NativeStyledChunk = text_buffer::StyledChunk;
pub type NativeHighlight = text_buffer::ExternalHighlight;
pub type NativeTextBuffer = text_buffer::TextBufferState;
pub type NativeLineInfo = text_buffer_view::LineInfoOut;
pub type NativeMeasureResult = text_buffer_view::MeasureResultOut;
pub type NativeTextBufferView = text_buffer_view::TextBufferViewState;
pub type NativeTerminalCapabilities = terminal_state::TerminalCapabilitiesOut;
pub type NativeCursorState = terminal_state::CursorState;
pub type NativeCursorStyleOptions = terminal_state::CursorStyleOptions;
pub type NativeRenderer = renderer_state::RendererState;

use edit_buffer::EditBufferState;
use editor_view::EditorViewState;
use native_span_feed::{default_options as default_native_span_feed_options, error_to_status};
use optimized_buffer::{
    BorderSides as BufferBorderSides, GridDrawOptions as BufferGridDrawOptions, OptimizedBuffer,
};
use renderer_state::RendererState;
use syntax_style::{Rgba, SyntaxStyleState};
use terminal_state::CursorStyleOptions as TerminalCursorStyleOptions;
use text_buffer::{StyleSpan, TextBufferState, copy_bytes_to_out, text_width};
use text_buffer_view::{NO_SELECTION, TextBufferViewState, VisibleLine, copy_selected_text};

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct NativeBuildOptions {
    pub gpa_safe_stats: bool,
    pub gpa_memory_limit_tracking: bool,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct NativeAllocatorStats {
    pub total_requested_bytes: u64,
    pub active_allocations: u64,
    pub small_allocations: u64,
    pub large_allocations: u64,
    pub requested_bytes_valid: bool,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct NativeEncodedChar {
    pub width: u8,
    pub char: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct NativeGridDrawOptions {
    pub draw_inner: bool,
    pub draw_outer: bool,
}

const ABI_SYMBOL_COUNT: c_uint = parse_symbol_count();
const ABI_HASH_CSTR: &[u8] = concat!(env!("OPENTUI_ABI_SYMBOL_HASH"), "\0").as_bytes();
const BUILD_PROFILE_CSTR: &[u8] = concat!(env!("OPENTUI_BUILD_PROFILE"), "\0").as_bytes();
const CRATE_VERSION_CSTR: &[u8] = concat!(env!("CARGO_PKG_VERSION"), "\0").as_bytes();
static NEXT_LINK_ID: AtomicU32 = AtomicU32::new(1);
static NEXT_GRAPHEME_ID: AtomicU32 = AtomicU32::new(1);
const INVERSE_ATTRIBUTE: u32 = 1 << 5;
static LINK_REGISTRY: OnceLock<Mutex<HashMap<u32, Vec<u8>>>> = OnceLock::new();
static GRAPHEME_REGISTRY: OnceLock<Mutex<HashMap<u32, GraphemeEntry>>> = OnceLock::new();
static LOG_CALLBACK: OnceLock<Mutex<Option<usize>>> = OnceLock::new();
static EVENT_CALLBACK: OnceLock<Mutex<Option<usize>>> = OnceLock::new();

const CHAR_FLAG_GRAPHEME: u32 = 0x8000_0000;
const CHAR_FLAG_CONTINUATION: u32 = 0xC000_0000;
const CHAR_EXT_RIGHT_SHIFT: u32 = 28;
const CHAR_EXT_LEFT_SHIFT: u32 = 26;
const CHAR_EXT_MASK: u32 = 0x3;
const GRAPHEME_ID_MASK: u32 = 0x03FF_FFFF;

#[derive(Clone, Debug)]
struct GraphemeEntry {
    bytes: Vec<u8>,
    refs: u32,
}

#[repr(C)]
pub struct OpentuiRustFoundationAbiInfo {
    pub abi_symbol_count: c_uint,
    pub abi_manifest_hash: *const c_char,
    pub crate_version: *const c_char,
    pub build_profile: *const c_char,
}

const fn parse_symbol_count() -> c_uint {
    let bytes = env!("OPENTUI_ABI_SYMBOL_COUNT").as_bytes();
    let mut index = 0;
    let mut value = 0_u32;

    while index < bytes.len() {
        let digit = bytes[index];
        assert!(
            digit >= b'0' && digit <= b'9',
            "OPENTUI_ABI_SYMBOL_COUNT must be numeric"
        );
        value = value * 10 + (digit - b'0') as u32;
        index += 1;
    }

    value
}

const fn static_cstr(bytes: &'static [u8]) -> *const c_char {
    bytes.as_ptr().cast()
}

fn color_from_ptr(ptr: *const f32) -> Rgba {
    unsafe { std::ptr::read_unaligned(ptr.cast::<Rgba>()) }
}

fn resolve_line_style(
    syntax_style: Option<&SyntaxStyleState>,
    spans: &[StyleSpan],
    segment_col: u32,
    default_fg: Rgba,
    default_bg: Rgba,
    default_attributes: u32,
) -> (Rgba, Rgba, u32) {
    let mut fg = default_fg;
    let mut bg = default_bg;
    let mut attributes = default_attributes;

    let style_id = spans
        .iter()
        .find(|span| span.col <= segment_col && segment_col < span.next_col)
        .map(|span| span.style_id)
        .unwrap_or(0);

    if style_id != 0 {
        if let Some(style) =
            syntax_style.and_then(|syntax_style| syntax_style.resolve_by_id(style_id))
        {
            if let Some(style_fg) = style.fg {
                fg = style_fg;
            }
            if let Some(style_bg) = style.bg {
                bg = style_bg;
            }
            attributes |= style.attributes;
        }
    }

    (fg, bg, attributes)
}

fn collect_line_boundaries(line: &VisibleLine, spans: &[StyleSpan]) -> Vec<u32> {
    let mut boundaries = vec![line.start_offset, line.end_offset];

    if let Some(selection_start) = line.selection_start {
        if selection_start > line.start_offset && selection_start < line.end_offset {
            boundaries.push(selection_start);
        }
    }
    if let Some(selection_end) = line.selection_end {
        if selection_end > line.start_offset && selection_end < line.end_offset {
            boundaries.push(selection_end);
        }
    }

    for span in spans {
        let start = line
            .line_start_offset()
            .saturating_add(span.col.max(line.source_col_start).min(line.source_col_end));
        let end = line.line_start_offset().saturating_add(
            span.next_col
                .max(line.source_col_start)
                .min(line.source_col_end),
        );
        if start < end {
            boundaries.push(start);
            boundaries.push(end);
        }
    }

    boundaries.sort_unstable();
    boundaries.dedup();
    boundaries
}

fn clip_visible_line(line: &VisibleLine, viewport_x: u32, viewport_width: u32) -> VisibleLine {
    if viewport_width == 0 {
        return *line;
    }

    let clipped_source_start = line
        .source_col_start
        .saturating_add(viewport_x)
        .min(line.source_col_end);
    let clipped_source_end = clipped_source_start
        .saturating_add(viewport_width)
        .min(line.source_col_end);
    let start_offset = line
        .start_offset
        .saturating_add(clipped_source_start.saturating_sub(line.source_col_start));
    let end_offset =
        start_offset.saturating_add(clipped_source_end.saturating_sub(clipped_source_start));

    VisibleLine {
        source_col_start: clipped_source_start,
        source_col_end: clipped_source_end,
        start_offset,
        end_offset,
        ..*line
    }
}

fn draw_truncated_line(
    buffer: &mut NativeOptimizedBuffer,
    text_at: impl Fn(u32, u32) -> String + Copy,
    line: &VisibleLine,
    spans: &[StyleSpan],
    syntax_style: Option<&SyntaxStyleState>,
    x: usize,
    row: usize,
    default_fg: Rgba,
    default_bg: Rgba,
    default_attributes: u32,
    selection_bg: Option<Rgba>,
    selection_fg: Option<Rgba>,
    tab_width: u8,
    viewport_width: u32,
) {
    let visible_width =
        viewport_width.min(line.source_col_end.saturating_sub(line.source_col_start));
    if visible_width == 0 {
        return;
    }
    if visible_width <= 3
        || line.source_col_end.saturating_sub(line.source_col_start) <= visible_width
    {
        draw_visible_line(
            buffer,
            text_at,
            &clip_visible_line(line, 0, visible_width),
            spans,
            syntax_style,
            x,
            row,
            default_fg,
            default_bg,
            default_attributes,
            selection_bg,
            selection_fg,
            tab_width,
        );
        return;
    }

    let prefix_cols = (visible_width - 3) / 2;
    let suffix_cols = visible_width - 3 - prefix_cols;
    let prefix_line = VisibleLine {
        source_col_end: line.source_col_start.saturating_add(prefix_cols),
        end_offset: line.start_offset.saturating_add(prefix_cols),
        ..*line
    };
    let suffix_start_offset = line.end_offset.saturating_sub(suffix_cols);
    let suffix_line = VisibleLine {
        source_col_start: line.source_col_end.saturating_sub(suffix_cols),
        start_offset: suffix_start_offset,
        ..*line
    };

    draw_visible_line(
        buffer,
        text_at,
        &prefix_line,
        spans,
        syntax_style,
        x,
        row,
        default_fg,
        default_bg,
        default_attributes,
        selection_bg,
        selection_fg,
        tab_width,
    );

    let hidden_start = prefix_line.end_offset;
    let hidden_end = suffix_line.start_offset;
    let mut ellipsis_fg = default_fg;
    let mut ellipsis_bg = default_bg;
    if let (Some(selection_start), Some(selection_end)) = (line.selection_start, line.selection_end)
    {
        if selection_start < hidden_end && selection_end > hidden_start {
            if let Some(selection_bg) = selection_bg {
                ellipsis_bg = selection_bg;
                if let Some(selection_fg) = selection_fg {
                    ellipsis_fg = selection_fg;
                }
            } else {
                (ellipsis_fg, ellipsis_bg) = (
                    if default_bg[3] > 0.0 {
                        default_bg
                    } else {
                        [0.0, 0.0, 0.0, 1.0]
                    },
                    default_fg,
                );
            }
        }
    }
    let _ = buffer.draw_text(
        x + prefix_cols as usize,
        row,
        "...",
        ellipsis_fg,
        ellipsis_bg,
        default_attributes,
    );

    draw_visible_line(
        buffer,
        text_at,
        &suffix_line,
        spans,
        syntax_style,
        x + prefix_cols as usize + 3,
        row,
        default_fg,
        default_bg,
        default_attributes,
        selection_bg,
        selection_fg,
        tab_width,
    );
}

fn draw_visible_line(
    buffer: &mut NativeOptimizedBuffer,
    text_at: impl Fn(u32, u32) -> String,
    line: &VisibleLine,
    spans: &[StyleSpan],
    syntax_style: Option<&SyntaxStyleState>,
    x: usize,
    row: usize,
    default_fg: Rgba,
    default_bg: Rgba,
    default_attributes: u32,
    selection_bg: Option<Rgba>,
    selection_fg: Option<Rgba>,
    tab_width: u8,
) {
    let boundaries = collect_line_boundaries(line, spans);
    let mut cursor_x = x;

    for window in boundaries.windows(2) {
        let [start, end] = <[u32; 2]>::try_from(window).unwrap_or([0, 0]);
        if start >= end {
            continue;
        }

        let text = text_at(start, end);
        if text.is_empty() {
            continue;
        }

        let segment_col = start.saturating_sub(line.line_start_offset());
        let (mut fg, mut bg, attributes) = resolve_line_style(
            syntax_style,
            spans,
            segment_col,
            default_fg,
            default_bg,
            default_attributes,
        );

        if let (Some(selection_start), Some(selection_end)) =
            (line.selection_start, line.selection_end)
        {
            if selection_start <= start && end <= selection_end {
                if let Some(selection_bg) = selection_bg {
                    bg = selection_bg;
                    if let Some(selection_fg) = selection_fg {
                        fg = selection_fg;
                    }
                } else {
                    (fg, bg) = (
                        if bg[3] > 0.0 {
                            bg
                        } else {
                            [0.0, 0.0, 0.0, 1.0]
                        },
                        fg,
                    );
                }
            }
        }

        if attributes & INVERSE_ATTRIBUTE != 0 {
            (fg, bg) = (bg, fg);
        }

        let _ = buffer.draw_text(cursor_x, row, &text, fg, bg, attributes);
        cursor_x += text_width(&text, tab_width) as usize;
    }
}

fn link_registry() -> &'static Mutex<HashMap<u32, Vec<u8>>> {
    LINK_REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

fn grapheme_registry() -> &'static Mutex<HashMap<u32, GraphemeEntry>> {
    GRAPHEME_REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(crate) fn is_grapheme_char(value: u32) -> bool {
    (value & 0xC000_0000) == CHAR_FLAG_GRAPHEME
}

pub(crate) fn is_continuation_char(value: u32) -> bool {
    (value & 0xC000_0000) == CHAR_FLAG_CONTINUATION
}

pub(crate) fn grapheme_id_from_char(value: u32) -> u32 {
    value & GRAPHEME_ID_MASK
}

pub(crate) fn char_right_extent(value: u32) -> u32 {
    (value >> CHAR_EXT_RIGHT_SHIFT) & CHAR_EXT_MASK
}

pub(crate) fn char_left_extent(value: u32) -> u32 {
    (value >> CHAR_EXT_LEFT_SHIFT) & CHAR_EXT_MASK
}

pub(crate) fn encoded_char_width(value: u32) -> u32 {
    if is_continuation_char(value) {
        char_left_extent(value) + char_right_extent(value) + 1
    } else if is_grapheme_char(value) {
        char_right_extent(value) + 1
    } else {
        1
    }
}

pub(crate) fn pack_grapheme_start(gid: u32, total_width: u32) -> u32 {
    let width_minus_one = total_width.saturating_sub(1).min(3);
    CHAR_FLAG_GRAPHEME
        | ((width_minus_one & CHAR_EXT_MASK) << CHAR_EXT_RIGHT_SHIFT)
        | (gid & GRAPHEME_ID_MASK)
}

pub(crate) fn pack_continuation(left: u32, right: u32, gid: u32) -> u32 {
    CHAR_FLAG_CONTINUATION
        | ((left.min(3) & CHAR_EXT_MASK) << CHAR_EXT_LEFT_SHIFT)
        | ((right.min(3) & CHAR_EXT_MASK) << CHAR_EXT_RIGHT_SHIFT)
        | (gid & GRAPHEME_ID_MASK)
}

pub(crate) fn alloc_grapheme_bytes(bytes: &[u8]) -> u32 {
    let mut registry = grapheme_registry().lock().unwrap();
    if let Some((&id, entry)) = registry.iter_mut().find(|(_, entry)| entry.bytes == bytes) {
        entry.refs = entry.refs.saturating_add(1);
        return id;
    }

    let id = NEXT_GRAPHEME_ID.fetch_add(1, Ordering::Relaxed) & GRAPHEME_ID_MASK;
    registry.insert(
        id,
        GraphemeEntry {
            bytes: bytes.to_vec(),
            refs: 1,
        },
    );
    id
}

pub(crate) fn retain_grapheme_id(id: u32) -> bool {
    let mut registry = grapheme_registry().lock().unwrap();
    let Some(entry) = registry.get_mut(&id) else {
        return false;
    };
    entry.refs = entry.refs.saturating_add(1);
    true
}

pub(crate) fn release_grapheme_id(id: u32) {
    let mut registry = grapheme_registry().lock().unwrap();
    let Some(entry) = registry.get_mut(&id) else {
        return;
    };
    if entry.refs <= 1 {
        registry.remove(&id);
        return;
    }
    entry.refs -= 1;
}

pub(crate) fn grapheme_bytes(id: u32) -> Option<Vec<u8>> {
    grapheme_registry()
        .lock()
        .unwrap()
        .get(&id)
        .map(|entry| entry.bytes.clone())
}

fn log_callback() -> &'static Mutex<Option<usize>> {
    LOG_CALLBACK.get_or_init(|| Mutex::new(None))
}

fn event_callback() -> &'static Mutex<Option<usize>> {
    EVENT_CALLBACK.get_or_init(|| Mutex::new(None))
}

fn emit_native_event(name: &[u8], payload: &[u8]) {
    let callback_ptr = *event_callback().lock().unwrap();
    let Some(callback_ptr) = callback_ptr else {
        return;
    };

    type EventCallback = unsafe extern "C" fn(*const u8, usize, *const u8, usize);
    let callback: EventCallback = unsafe { std::mem::transmute(callback_ptr) };
    unsafe {
        callback(name.as_ptr(), name.len(), payload.as_ptr(), payload.len());
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn linkAlloc(url_ptr: *const u8, url_len: usize) -> u32 {
    if url_ptr.is_null() {
        return 0;
    }
    let url = unsafe { std::slice::from_raw_parts(url_ptr, url_len) };
    let id = NEXT_LINK_ID.fetch_add(1, Ordering::Relaxed);
    link_registry().lock().unwrap().insert(id, url.to_vec());
    id
}

#[unsafe(no_mangle)]
pub extern "C" fn linkGetUrl(link_id: u32, out_ptr: *mut u8, max_len: usize) -> usize {
    let registry = link_registry().lock().unwrap();
    let Some(url) = registry.get(&link_id) else {
        return 0;
    };
    copy_bytes_to_out(url, out_ptr, max_len)
}

#[unsafe(no_mangle)]
pub extern "C" fn attributesWithLink(base_attributes: u32, link_id: u32) -> u32 {
    (link_id << 8) | (base_attributes & 0xff)
}

#[unsafe(no_mangle)]
pub extern "C" fn attributesGetLinkId(attributes: u32) -> u32 {
    attributes >> 8
}

#[unsafe(no_mangle)]
pub extern "C" fn encodeUnicode(
    text_ptr: *const u8,
    text_len: usize,
    out_ptr_ptr: *mut u64,
    out_len_ptr: *mut u64,
    _width_method: u8,
) -> bool {
    if text_ptr.is_null() || out_ptr_ptr.is_null() || out_len_ptr.is_null() {
        return false;
    }
    let text = unsafe { std::slice::from_raw_parts(text_ptr, text_len) };
    let text = std::str::from_utf8(text).unwrap_or("");

    let data: Vec<NativeEncodedChar> = text
        .graphemes(true)
        .map(|grapheme| {
            let width = UnicodeWidthStr::width(grapheme).min(255) as u8;
            let scalar_count = grapheme.chars().count();
            let char = if scalar_count == 1 && width <= 1 {
                grapheme.chars().next().map(u32::from).unwrap_or_default()
            } else {
                pack_grapheme_start(
                    alloc_grapheme_bytes(grapheme.as_bytes()),
                    u32::from(width.max(1)),
                )
            };
            NativeEncodedChar { width, char }
        })
        .collect();
    let boxed = data.into_boxed_slice();
    let ptr = boxed.as_ptr();
    let len = boxed.len();
    std::mem::forget(boxed);
    unsafe {
        *out_ptr_ptr = ptr as usize as u64;
        *out_len_ptr = len as u64;
    }
    true
}

#[unsafe(no_mangle)]
pub extern "C" fn freeUnicode(ptr: *const NativeEncodedChar, len: usize) {
    if ptr.is_null() || len == 0 {
        return;
    }
    unsafe {
        let data = Vec::from_raw_parts(ptr as *mut NativeEncodedChar, len, len);
        for encoded in &data {
            if is_grapheme_char(encoded.char) {
                release_grapheme_id(grapheme_id_from_char(encoded.char));
            }
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn createRenderer(
    width: u32,
    height: u32,
    testing: bool,
    _remote: bool,
) -> *mut NativeRenderer {
    if width == 0 || height == 0 {
        return core::ptr::null_mut();
    }

    Box::into_raw(Box::new(RendererState::new_with_terminal_output(
        width, height, !testing,
    )))
}

#[unsafe(no_mangle)]
pub extern "C" fn destroyRenderer(renderer: *mut NativeRenderer) {
    if renderer.is_null() {
        return;
    }

    unsafe {
        drop(Box::from_raw(renderer));
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn setTerminalEnvVar(
    renderer: *mut NativeRenderer,
    key_ptr: *const u8,
    key_len: usize,
    value_ptr: *const u8,
    value_len: usize,
) -> bool {
    if renderer.is_null() || key_ptr.is_null() || value_ptr.is_null() {
        return false;
    }

    let renderer = unsafe { &mut *renderer };
    let key = unsafe { std::slice::from_raw_parts(key_ptr, key_len) };
    let value = unsafe { std::slice::from_raw_parts(value_ptr, value_len) };
    renderer.terminal.set_terminal_env_var(key, value)
}

#[unsafe(no_mangle)]
pub extern "C" fn setUseThread(renderer: *mut NativeRenderer, use_thread: bool) {
    if renderer.is_null() {
        return;
    }
    let renderer = unsafe { &mut *renderer };
    renderer.use_thread = use_thread;
}

#[unsafe(no_mangle)]
pub extern "C" fn setBackgroundColor(renderer: *mut NativeRenderer, color: *const f32) {
    if renderer.is_null() || color.is_null() {
        return;
    }
    let renderer = unsafe { &mut *renderer };
    renderer.background_color = color_from_ptr(color);
}

#[unsafe(no_mangle)]
pub extern "C" fn setRenderOffset(renderer: *mut NativeRenderer, offset: u32) {
    if renderer.is_null() {
        return;
    }
    let renderer = unsafe { &mut *renderer };
    renderer.render_offset = offset;
}

#[unsafe(no_mangle)]
pub extern "C" fn updateStats(
    renderer: *mut NativeRenderer,
    time: f64,
    fps: u32,
    frame_callback_time: f64,
) {
    if renderer.is_null() {
        return;
    }
    let renderer = unsafe { &mut *renderer };
    renderer.render_stats.time = time;
    renderer.render_stats.fps = fps;
    renderer.render_stats.frame_callback_time = frame_callback_time;
}

#[unsafe(no_mangle)]
pub extern "C" fn updateMemoryStats(
    renderer: *mut NativeRenderer,
    heap_used: u32,
    heap_total: u32,
    array_buffers: u32,
) {
    if renderer.is_null() {
        return;
    }
    let renderer = unsafe { &mut *renderer };
    renderer.memory_stats.heap_used = heap_used;
    renderer.memory_stats.heap_total = heap_total;
    renderer.memory_stats.array_buffers = array_buffers;
}

#[unsafe(no_mangle)]
pub extern "C" fn render(renderer: *mut NativeRenderer, _force: bool) {
    if renderer.is_null() {
        return;
    }
    let renderer = unsafe { &mut *renderer };
    renderer.render();
}

#[unsafe(no_mangle)]
pub extern "C" fn startNativeInputLoop(renderer: *mut NativeRenderer) {
    if renderer.is_null() {
        return;
    }
    let renderer_ref = unsafe { &mut *renderer };
    renderer_ref.start_input_loop(renderer as usize as u64);
}

#[unsafe(no_mangle)]
pub extern "C" fn stopNativeInputLoop(renderer: *mut NativeRenderer) {
    if renderer.is_null() {
        return;
    }
    let renderer_ref = unsafe { &mut *renderer };
    renderer_ref.stop_input_loop();
}

#[unsafe(no_mangle)]
pub extern "C" fn pumpNativeInputEvents(renderer: *mut NativeRenderer) {
    if renderer.is_null() {
        return;
    }
    let renderer_ref = unsafe { &mut *renderer };
    renderer_ref.pump_input_events();
}

#[unsafe(no_mangle)]
pub extern "C" fn getNextBuffer(renderer: *mut NativeRenderer) -> *mut NativeOptimizedBuffer {
    if renderer.is_null() {
        return core::ptr::null_mut();
    }
    let renderer = unsafe { &mut *renderer };
    renderer.next_buffer_ptr()
}

#[unsafe(no_mangle)]
pub extern "C" fn getCurrentBuffer(renderer: *mut NativeRenderer) -> *mut NativeOptimizedBuffer {
    if renderer.is_null() {
        return core::ptr::null_mut();
    }
    let renderer = unsafe { &mut *renderer };
    renderer.current_buffer_ptr()
}

#[unsafe(no_mangle)]
pub extern "C" fn resizeRenderer(renderer: *mut NativeRenderer, width: u32, height: u32) {
    if renderer.is_null() {
        return;
    }
    let renderer = unsafe { &mut *renderer };
    renderer.resize(width, height);
}

#[unsafe(no_mangle)]
pub extern "C" fn setCursorPosition(renderer: *mut NativeRenderer, x: i32, y: i32, visible: bool) {
    if renderer.is_null() {
        return;
    }
    let renderer = unsafe { &mut *renderer };
    renderer.terminal.set_cursor_position(x, y, visible);
}

#[unsafe(no_mangle)]
pub extern "C" fn setCursorColor(renderer: *mut NativeRenderer, color: *const f32) {
    if renderer.is_null() || color.is_null() {
        return;
    }
    let renderer = unsafe { &mut *renderer };
    renderer.terminal.set_cursor_color(color_from_ptr(color));
}

#[unsafe(no_mangle)]
pub extern "C" fn getCursorState(renderer: *const NativeRenderer, out_ptr: *mut NativeCursorState) {
    if renderer.is_null() || out_ptr.is_null() {
        return;
    }
    let renderer = unsafe { &*renderer };
    unsafe {
        *out_ptr = renderer.terminal.cursor_state();
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn setCursorStyleOptions(
    renderer: *mut NativeRenderer,
    options_ptr: *const NativeCursorStyleOptions,
) {
    if renderer.is_null() || options_ptr.is_null() {
        return;
    }
    let renderer = unsafe { &mut *renderer };
    let options = unsafe { *options_ptr };
    renderer
        .terminal
        .set_cursor_style_options(TerminalCursorStyleOptions {
            style: options.style,
            blinking: options.blinking,
            color: options.color,
            cursor: options.cursor,
        });
}

#[unsafe(no_mangle)]
pub extern "C" fn clearTerminal(renderer: *mut NativeRenderer) {
    if renderer.is_null() {
        return;
    }
    let renderer = unsafe { &mut *renderer };
    renderer.terminal.clear_terminal();
}

#[unsafe(no_mangle)]
pub extern "C" fn setTerminalTitle(
    renderer: *mut NativeRenderer,
    title_ptr: *const u8,
    title_len: usize,
) {
    if renderer.is_null() || title_ptr.is_null() {
        return;
    }
    let renderer = unsafe { &mut *renderer };
    let title = unsafe { std::slice::from_raw_parts(title_ptr, title_len) };
    renderer.terminal.set_terminal_title(title);
}

#[unsafe(no_mangle)]
pub extern "C" fn copyToClipboardOSC52(
    renderer: *mut NativeRenderer,
    target: u8,
    payload_ptr: *const u8,
    payload_len: usize,
) -> bool {
    if renderer.is_null() || payload_ptr.is_null() {
        return false;
    }
    let renderer = unsafe { &mut *renderer };
    let payload = unsafe { std::slice::from_raw_parts(payload_ptr, payload_len) };
    renderer.terminal.copy_to_clipboard_osc52(target, payload)
}

#[unsafe(no_mangle)]
pub extern "C" fn clearClipboardOSC52(renderer: *mut NativeRenderer, target: u8) -> bool {
    if renderer.is_null() {
        return false;
    }
    let renderer = unsafe { &mut *renderer };
    renderer.terminal.clear_clipboard_osc52(target)
}

#[unsafe(no_mangle)]
pub extern "C" fn addToHitGrid(
    renderer: *mut NativeRenderer,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    id: u32,
) {
    if renderer.is_null() {
        return;
    }
    let renderer = unsafe { &mut *renderer };
    renderer.add_to_hit_grid(x, y, width, height, id);
}

#[unsafe(no_mangle)]
pub extern "C" fn addToCurrentHitGridClipped(
    renderer: *mut NativeRenderer,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    id: u32,
) {
    if renderer.is_null() {
        return;
    }
    let renderer = unsafe { &mut *renderer };
    renderer.add_to_current_hit_grid_clipped(x, y, width, height, id);
}

#[unsafe(no_mangle)]
pub extern "C" fn checkHit(renderer: *const NativeRenderer, x: u32, y: u32) -> u32 {
    if renderer.is_null() {
        return 0;
    }
    let renderer = unsafe { &*renderer };
    renderer.check_hit(x, y)
}

#[unsafe(no_mangle)]
pub extern "C" fn clearCurrentHitGrid(renderer: *mut NativeRenderer) {
    if renderer.is_null() {
        return;
    }
    let renderer = unsafe { &mut *renderer };
    renderer.clear_current_hit_grid();
}

#[unsafe(no_mangle)]
pub extern "C" fn getHitGridDirty(renderer: *const NativeRenderer) -> bool {
    if renderer.is_null() {
        return false;
    }
    let renderer = unsafe { &*renderer };
    renderer.hit_grid_dirty()
}

#[unsafe(no_mangle)]
pub extern "C" fn dumpBuffers(_renderer: *mut NativeRenderer, _timestamp: u64) {}

#[unsafe(no_mangle)]
pub extern "C" fn dumpHitGrid(_renderer: *mut NativeRenderer) {}

#[unsafe(no_mangle)]
pub extern "C" fn dumpStdoutBuffer(_renderer: *mut NativeRenderer, _timestamp: u64) {}

#[unsafe(no_mangle)]
pub extern "C" fn restoreTerminalModes(renderer: *mut NativeRenderer) {
    if renderer.is_null() {
        return;
    }
    let renderer = unsafe { &mut *renderer };
    renderer.terminal.restore_terminal_modes();
}

#[unsafe(no_mangle)]
pub extern "C" fn enableMouse(renderer: *mut NativeRenderer, enable_movement: bool) {
    if renderer.is_null() {
        return;
    }
    let renderer = unsafe { &mut *renderer };
    renderer.terminal.enable_mouse(enable_movement);
}

#[unsafe(no_mangle)]
pub extern "C" fn disableMouse(renderer: *mut NativeRenderer) {
    if renderer.is_null() {
        return;
    }
    let renderer = unsafe { &mut *renderer };
    renderer.terminal.disable_mouse();
}

#[unsafe(no_mangle)]
pub extern "C" fn enableKittyKeyboard(renderer: *mut NativeRenderer, flags: u8) {
    if renderer.is_null() {
        return;
    }
    let renderer = unsafe { &mut *renderer };
    renderer.terminal.enable_kitty_keyboard(flags);
}

#[unsafe(no_mangle)]
pub extern "C" fn disableKittyKeyboard(renderer: *mut NativeRenderer) {
    if renderer.is_null() {
        return;
    }
    let renderer = unsafe { &mut *renderer };
    renderer.terminal.disable_kitty_keyboard();
}

#[unsafe(no_mangle)]
pub extern "C" fn setKittyKeyboardFlags(renderer: *mut NativeRenderer, flags: u8) {
    if renderer.is_null() {
        return;
    }
    let renderer = unsafe { &mut *renderer };
    renderer.terminal.set_kitty_keyboard_flags(flags);
}

#[unsafe(no_mangle)]
pub extern "C" fn getKittyKeyboardFlags(renderer: *const NativeRenderer) -> u8 {
    if renderer.is_null() {
        return 0;
    }
    let renderer = unsafe { &*renderer };
    renderer.terminal.kitty_keyboard_flags()
}

#[unsafe(no_mangle)]
pub extern "C" fn setupTerminal(renderer: *mut NativeRenderer, use_alternate_screen: bool) {
    if renderer.is_null() {
        return;
    }
    let renderer = unsafe { &mut *renderer };
    renderer.terminal.setup_terminal(use_alternate_screen);
}

#[unsafe(no_mangle)]
pub extern "C" fn suspendRenderer(renderer: *mut NativeRenderer) {
    if renderer.is_null() {
        return;
    }
    let renderer = unsafe { &mut *renderer };
    renderer.terminal.suspend();
}

#[unsafe(no_mangle)]
pub extern "C" fn resumeRenderer(renderer: *mut NativeRenderer) {
    if renderer.is_null() {
        return;
    }
    let renderer = unsafe { &mut *renderer };
    renderer.terminal.resume();
}

#[unsafe(no_mangle)]
pub extern "C" fn queryPixelResolution(renderer: *mut NativeRenderer) {
    if renderer.is_null() {
        return;
    }
    let renderer = unsafe { &mut *renderer };
    renderer.terminal.query_pixel_resolution();
}

#[unsafe(no_mangle)]
pub extern "C" fn writeOut(renderer: *mut NativeRenderer, data_ptr: *const u8, data_len: usize) {
    if renderer.is_null() || data_ptr.is_null() {
        return;
    }
    let renderer = unsafe { &mut *renderer };
    let data = unsafe { std::slice::from_raw_parts(data_ptr, data_len) };
    renderer.terminal.write_out(data);
}

#[unsafe(no_mangle)]
pub extern "C" fn getTerminalCapabilities(
    renderer: *const NativeRenderer,
    out_ptr: *mut NativeTerminalCapabilities,
) {
    if renderer.is_null() || out_ptr.is_null() {
        return;
    }
    let renderer = unsafe { &*renderer };
    unsafe {
        *out_ptr = renderer.terminal.capabilities_out();
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn processCapabilityResponse(
    renderer: *mut NativeRenderer,
    response_ptr: *const u8,
    response_len: usize,
) {
    if renderer.is_null() || response_ptr.is_null() {
        return;
    }
    let renderer = unsafe { &mut *renderer };
    let response = unsafe { std::slice::from_raw_parts(response_ptr, response_len) };
    renderer.terminal.process_capability_response(response);
}

#[unsafe(no_mangle)]
pub extern "C" fn setDebugOverlay(renderer: *mut NativeRenderer, enabled: bool, corner: u8) {
    if renderer.is_null() {
        return;
    }
    let renderer = unsafe { &mut *renderer };
    renderer.debug_overlay.enabled = enabled;
    renderer.debug_overlay.corner = corner;
}

#[unsafe(no_mangle)]
pub extern "C" fn setHyperlinksCapability(renderer: *mut NativeRenderer, enabled: bool) {
    if renderer.is_null() {
        return;
    }
    let renderer = unsafe { &mut *renderer };
    renderer.terminal.set_hyperlinks_capability(enabled);
}

#[unsafe(no_mangle)]
pub extern "C" fn clearGlobalLinkPool() {}

#[unsafe(no_mangle)]
pub extern "C" fn hitGridPushScissorRect(
    renderer: *mut NativeRenderer,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) {
    if renderer.is_null() {
        return;
    }
    let renderer = unsafe { &mut *renderer };
    renderer.push_hit_grid_scissor_rect(x, y, width, height);
}

#[unsafe(no_mangle)]
pub extern "C" fn hitGridPopScissorRect(renderer: *mut NativeRenderer) {
    if renderer.is_null() {
        return;
    }
    let renderer = unsafe { &mut *renderer };
    renderer.pop_hit_grid_scissor_rect();
}

#[unsafe(no_mangle)]
pub extern "C" fn hitGridClearScissorRects(renderer: *mut NativeRenderer) {
    if renderer.is_null() {
        return;
    }
    let renderer = unsafe { &mut *renderer };
    renderer.clear_hit_grid_scissor_rects();
}

#[unsafe(no_mangle)]
pub extern "C" fn opentui_rust_foundation_abi_hash() -> *const c_char {
    static_cstr(ABI_HASH_CSTR)
}

#[unsafe(no_mangle)]
pub extern "C" fn opentui_rust_foundation_abi_info(out: *mut OpentuiRustFoundationAbiInfo) -> bool {
    if out.is_null() {
        return false;
    }

    unsafe {
        *out = OpentuiRustFoundationAbiInfo {
            abi_symbol_count: ABI_SYMBOL_COUNT,
            abi_manifest_hash: static_cstr(ABI_HASH_CSTR),
            crate_version: static_cstr(CRATE_VERSION_CSTR),
            build_profile: static_cstr(BUILD_PROFILE_CSTR),
        };
    }

    true
}

#[unsafe(no_mangle)]
pub extern "C" fn setLogCallback(callback_ptr: *const core::ffi::c_void) {
    *log_callback().lock().unwrap() = (!callback_ptr.is_null()).then_some(callback_ptr as usize);
}

#[unsafe(no_mangle)]
pub extern "C" fn setEventCallback(callback_ptr: *const core::ffi::c_void) {
    *event_callback().lock().unwrap() = (!callback_ptr.is_null()).then_some(callback_ptr as usize);
}

#[unsafe(no_mangle)]
pub extern "C" fn getArenaAllocatedBytes() -> usize {
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn getBuildOptions(out_ptr: *mut NativeBuildOptions) {
    if out_ptr.is_null() {
        return;
    }
    unsafe {
        *out_ptr = NativeBuildOptions {
            gpa_safe_stats: false,
            gpa_memory_limit_tracking: false,
        };
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn getAllocatorStats(out_ptr: *mut NativeAllocatorStats) {
    if out_ptr.is_null() {
        return;
    }
    unsafe {
        *out_ptr = NativeAllocatorStats {
            total_requested_bytes: 0,
            active_allocations: 0,
            small_allocations: 0,
            large_allocations: 0,
            requested_bytes_valid: false,
        };
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn createSyntaxStyle() -> *mut SyntaxStyleState {
    Box::into_raw(Box::new(SyntaxStyleState::default()))
}

#[unsafe(no_mangle)]
pub extern "C" fn destroySyntaxStyle(style: *mut SyntaxStyleState) {
    if style.is_null() {
        return;
    }

    unsafe {
        drop(Box::from_raw(style));
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn syntaxStyleRegister(
    style: *mut SyntaxStyleState,
    name_ptr: *const u8,
    name_len: usize,
    fg: *const f32,
    bg: *const f32,
    attributes: u32,
) -> u32 {
    if style.is_null() || name_ptr.is_null() {
        return 0;
    }

    let state = unsafe { &mut *style };
    let name = unsafe { std::slice::from_raw_parts(name_ptr, name_len) };
    let fg = (!fg.is_null()).then(|| color_from_ptr(fg));
    let bg = (!bg.is_null()).then(|| color_from_ptr(bg));

    state.register_style(name, fg, bg, attributes)
}

#[unsafe(no_mangle)]
pub extern "C" fn syntaxStyleResolveByName(
    style: *const SyntaxStyleState,
    name_ptr: *const u8,
    name_len: usize,
) -> u32 {
    if style.is_null() || name_ptr.is_null() {
        return 0;
    }

    let state = unsafe { &*style };
    let name = unsafe { std::slice::from_raw_parts(name_ptr, name_len) };

    state.resolve_by_name(name).unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn syntaxStyleGetStyleCount(style: *const SyntaxStyleState) -> usize {
    if style.is_null() {
        return 0;
    }

    let state = unsafe { &*style };
    state.style_count()
}

#[unsafe(no_mangle)]
pub extern "C" fn createTextBuffer(width_method: u8) -> *mut NativeTextBuffer {
    Box::into_raw(Box::new(TextBufferState::new(width_method)))
}

#[unsafe(no_mangle)]
pub extern "C" fn destroyTextBuffer(tb: *mut NativeTextBuffer) {
    if tb.is_null() {
        return;
    }

    unsafe {
        drop(Box::from_raw(tb));
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn textBufferGetLength(tb: *const NativeTextBuffer) -> u32 {
    if tb.is_null() {
        return 0;
    }

    let tb = unsafe { &*tb };
    tb.length()
}

#[unsafe(no_mangle)]
pub extern "C" fn textBufferGetByteSize(tb: *const NativeTextBuffer) -> u32 {
    if tb.is_null() {
        return 0;
    }

    let tb = unsafe { &*tb };
    tb.byte_size()
}

#[unsafe(no_mangle)]
pub extern "C" fn textBufferReset(tb: *mut NativeTextBuffer) {
    if tb.is_null() {
        return;
    }

    let tb = unsafe { &mut *tb };
    tb.reset();
}

#[unsafe(no_mangle)]
pub extern "C" fn textBufferClear(tb: *mut NativeTextBuffer) {
    if tb.is_null() {
        return;
    }

    let tb = unsafe { &mut *tb };
    tb.clear();
}

#[unsafe(no_mangle)]
pub extern "C" fn textBufferSetDefaultFg(tb: *mut NativeTextBuffer, fg: *const f32) {
    if tb.is_null() {
        return;
    }

    let tb = unsafe { &mut *tb };
    let fg = (!fg.is_null()).then(|| color_from_ptr(fg));
    tb.set_default_fg(fg);
}

#[unsafe(no_mangle)]
pub extern "C" fn textBufferSetDefaultBg(tb: *mut NativeTextBuffer, bg: *const f32) {
    if tb.is_null() {
        return;
    }

    let tb = unsafe { &mut *tb };
    let bg = (!bg.is_null()).then(|| color_from_ptr(bg));
    tb.set_default_bg(bg);
}

#[unsafe(no_mangle)]
pub extern "C" fn textBufferSetDefaultAttributes(tb: *mut NativeTextBuffer, attr: *const u32) {
    if tb.is_null() {
        return;
    }

    let tb = unsafe { &mut *tb };
    let attributes = (!attr.is_null()).then(|| unsafe { *attr });
    tb.set_default_attributes(attributes);
}

#[unsafe(no_mangle)]
pub extern "C" fn textBufferResetDefaults(tb: *mut NativeTextBuffer) {
    if tb.is_null() {
        return;
    }

    let tb = unsafe { &mut *tb };
    tb.reset_defaults();
}

#[unsafe(no_mangle)]
pub extern "C" fn textBufferGetTabWidth(tb: *const NativeTextBuffer) -> u8 {
    if tb.is_null() {
        return 0;
    }

    let tb = unsafe { &*tb };
    tb.tab_width()
}

#[unsafe(no_mangle)]
pub extern "C" fn textBufferSetTabWidth(tb: *mut NativeTextBuffer, width: u8) {
    if tb.is_null() {
        return;
    }

    let tb = unsafe { &mut *tb };
    tb.set_tab_width(width);
}

#[unsafe(no_mangle)]
pub extern "C" fn textBufferRegisterMemBuffer(
    tb: *mut NativeTextBuffer,
    data_ptr: *const u8,
    data_len: usize,
    _owned: bool,
) -> u16 {
    if tb.is_null() || data_ptr.is_null() {
        return 0xffff;
    }

    let tb = unsafe { &mut *tb };
    let data = unsafe { std::slice::from_raw_parts(data_ptr, data_len) };
    tb.register_mem_buffer(data)
        .map(u16::from)
        .unwrap_or(0xffff)
}

#[unsafe(no_mangle)]
pub extern "C" fn textBufferReplaceMemBuffer(
    tb: *mut NativeTextBuffer,
    mem_id: u8,
    data_ptr: *const u8,
    data_len: usize,
    _owned: bool,
) -> bool {
    if tb.is_null() || data_ptr.is_null() {
        return false;
    }

    let tb = unsafe { &mut *tb };
    let data = unsafe { std::slice::from_raw_parts(data_ptr, data_len) };
    tb.replace_mem_buffer(mem_id, data)
}

#[unsafe(no_mangle)]
pub extern "C" fn textBufferClearMemRegistry(tb: *mut NativeTextBuffer) {
    if tb.is_null() {
        return;
    }

    let tb = unsafe { &mut *tb };
    tb.clear_mem_registry();
}

#[unsafe(no_mangle)]
pub extern "C" fn textBufferSetTextFromMem(tb: *mut NativeTextBuffer, mem_id: u8) {
    if tb.is_null() {
        return;
    }

    let tb = unsafe { &mut *tb };
    tb.set_text_from_mem(mem_id);
}

#[unsafe(no_mangle)]
pub extern "C" fn textBufferAppend(
    tb: *mut NativeTextBuffer,
    data_ptr: *const u8,
    data_len: usize,
) {
    if tb.is_null() || data_ptr.is_null() {
        return;
    }

    let tb = unsafe { &mut *tb };
    let data = unsafe { std::slice::from_raw_parts(data_ptr, data_len) };
    tb.append_bytes(data);
}

#[unsafe(no_mangle)]
pub extern "C" fn textBufferAppendFromMemId(tb: *mut NativeTextBuffer, mem_id: u8) {
    if tb.is_null() {
        return;
    }

    let tb = unsafe { &mut *tb };
    tb.append_from_mem(mem_id);
}

#[unsafe(no_mangle)]
pub extern "C" fn textBufferLoadFile(
    tb: *mut NativeTextBuffer,
    path_ptr: *const u8,
    path_len: usize,
) -> bool {
    if tb.is_null() || path_ptr.is_null() {
        return false;
    }

    let tb = unsafe { &mut *tb };
    let path = unsafe { std::slice::from_raw_parts(path_ptr, path_len) };
    tb.load_file(path)
}

#[unsafe(no_mangle)]
pub extern "C" fn textBufferSetStyledText(
    tb: *mut NativeTextBuffer,
    chunks_ptr: *const NativeStyledChunk,
    chunk_count: usize,
) {
    if tb.is_null() {
        return;
    }

    let tb = unsafe { &mut *tb };
    let chunks = if chunks_ptr.is_null() || chunk_count == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(chunks_ptr, chunk_count) }
    };
    tb.set_styled_text(chunks);
}

#[unsafe(no_mangle)]
pub extern "C" fn textBufferGetLineCount(tb: *const NativeTextBuffer) -> u32 {
    if tb.is_null() {
        return 0;
    }

    let tb = unsafe { &*tb };
    tb.line_count()
}

#[unsafe(no_mangle)]
pub extern "C" fn textBufferGetPlainText(
    tb: *const NativeTextBuffer,
    out_ptr: *mut u8,
    max_len: usize,
) -> usize {
    if tb.is_null() {
        return 0;
    }

    let tb = unsafe { &*tb };
    copy_bytes_to_out(tb.plain_text_bytes(), out_ptr, max_len)
}

#[unsafe(no_mangle)]
pub extern "C" fn textBufferGetTextRange(
    tb: *const NativeTextBuffer,
    start_offset: u32,
    end_offset: u32,
    out_ptr: *mut u8,
    max_len: usize,
) -> usize {
    if tb.is_null() {
        return 0;
    }

    let tb = unsafe { &*tb };
    let text = tb.text_range(start_offset, end_offset);
    copy_bytes_to_out(text.as_bytes(), out_ptr, max_len)
}

#[unsafe(no_mangle)]
pub extern "C" fn textBufferGetTextRangeByCoords(
    tb: *const NativeTextBuffer,
    start_row: u32,
    start_col: u32,
    end_row: u32,
    end_col: u32,
    out_ptr: *mut u8,
    max_len: usize,
) -> usize {
    if tb.is_null() {
        return 0;
    }

    let tb = unsafe { &*tb };
    let text = tb.text_range_by_coords(start_row, start_col, end_row, end_col);
    copy_bytes_to_out(text.as_bytes(), out_ptr, max_len)
}

#[unsafe(no_mangle)]
pub extern "C" fn textBufferAddHighlightByCharRange(
    tb: *mut NativeTextBuffer,
    highlight_ptr: *const NativeHighlight,
) {
    if tb.is_null() || highlight_ptr.is_null() {
        return;
    }

    let tb = unsafe { &mut *tb };
    let highlight = unsafe { *highlight_ptr };
    tb.add_highlight_by_char_range(
        highlight.start,
        highlight.end,
        highlight.style_id,
        highlight.priority,
        highlight.hl_ref,
    );
}

#[unsafe(no_mangle)]
pub extern "C" fn textBufferAddHighlight(
    tb: *mut NativeTextBuffer,
    line_idx: u32,
    highlight_ptr: *const NativeHighlight,
) {
    if tb.is_null() || highlight_ptr.is_null() {
        return;
    }

    let tb = unsafe { &mut *tb };
    let highlight = unsafe { *highlight_ptr };
    tb.add_highlight(
        line_idx as usize,
        highlight.start,
        highlight.end,
        highlight.style_id,
        highlight.priority,
        highlight.hl_ref,
    );
}

#[unsafe(no_mangle)]
pub extern "C" fn textBufferRemoveHighlightsByRef(tb: *mut NativeTextBuffer, hl_ref: u16) {
    if tb.is_null() {
        return;
    }

    let tb = unsafe { &mut *tb };
    tb.remove_highlights_by_ref(hl_ref);
}

#[unsafe(no_mangle)]
pub extern "C" fn textBufferClearLineHighlights(tb: *mut NativeTextBuffer, line_idx: u32) {
    if tb.is_null() {
        return;
    }

    let tb = unsafe { &mut *tb };
    tb.clear_line_highlights(line_idx as usize);
}

#[unsafe(no_mangle)]
pub extern "C" fn textBufferClearAllHighlights(tb: *mut NativeTextBuffer) {
    if tb.is_null() {
        return;
    }

    let tb = unsafe { &mut *tb };
    tb.clear_all_highlights();
}

#[unsafe(no_mangle)]
pub extern "C" fn textBufferSetSyntaxStyle(
    tb: *mut NativeTextBuffer,
    style: *const SyntaxStyleState,
) {
    if tb.is_null() {
        return;
    }

    let tb = unsafe { &mut *tb };
    let style = (!style.is_null()).then_some(style);
    tb.set_syntax_style(style);
}

#[unsafe(no_mangle)]
pub extern "C" fn textBufferGetLineHighlightsPtr(
    tb: *const NativeTextBuffer,
    line_idx: u32,
    out_count: *mut usize,
) -> *const NativeHighlight {
    if tb.is_null() || out_count.is_null() {
        return core::ptr::null();
    }

    let tb = unsafe { &*tb };
    let highlights = tb.get_line_highlights(line_idx as usize);
    unsafe {
        *out_count = highlights.len();
    }
    if highlights.is_empty() {
        return core::ptr::null();
    }

    let boxed: Box<[NativeHighlight]> = highlights.to_vec().into_boxed_slice();
    let ptr = boxed.as_ptr();
    std::mem::forget(boxed);
    ptr
}

#[unsafe(no_mangle)]
pub extern "C" fn textBufferFreeLineHighlights(ptr: *const NativeHighlight, count: usize) {
    if ptr.is_null() || count == 0 {
        return;
    }

    unsafe {
        let _ = Vec::from_raw_parts(ptr as *mut NativeHighlight, count, count);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn textBufferGetHighlightCount(tb: *const NativeTextBuffer) -> u32 {
    if tb.is_null() {
        return 0;
    }

    let tb = unsafe { &*tb };
    tb.get_highlight_count()
}

#[unsafe(no_mangle)]
pub extern "C" fn createTextBufferView(tb: *mut NativeTextBuffer) -> *mut NativeTextBufferView {
    if tb.is_null() {
        return core::ptr::null_mut();
    }

    Box::into_raw(Box::new(TextBufferViewState::new(tb)))
}

#[unsafe(no_mangle)]
pub extern "C" fn destroyTextBufferView(view: *mut NativeTextBufferView) {
    if view.is_null() {
        return;
    }

    unsafe {
        drop(Box::from_raw(view));
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn textBufferViewSetSelection(
    view: *mut NativeTextBufferView,
    start: u32,
    end: u32,
    bg_color: *const f32,
    fg_color: *const f32,
) {
    if view.is_null() {
        return;
    }

    let view = unsafe { &mut *view };
    view.set_selection(
        start,
        end,
        (!bg_color.is_null()).then(|| color_from_ptr(bg_color)),
        (!fg_color.is_null()).then(|| color_from_ptr(fg_color)),
    );
}

#[unsafe(no_mangle)]
pub extern "C" fn textBufferViewResetSelection(view: *mut NativeTextBufferView) {
    if view.is_null() {
        return;
    }

    let view = unsafe { &mut *view };
    view.reset_selection();
}

#[unsafe(no_mangle)]
pub extern "C" fn textBufferViewGetSelectionInfo(view: *const NativeTextBufferView) -> u64 {
    if view.is_null() {
        return NO_SELECTION;
    }

    let view = unsafe { &*view };
    view.selection_info()
}

#[unsafe(no_mangle)]
pub extern "C" fn textBufferViewUpdateSelection(
    view: *mut NativeTextBufferView,
    end: u32,
    bg_color: *const f32,
    fg_color: *const f32,
) {
    if view.is_null() {
        return;
    }

    let view = unsafe { &mut *view };
    view.update_selection(
        end,
        (!bg_color.is_null()).then(|| color_from_ptr(bg_color)),
        (!fg_color.is_null()).then(|| color_from_ptr(fg_color)),
    );
}

#[unsafe(no_mangle)]
pub extern "C" fn textBufferViewSetWrapWidth(view: *mut NativeTextBufferView, width: u32) {
    if view.is_null() {
        return;
    }

    let view = unsafe { &mut *view };
    view.set_wrap_width(width);
}

#[unsafe(no_mangle)]
pub extern "C" fn textBufferViewSetWrapMode(view: *mut NativeTextBufferView, mode: u8) {
    if view.is_null() {
        return;
    }

    let view = unsafe { &mut *view };
    view.set_wrap_mode(mode);
}

#[unsafe(no_mangle)]
pub extern "C" fn textBufferViewSetViewportSize(
    view: *mut NativeTextBufferView,
    width: u32,
    height: u32,
) {
    if view.is_null() {
        return;
    }

    let view = unsafe { &mut *view };
    view.set_viewport_size(width, height);
}

#[unsafe(no_mangle)]
pub extern "C" fn textBufferViewSetViewport(
    view: *mut NativeTextBufferView,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
) {
    if view.is_null() {
        return;
    }

    let view = unsafe { &mut *view };
    view.set_viewport(x, y, width, height);
}

#[unsafe(no_mangle)]
pub extern "C" fn textBufferViewGetVirtualLineCount(view: *const NativeTextBufferView) -> u32 {
    if view.is_null() {
        return 0;
    }

    let view = unsafe { &*view };
    view.virtual_line_count()
}

#[unsafe(no_mangle)]
pub extern "C" fn textBufferViewGetSelectedText(
    view: *const NativeTextBufferView,
    out_ptr: *mut u8,
    max_len: usize,
) -> usize {
    if view.is_null() {
        return 0;
    }

    let view = unsafe { &*view };
    copy_selected_text(view, out_ptr, max_len)
}

#[unsafe(no_mangle)]
pub extern "C" fn textBufferViewGetPlainText(
    view: *const NativeTextBufferView,
    out_ptr: *mut u8,
    max_len: usize,
) -> usize {
    if view.is_null() {
        return 0;
    }

    let view = unsafe { &*view };
    copy_bytes_to_out(view.plain_text_bytes(), out_ptr, max_len)
}

#[unsafe(no_mangle)]
pub extern "C" fn textBufferViewSetLocalSelection(
    view: *mut NativeTextBufferView,
    anchor_x: i32,
    anchor_y: i32,
    focus_x: i32,
    focus_y: i32,
    bg_color: *const f32,
    fg_color: *const f32,
) -> bool {
    if view.is_null() {
        return false;
    }

    let view = unsafe { &mut *view };
    view.set_local_selection(
        anchor_x,
        anchor_y,
        focus_x,
        focus_y,
        (!bg_color.is_null()).then(|| color_from_ptr(bg_color)),
        (!fg_color.is_null()).then(|| color_from_ptr(fg_color)),
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn textBufferViewUpdateLocalSelection(
    view: *mut NativeTextBufferView,
    anchor_x: i32,
    anchor_y: i32,
    focus_x: i32,
    focus_y: i32,
    bg_color: *const f32,
    fg_color: *const f32,
) -> bool {
    if view.is_null() {
        return false;
    }

    let view = unsafe { &mut *view };
    view.update_local_selection(
        anchor_x,
        anchor_y,
        focus_x,
        focus_y,
        (!bg_color.is_null()).then(|| color_from_ptr(bg_color)),
        (!fg_color.is_null()).then(|| color_from_ptr(fg_color)),
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn textBufferViewResetLocalSelection(view: *mut NativeTextBufferView) {
    if view.is_null() {
        return;
    }

    let view = unsafe { &mut *view };
    view.reset_local_selection();
}

#[unsafe(no_mangle)]
pub extern "C" fn textBufferViewGetLineInfoDirect(
    view: *mut NativeTextBufferView,
    out_ptr: *mut NativeLineInfo,
) {
    if view.is_null() || out_ptr.is_null() {
        return;
    }

    let view = unsafe { &mut *view };
    unsafe {
        *out_ptr = view.line_info();
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn textBufferViewGetLogicalLineInfoDirect(
    view: *mut NativeTextBufferView,
    out_ptr: *mut NativeLineInfo,
) {
    if view.is_null() || out_ptr.is_null() {
        return;
    }

    let view = unsafe { &mut *view };
    unsafe {
        *out_ptr = view.logical_line_info();
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn textBufferViewSetTabIndicator(view: *mut NativeTextBufferView, indicator: u32) {
    if view.is_null() {
        return;
    }

    let view = unsafe { &mut *view };
    view.set_tab_indicator(indicator);
}

#[unsafe(no_mangle)]
pub extern "C" fn textBufferViewSetTabIndicatorColor(
    view: *mut NativeTextBufferView,
    color: *const f32,
) {
    if view.is_null() || color.is_null() {
        return;
    }

    let view = unsafe { &mut *view };
    view.set_tab_indicator_color(color_from_ptr(color));
}

#[unsafe(no_mangle)]
pub extern "C" fn textBufferViewSetTruncate(view: *mut NativeTextBufferView, truncate: bool) {
    if view.is_null() {
        return;
    }

    let view = unsafe { &mut *view };
    view.set_truncate(truncate);
}

#[unsafe(no_mangle)]
pub extern "C" fn textBufferViewMeasureForDimensions(
    view: *mut NativeTextBufferView,
    width: u32,
    height: u32,
    out_ptr: *mut NativeMeasureResult,
) -> bool {
    if view.is_null() || out_ptr.is_null() {
        return false;
    }

    let view = unsafe { &mut *view };
    unsafe {
        *out_ptr = view.measure_for_dimensions(width, height);
    }
    true
}

#[unsafe(no_mangle)]
pub extern "C" fn createEditBuffer(width_method: u8) -> *mut NativeEditBuffer {
    Box::into_raw(Box::new(EditBufferState::new(width_method)))
}

#[unsafe(no_mangle)]
pub extern "C" fn destroyEditBuffer(buffer: *mut NativeEditBuffer) {
    if buffer.is_null() {
        return;
    }

    unsafe {
        drop(Box::from_raw(buffer));
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn editBufferSetText(
    buffer: *mut NativeEditBuffer,
    text_ptr: *const u8,
    text_len: usize,
) {
    if buffer.is_null() || text_ptr.is_null() {
        return;
    }

    let buffer = unsafe { &mut *buffer };
    let data = unsafe { std::slice::from_raw_parts(text_ptr, text_len) };
    buffer.set_text_bytes(data);
}

#[unsafe(no_mangle)]
pub extern "C" fn editBufferSetTextFromMem(buffer: *mut NativeEditBuffer, mem_id: u8) {
    if buffer.is_null() {
        return;
    }

    let buffer = unsafe { &mut *buffer };
    buffer.set_text_from_mem(mem_id);
}

#[unsafe(no_mangle)]
pub extern "C" fn editBufferReplaceText(
    buffer: *mut NativeEditBuffer,
    text_ptr: *const u8,
    text_len: usize,
) {
    if buffer.is_null() || text_ptr.is_null() {
        return;
    }

    let buffer = unsafe { &mut *buffer };
    let data = unsafe { std::slice::from_raw_parts(text_ptr, text_len) };
    buffer.replace_text_bytes(data);
}

#[unsafe(no_mangle)]
pub extern "C" fn editBufferReplaceTextFromMem(buffer: *mut NativeEditBuffer, mem_id: u8) {
    if buffer.is_null() {
        return;
    }

    let buffer = unsafe { &mut *buffer };
    buffer.replace_text_from_mem(mem_id);
}

#[unsafe(no_mangle)]
pub extern "C" fn editBufferGetText(
    buffer: *const NativeEditBuffer,
    out_ptr: *mut u8,
    max_len: usize,
) -> usize {
    if buffer.is_null() {
        return 0;
    }

    let buffer = unsafe { &*buffer };
    copy_bytes_to_out(buffer.text_bytes(), out_ptr, max_len)
}

#[unsafe(no_mangle)]
pub extern "C" fn editBufferInsertChar(
    buffer: *mut NativeEditBuffer,
    char_ptr: *const u8,
    char_len: usize,
) {
    if buffer.is_null() || char_ptr.is_null() {
        return;
    }

    let buffer = unsafe { &mut *buffer };
    let data = unsafe { std::slice::from_raw_parts(char_ptr, char_len) };
    buffer.insert_char(data);
}

#[unsafe(no_mangle)]
pub extern "C" fn editBufferInsertText(
    buffer: *mut NativeEditBuffer,
    text_ptr: *const u8,
    text_len: usize,
) {
    if buffer.is_null() || text_ptr.is_null() {
        return;
    }

    let buffer = unsafe { &mut *buffer };
    let data = unsafe { std::slice::from_raw_parts(text_ptr, text_len) };
    buffer.insert_text(data);
}

#[unsafe(no_mangle)]
pub extern "C" fn editBufferDeleteChar(buffer: *mut NativeEditBuffer) {
    if buffer.is_null() {
        return;
    }

    let buffer = unsafe { &mut *buffer };
    buffer.delete_char();
}

#[unsafe(no_mangle)]
pub extern "C" fn editBufferDeleteCharBackward(buffer: *mut NativeEditBuffer) {
    if buffer.is_null() {
        return;
    }

    let buffer = unsafe { &mut *buffer };
    buffer.delete_char_backward();
}

#[unsafe(no_mangle)]
pub extern "C" fn editBufferDeleteRange(
    buffer: *mut NativeEditBuffer,
    start_row: u32,
    start_col: u32,
    end_row: u32,
    end_col: u32,
) {
    if buffer.is_null() {
        return;
    }

    let buffer = unsafe { &mut *buffer };
    buffer.delete_range_by_coords(start_row, start_col, end_row, end_col);
}

#[unsafe(no_mangle)]
pub extern "C" fn editBufferNewLine(buffer: *mut NativeEditBuffer) {
    if buffer.is_null() {
        return;
    }

    let buffer = unsafe { &mut *buffer };
    buffer.new_line();
}

#[unsafe(no_mangle)]
pub extern "C" fn editBufferMoveCursorLeft(buffer: *mut NativeEditBuffer) {
    if buffer.is_null() {
        return;
    }

    let buffer = unsafe { &mut *buffer };
    buffer.move_cursor_left();
}

#[unsafe(no_mangle)]
pub extern "C" fn editBufferMoveCursorRight(buffer: *mut NativeEditBuffer) {
    if buffer.is_null() {
        return;
    }

    let buffer = unsafe { &mut *buffer };
    buffer.move_cursor_right();
}

#[unsafe(no_mangle)]
pub extern "C" fn editBufferMoveCursorUp(buffer: *mut NativeEditBuffer) {
    if buffer.is_null() {
        return;
    }

    let buffer = unsafe { &mut *buffer };
    buffer.move_cursor_up();
}

#[unsafe(no_mangle)]
pub extern "C" fn editBufferMoveCursorDown(buffer: *mut NativeEditBuffer) {
    if buffer.is_null() {
        return;
    }

    let buffer = unsafe { &mut *buffer };
    buffer.move_cursor_down();
}

#[unsafe(no_mangle)]
pub extern "C" fn editBufferGotoLine(buffer: *mut NativeEditBuffer, line: u32) {
    if buffer.is_null() {
        return;
    }

    let buffer = unsafe { &mut *buffer };
    buffer.goto_line(line);
}

#[unsafe(no_mangle)]
pub extern "C" fn editBufferSetCursor(buffer: *mut NativeEditBuffer, line: u32, col: u32) {
    editBufferSetCursorToLineCol(buffer, line, col);
}

#[unsafe(no_mangle)]
pub extern "C" fn editBufferSetCursorToLineCol(buffer: *mut NativeEditBuffer, line: u32, col: u32) {
    if buffer.is_null() {
        return;
    }

    let buffer = unsafe { &mut *buffer };
    buffer.set_cursor_to_line_col(line, col);
}

#[unsafe(no_mangle)]
pub extern "C" fn editBufferSetCursorByOffset(buffer: *mut NativeEditBuffer, offset: u32) {
    if buffer.is_null() {
        return;
    }

    let buffer = unsafe { &mut *buffer };
    buffer.set_cursor_by_offset(offset);
}

#[unsafe(no_mangle)]
pub extern "C" fn editBufferGetCursorPosition(
    buffer: *const NativeEditBuffer,
    out_ptr: *mut NativeLogicalCursor,
) {
    if buffer.is_null() || out_ptr.is_null() {
        return;
    }

    let buffer = unsafe { &*buffer };
    unsafe {
        *out_ptr = buffer.cursor();
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn editBufferGetId(buffer: *const NativeEditBuffer) -> u16 {
    if buffer.is_null() {
        return 0;
    }

    let buffer = unsafe { &*buffer };
    buffer.id()
}

#[unsafe(no_mangle)]
pub extern "C" fn editBufferGetTextBuffer(buffer: *mut NativeEditBuffer) -> *mut NativeTextBuffer {
    if buffer.is_null() {
        return core::ptr::null_mut();
    }

    let buffer = unsafe { &mut *buffer };
    buffer.text_buffer_ptr()
}

#[unsafe(no_mangle)]
pub extern "C" fn editBufferClear(buffer: *mut NativeEditBuffer) {
    if buffer.is_null() {
        return;
    }

    let buffer = unsafe { &mut *buffer };
    buffer.clear();
}

#[unsafe(no_mangle)]
pub extern "C" fn editBufferOffsetToPosition(
    buffer: *const NativeEditBuffer,
    offset: u32,
    out_ptr: *mut NativeLogicalCursor,
) -> bool {
    if buffer.is_null() || out_ptr.is_null() {
        return false;
    }

    let buffer = unsafe { &*buffer };
    let Some(cursor) = buffer.offset_to_position(offset) else {
        return false;
    };
    unsafe {
        *out_ptr = cursor;
    }
    true
}

#[unsafe(no_mangle)]
pub extern "C" fn editBufferPositionToOffset(
    buffer: *const NativeEditBuffer,
    row: u32,
    col: u32,
) -> u32 {
    if buffer.is_null() {
        return 0;
    }

    let buffer = unsafe { &*buffer };
    buffer.position_to_offset(row, col)
}

#[unsafe(no_mangle)]
pub extern "C" fn editBufferGetLineStartOffset(buffer: *const NativeEditBuffer, row: u32) -> u32 {
    if buffer.is_null() {
        return 0;
    }

    let buffer = unsafe { &*buffer };
    buffer.line_start_offset(row)
}

#[unsafe(no_mangle)]
pub extern "C" fn editBufferGetTextRange(
    buffer: *const NativeEditBuffer,
    start_offset: u32,
    end_offset: u32,
    out_ptr: *mut u8,
    max_len: usize,
) -> usize {
    if buffer.is_null() {
        return 0;
    }

    let buffer = unsafe { &*buffer };
    let text = buffer.text_buffer_text_range(start_offset, end_offset);
    copy_bytes_to_out(text.as_bytes(), out_ptr, max_len)
}

#[unsafe(no_mangle)]
pub extern "C" fn editBufferGetTextRangeByCoords(
    buffer: *const NativeEditBuffer,
    start_row: u32,
    start_col: u32,
    end_row: u32,
    end_col: u32,
    out_ptr: *mut u8,
    max_len: usize,
) -> usize {
    if buffer.is_null() {
        return 0;
    }

    let buffer = unsafe { &*buffer };
    let text = buffer.text_buffer_text_range_by_coords(start_row, start_col, end_row, end_col);
    copy_bytes_to_out(text.as_bytes(), out_ptr, max_len)
}

#[unsafe(no_mangle)]
pub extern "C" fn editBufferDeleteLine(buffer: *mut NativeEditBuffer) {
    if buffer.is_null() {
        return;
    }

    let buffer = unsafe { &mut *buffer };
    buffer.delete_line();
}

#[unsafe(no_mangle)]
pub extern "C" fn editBufferGetNextWordBoundary(
    buffer: *const NativeEditBuffer,
    out_ptr: *mut NativeLogicalCursor,
) {
    if buffer.is_null() || out_ptr.is_null() {
        return;
    }

    let buffer = unsafe { &*buffer };
    unsafe {
        *out_ptr = buffer.next_word_boundary();
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn editBufferGetPrevWordBoundary(
    buffer: *const NativeEditBuffer,
    out_ptr: *mut NativeLogicalCursor,
) {
    if buffer.is_null() || out_ptr.is_null() {
        return;
    }

    let buffer = unsafe { &*buffer };
    unsafe {
        *out_ptr = buffer.prev_word_boundary();
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn editBufferGetEOL(
    buffer: *const NativeEditBuffer,
    out_ptr: *mut NativeLogicalCursor,
) {
    if buffer.is_null() || out_ptr.is_null() {
        return;
    }

    let buffer = unsafe { &*buffer };
    unsafe {
        *out_ptr = buffer.eol();
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn editBufferDebugLogRope(buffer: *const NativeEditBuffer) {
    if buffer.is_null() {
        return;
    }

    let buffer = unsafe { &*buffer };
    buffer.debug_log_rope();
}

#[unsafe(no_mangle)]
pub extern "C" fn editBufferUndo(
    buffer: *mut NativeEditBuffer,
    out_ptr: *mut u8,
    max_len: usize,
) -> usize {
    if buffer.is_null() {
        return 0;
    }

    let buffer = unsafe { &mut *buffer };
    let Some(meta) = buffer.undo() else {
        return 0;
    };
    copy_bytes_to_out(meta.as_bytes(), out_ptr, max_len)
}

#[unsafe(no_mangle)]
pub extern "C" fn editBufferRedo(
    buffer: *mut NativeEditBuffer,
    out_ptr: *mut u8,
    max_len: usize,
) -> usize {
    if buffer.is_null() {
        return 0;
    }

    let buffer = unsafe { &mut *buffer };
    let Some(meta) = buffer.redo() else {
        return 0;
    };
    copy_bytes_to_out(meta.as_bytes(), out_ptr, max_len)
}

#[unsafe(no_mangle)]
pub extern "C" fn editBufferCanUndo(buffer: *const NativeEditBuffer) -> bool {
    if buffer.is_null() {
        return false;
    }

    let buffer = unsafe { &*buffer };
    buffer.can_undo()
}

#[unsafe(no_mangle)]
pub extern "C" fn editBufferCanRedo(buffer: *const NativeEditBuffer) -> bool {
    if buffer.is_null() {
        return false;
    }

    let buffer = unsafe { &*buffer };
    buffer.can_redo()
}

#[unsafe(no_mangle)]
pub extern "C" fn editBufferClearHistory(buffer: *mut NativeEditBuffer) {
    if buffer.is_null() {
        return;
    }

    let buffer = unsafe { &mut *buffer };
    buffer.clear_history();
}

#[unsafe(no_mangle)]
pub extern "C" fn createEditorView(
    edit_buffer_ptr: *mut NativeEditBuffer,
    viewport_width: u32,
    viewport_height: u32,
) -> *mut NativeEditorView {
    if edit_buffer_ptr.is_null() {
        return core::ptr::null_mut();
    }

    Box::into_raw(Box::new(EditorViewState::new(
        edit_buffer_ptr,
        viewport_width,
        viewport_height,
    )))
}

#[unsafe(no_mangle)]
pub extern "C" fn destroyEditorView(view: *mut NativeEditorView) {
    if view.is_null() {
        return;
    }

    unsafe {
        drop(Box::from_raw(view));
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn editorViewSetViewportSize(view: *mut NativeEditorView, width: u32, height: u32) {
    if view.is_null() {
        return;
    }

    let view = unsafe { &mut *view };
    view.set_viewport_size(width, height);
}

#[unsafe(no_mangle)]
pub extern "C" fn editorViewSetViewport(
    view: *mut NativeEditorView,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    move_cursor: bool,
) {
    if view.is_null() {
        return;
    }

    let view = unsafe { &mut *view };
    view.set_viewport(x, y, width, height, move_cursor);
}

#[unsafe(no_mangle)]
pub extern "C" fn editorViewGetViewport(
    view: *const NativeEditorView,
    x_ptr: *mut u32,
    y_ptr: *mut u32,
    width_ptr: *mut u32,
    height_ptr: *mut u32,
) {
    if view.is_null()
        || x_ptr.is_null()
        || y_ptr.is_null()
        || width_ptr.is_null()
        || height_ptr.is_null()
    {
        return;
    }

    let view = unsafe { &*view };
    let (x, y, width, height) = view.viewport();
    unsafe {
        *x_ptr = x;
        *y_ptr = y;
        *width_ptr = width;
        *height_ptr = height;
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn editorViewSetScrollMargin(view: *mut NativeEditorView, margin: f32) {
    if view.is_null() {
        return;
    }

    let view = unsafe { &mut *view };
    view.set_scroll_margin(margin);
}

#[unsafe(no_mangle)]
pub extern "C" fn editorViewSetWrapMode(view: *mut NativeEditorView, mode: u8) {
    if view.is_null() {
        return;
    }

    let view = unsafe { &mut *view };
    view.set_wrap_mode(mode);
}

#[unsafe(no_mangle)]
pub extern "C" fn editorViewGetVirtualLineCount(view: *const NativeEditorView) -> u32 {
    if view.is_null() {
        return 0;
    }

    let view = unsafe { &*view };
    view.virtual_line_count()
}

#[unsafe(no_mangle)]
pub extern "C" fn editorViewGetTotalVirtualLineCount(view: *const NativeEditorView) -> u32 {
    if view.is_null() {
        return 0;
    }

    let view = unsafe { &*view };
    view.total_virtual_line_count()
}

#[unsafe(no_mangle)]
pub extern "C" fn editorViewGetTextBufferView(
    view: *mut NativeEditorView,
) -> *mut NativeTextBufferView {
    if view.is_null() {
        return core::ptr::null_mut();
    }

    let view = unsafe { &mut *view };
    view.text_buffer_view_ptr()
}

#[unsafe(no_mangle)]
pub extern "C" fn editorViewSetSelection(
    view: *mut NativeEditorView,
    start: u32,
    end: u32,
    bg_color: *const f32,
    fg_color: *const f32,
) {
    if view.is_null() {
        return;
    }

    let view = unsafe { &mut *view };
    view.set_selection(
        start,
        end,
        (!bg_color.is_null()).then(|| color_from_ptr(bg_color)),
        (!fg_color.is_null()).then(|| color_from_ptr(fg_color)),
    );
}

#[unsafe(no_mangle)]
pub extern "C" fn editorViewResetSelection(view: *mut NativeEditorView) {
    if view.is_null() {
        return;
    }

    let view = unsafe { &mut *view };
    view.reset_selection();
}

#[unsafe(no_mangle)]
pub extern "C" fn editorViewGetSelection(view: *const NativeEditorView) -> u64 {
    if view.is_null() {
        return NO_SELECTION;
    }

    let view = unsafe { &*view };
    view.selection_info()
}

#[unsafe(no_mangle)]
pub extern "C" fn editorViewUpdateSelection(
    view: *mut NativeEditorView,
    end: u32,
    bg_color: *const f32,
    fg_color: *const f32,
) {
    if view.is_null() {
        return;
    }

    let view = unsafe { &mut *view };
    view.update_selection(
        end,
        (!bg_color.is_null()).then(|| color_from_ptr(bg_color)),
        (!fg_color.is_null()).then(|| color_from_ptr(fg_color)),
    );
}

#[unsafe(no_mangle)]
pub extern "C" fn editorViewGetSelectedTextBytes(
    view: *const NativeEditorView,
    out_ptr: *mut u8,
    max_len: usize,
) -> usize {
    if view.is_null() {
        return 0;
    }

    let view = unsafe { &*view };
    let data = view.selected_text_bytes();
    copy_bytes_to_out(&data, out_ptr, max_len)
}

#[unsafe(no_mangle)]
pub extern "C" fn editorViewGetCursor(
    view: *const NativeEditorView,
    row_ptr: *mut u32,
    col_ptr: *mut u32,
) {
    if view.is_null() || row_ptr.is_null() || col_ptr.is_null() {
        return;
    }

    let view = unsafe { &*view };
    let (row, col) = view.cursor();
    unsafe {
        *row_ptr = row;
        *col_ptr = col;
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn editorViewGetText(
    view: *const NativeEditorView,
    out_ptr: *mut u8,
    max_len: usize,
) -> usize {
    if view.is_null() {
        return 0;
    }

    let view = unsafe { &*view };
    copy_bytes_to_out(view.text_bytes(), out_ptr, max_len)
}

#[unsafe(no_mangle)]
pub extern "C" fn editorViewGetLineInfoDirect(
    view: *mut NativeEditorView,
    out_ptr: *mut NativeLineInfo,
) {
    if view.is_null() || out_ptr.is_null() {
        return;
    }

    let view = unsafe { &mut *view };
    unsafe {
        *out_ptr = view.line_info();
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn editorViewGetLogicalLineInfoDirect(
    view: *mut NativeEditorView,
    out_ptr: *mut NativeLineInfo,
) {
    if view.is_null() || out_ptr.is_null() {
        return;
    }

    let view = unsafe { &mut *view };
    unsafe {
        *out_ptr = view.logical_line_info();
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn editorViewSetPlaceholderStyledText(
    view: *mut NativeEditorView,
    chunks_ptr: *const NativeStyledChunk,
    chunk_count: usize,
) {
    if view.is_null() {
        return;
    }

    let view = unsafe { &mut *view };
    let chunks = if chunks_ptr.is_null() || chunk_count == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(chunks_ptr, chunk_count) }
    };
    view.set_placeholder_styled_text(chunks);
}

#[unsafe(no_mangle)]
pub extern "C" fn editorViewSetTabIndicator(view: *mut NativeEditorView, indicator: u32) {
    if view.is_null() {
        return;
    }

    let view = unsafe { &mut *view };
    view.set_tab_indicator(indicator);
}

#[unsafe(no_mangle)]
pub extern "C" fn editorViewSetTabIndicatorColor(view: *mut NativeEditorView, color: *const f32) {
    if view.is_null() || color.is_null() {
        return;
    }

    let view = unsafe { &mut *view };
    view.set_tab_indicator_color(color_from_ptr(color));
}

#[unsafe(no_mangle)]
pub extern "C" fn createOptimizedBuffer(
    width: u32,
    height: u32,
    respect_alpha: bool,
    _width_method: u8,
    id_ptr: *const u8,
    id_len: usize,
) -> *mut NativeOptimizedBuffer {
    if width == 0 || height == 0 {
        return core::ptr::null_mut();
    }

    let width = usize::try_from(width).ok();
    let height = usize::try_from(height).ok();
    let (Some(width), Some(height)) = (width, height) else {
        return core::ptr::null_mut();
    };

    let id = if id_ptr.is_null() || id_len == 0 {
        b"unnamed".to_vec()
    } else {
        unsafe { std::slice::from_raw_parts(id_ptr, id_len) }.to_vec()
    };

    Box::into_raw(Box::new(OptimizedBuffer::with_id(
        width,
        height,
        respect_alpha,
        id,
    )))
}

#[unsafe(no_mangle)]
pub extern "C" fn destroyOptimizedBuffer(buffer: *mut NativeOptimizedBuffer) {
    if buffer.is_null() {
        return;
    }

    unsafe {
        drop(Box::from_raw(buffer));
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn getBufferWidth(buffer: *const NativeOptimizedBuffer) -> u32 {
    if buffer.is_null() {
        return 0;
    }

    let buffer = unsafe { &*buffer };
    u32::try_from(buffer.width()).unwrap_or(u32::MAX)
}

#[unsafe(no_mangle)]
pub extern "C" fn getBufferHeight(buffer: *const NativeOptimizedBuffer) -> u32 {
    if buffer.is_null() {
        return 0;
    }

    let buffer = unsafe { &*buffer };
    u32::try_from(buffer.height()).unwrap_or(u32::MAX)
}

#[unsafe(no_mangle)]
pub extern "C" fn bufferGetCharPtr(buffer: *const NativeOptimizedBuffer) -> *const u32 {
    if buffer.is_null() {
        return core::ptr::null();
    }

    let buffer = unsafe { &*buffer };
    buffer.chars_ptr()
}

#[unsafe(no_mangle)]
pub extern "C" fn bufferGetFgPtr(buffer: *const NativeOptimizedBuffer) -> *const f32 {
    if buffer.is_null() {
        return core::ptr::null();
    }

    let buffer = unsafe { &*buffer };
    buffer.fg_ptr()
}

#[unsafe(no_mangle)]
pub extern "C" fn bufferGetBgPtr(buffer: *const NativeOptimizedBuffer) -> *const f32 {
    if buffer.is_null() {
        return core::ptr::null();
    }

    let buffer = unsafe { &*buffer };
    buffer.bg_ptr()
}

#[unsafe(no_mangle)]
pub extern "C" fn bufferGetAttributesPtr(buffer: *const NativeOptimizedBuffer) -> *const u32 {
    if buffer.is_null() {
        return core::ptr::null();
    }

    let buffer = unsafe { &*buffer };
    buffer.attributes_ptr()
}

#[unsafe(no_mangle)]
pub extern "C" fn bufferClear(buffer: *mut NativeOptimizedBuffer, bg: *const f32) {
    if buffer.is_null() {
        return;
    }

    let buffer = unsafe { &mut *buffer };
    let bg = if bg.is_null() {
        [0.0, 0.0, 0.0, 1.0]
    } else {
        color_from_ptr(bg)
    };
    buffer.clear_with_bg(bg);
}

#[unsafe(no_mangle)]
pub extern "C" fn bufferGetRespectAlpha(buffer: *const NativeOptimizedBuffer) -> bool {
    if buffer.is_null() {
        return false;
    }
    let buffer = unsafe { &*buffer };
    buffer.respect_alpha()
}

#[unsafe(no_mangle)]
pub extern "C" fn bufferSetRespectAlpha(buffer: *mut NativeOptimizedBuffer, respect_alpha: bool) {
    if buffer.is_null() {
        return;
    }
    let buffer = unsafe { &mut *buffer };
    buffer.set_respect_alpha(respect_alpha);
}

#[unsafe(no_mangle)]
pub extern "C" fn bufferGetId(
    buffer: *const NativeOptimizedBuffer,
    out_ptr: *mut u8,
    max_len: usize,
) -> usize {
    if buffer.is_null() {
        return 0;
    }
    let buffer = unsafe { &*buffer };
    copy_bytes_to_out(buffer.id_bytes(), out_ptr, max_len)
}

#[unsafe(no_mangle)]
pub extern "C" fn bufferGetRealCharSize(buffer: *const NativeOptimizedBuffer) -> u32 {
    if buffer.is_null() {
        return 0;
    }
    let buffer = unsafe { &*buffer };
    u32::try_from(buffer.real_char_size()).unwrap_or(u32::MAX)
}

#[unsafe(no_mangle)]
pub extern "C" fn bufferWriteResolvedChars(
    buffer: *const NativeOptimizedBuffer,
    output_ptr: *mut u8,
    output_len: usize,
    add_line_breaks: bool,
) -> u32 {
    if buffer.is_null() {
        return 0;
    }
    let buffer = unsafe { &*buffer };
    let bytes = buffer.write_resolved_chars_to_vec(add_line_breaks);
    u32::try_from(copy_bytes_to_out(&bytes, output_ptr, output_len)).unwrap_or(u32::MAX)
}

#[unsafe(no_mangle)]
pub extern "C" fn bufferSetCell(
    buffer: *mut NativeOptimizedBuffer,
    x: u32,
    y: u32,
    char_code: u32,
    fg: *const f32,
    bg: *const f32,
    attributes: u32,
) {
    if buffer.is_null() || fg.is_null() || bg.is_null() {
        return;
    }
    let buffer = unsafe { &mut *buffer };
    buffer.draw_char(
        char_code,
        x as usize,
        y as usize,
        color_from_ptr(fg),
        color_from_ptr(bg),
        attributes,
    );
}

#[unsafe(no_mangle)]
pub extern "C" fn bufferSetCellWithAlphaBlending(
    buffer: *mut NativeOptimizedBuffer,
    x: u32,
    y: u32,
    char_code: u32,
    fg: *const f32,
    bg: *const f32,
    attributes: u32,
) {
    if buffer.is_null() || fg.is_null() || bg.is_null() {
        return;
    }
    let buffer = unsafe { &mut *buffer };
    buffer.set_cell_with_alpha_blending(
        x as usize,
        y as usize,
        char_code,
        color_from_ptr(fg),
        color_from_ptr(bg),
        attributes,
    );
}

#[unsafe(no_mangle)]
pub extern "C" fn bufferFillRect(
    buffer: *mut NativeOptimizedBuffer,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    bg: *const f32,
) {
    if buffer.is_null() || bg.is_null() {
        return;
    }
    let buffer = unsafe { &mut *buffer };
    buffer.fill_rect(
        x as usize,
        y as usize,
        width as usize,
        height as usize,
        color_from_ptr(bg),
    );
}

#[unsafe(no_mangle)]
pub extern "C" fn bufferDrawText(
    buffer: *mut NativeOptimizedBuffer,
    text_ptr: *const u8,
    text_len: usize,
    x: u32,
    y: u32,
    fg: *const f32,
    bg: *const f32,
    attributes: u32,
) {
    if buffer.is_null() || text_ptr.is_null() || fg.is_null() {
        return;
    }
    let buffer = unsafe { &mut *buffer };
    let text = unsafe { std::slice::from_raw_parts(text_ptr, text_len) };
    let text = std::str::from_utf8(text).unwrap_or("");
    let bg = if bg.is_null() {
        [0.0, 0.0, 0.0, 1.0]
    } else {
        color_from_ptr(bg)
    };
    let _ = buffer.draw_text(
        x as usize,
        y as usize,
        text,
        color_from_ptr(fg),
        bg,
        attributes,
    );
}

#[unsafe(no_mangle)]
pub extern "C" fn bufferDrawChar(
    buffer: *mut NativeOptimizedBuffer,
    char_code: u32,
    x: u32,
    y: u32,
    fg: *const f32,
    bg: *const f32,
    attributes: u32,
) {
    if buffer.is_null() || fg.is_null() || bg.is_null() {
        return;
    }
    let buffer = unsafe { &mut *buffer };
    buffer.draw_char(
        char_code,
        x as usize,
        y as usize,
        color_from_ptr(fg),
        color_from_ptr(bg),
        attributes,
    );
}

#[unsafe(no_mangle)]
pub extern "C" fn bufferResize(buffer: *mut NativeOptimizedBuffer, width: u32, height: u32) {
    if buffer.is_null() {
        return;
    }
    let buffer = unsafe { &mut *buffer };
    buffer.resize(width as usize, height as usize);
}

#[unsafe(no_mangle)]
pub extern "C" fn bufferPushOpacity(buffer: *mut NativeOptimizedBuffer, opacity: f32) {
    if buffer.is_null() {
        return;
    }
    let buffer = unsafe { &mut *buffer };
    buffer.push_opacity(opacity);
}

#[unsafe(no_mangle)]
pub extern "C" fn bufferPopOpacity(buffer: *mut NativeOptimizedBuffer) {
    if buffer.is_null() {
        return;
    }
    let buffer = unsafe { &mut *buffer };
    buffer.pop_opacity();
}

#[unsafe(no_mangle)]
pub extern "C" fn bufferGetCurrentOpacity(buffer: *const NativeOptimizedBuffer) -> f32 {
    if buffer.is_null() {
        return 1.0;
    }
    let buffer = unsafe { &*buffer };
    buffer.current_opacity()
}

#[unsafe(no_mangle)]
pub extern "C" fn bufferClearOpacity(buffer: *mut NativeOptimizedBuffer) {
    if buffer.is_null() {
        return;
    }
    let buffer = unsafe { &mut *buffer };
    buffer.clear_opacity();
}

#[unsafe(no_mangle)]
pub extern "C" fn bufferPushScissorRect(
    buffer: *mut NativeOptimizedBuffer,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) {
    if buffer.is_null() {
        return;
    }
    let buffer = unsafe { &mut *buffer };
    buffer.push_scissor_rect(x, y, width, height);
}

#[unsafe(no_mangle)]
pub extern "C" fn bufferPopScissorRect(buffer: *mut NativeOptimizedBuffer) {
    if buffer.is_null() {
        return;
    }
    let buffer = unsafe { &mut *buffer };
    buffer.pop_scissor_rect();
}

#[unsafe(no_mangle)]
pub extern "C" fn bufferClearScissorRects(buffer: *mut NativeOptimizedBuffer) {
    if buffer.is_null() {
        return;
    }
    let buffer = unsafe { &mut *buffer };
    buffer.clear_scissor_rects();
}

#[unsafe(no_mangle)]
pub extern "C" fn drawFrameBuffer(
    target: *mut NativeOptimizedBuffer,
    dest_x: i32,
    dest_y: i32,
    source: *mut NativeOptimizedBuffer,
    src_x: u32,
    src_y: u32,
    src_width: u32,
    src_height: u32,
) {
    if target.is_null() || source.is_null() {
        return;
    }
    let target = unsafe { &mut *target };
    let source = unsafe { &*source };
    target.draw_frame_buffer(
        dest_x,
        dest_y,
        source,
        src_x as usize,
        src_y as usize,
        (src_width > 0).then_some(src_width as usize),
        (src_height > 0).then_some(src_height as usize),
    );
}

#[unsafe(no_mangle)]
pub extern "C" fn bufferDrawGrid(
    buffer: *mut NativeOptimizedBuffer,
    border_chars: *const u32,
    fg: *const f32,
    bg: *const f32,
    column_offsets: *const i32,
    column_count: usize,
    row_offsets: *const i32,
    row_count: usize,
    options_ptr: *const NativeGridDrawOptions,
) {
    if buffer.is_null()
        || border_chars.is_null()
        || fg.is_null()
        || bg.is_null()
        || column_offsets.is_null()
        || row_offsets.is_null()
    {
        return;
    }
    let buffer = unsafe { &mut *buffer };
    let chars = unsafe { std::slice::from_raw_parts(border_chars, 11) };
    let cols = unsafe { std::slice::from_raw_parts(column_offsets, column_count + 1) };
    let rows = unsafe { std::slice::from_raw_parts(row_offsets, row_count + 1) };
    let options = unsafe { options_ptr.as_ref() }
        .copied()
        .map(|options| BufferGridDrawOptions {
            draw_inner: options.draw_inner,
            draw_outer: options.draw_outer,
        })
        .unwrap_or(BufferGridDrawOptions {
            draw_inner: true,
            draw_outer: true,
        });

    buffer.draw_grid(
        cols,
        rows,
        chars,
        color_from_ptr(fg),
        color_from_ptr(bg),
        options,
    );
}

#[unsafe(no_mangle)]
pub extern "C" fn bufferDrawBox(
    buffer: *mut NativeOptimizedBuffer,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    border_chars: *const u32,
    packed_options: u32,
    border_color: *const f32,
    background_color: *const f32,
    title_ptr: *const u8,
    title_len: u32,
) {
    if buffer.is_null()
        || border_chars.is_null()
        || border_color.is_null()
        || background_color.is_null()
    {
        return;
    }
    let buffer = unsafe { &mut *buffer };
    let chars = unsafe { std::slice::from_raw_parts(border_chars, 11) };
    let border_sides = BufferBorderSides {
        top: (packed_options & 0b1000) != 0,
        right: (packed_options & 0b0100) != 0,
        bottom: (packed_options & 0b0010) != 0,
        left: (packed_options & 0b0001) != 0,
    };
    let should_fill = ((packed_options >> 4) & 1) != 0;
    let title_alignment = ((packed_options >> 5) & 0b11) as u8;
    let title = if title_ptr.is_null() || title_len == 0 {
        None
    } else {
        Some(
            std::str::from_utf8(unsafe {
                std::slice::from_raw_parts(title_ptr, title_len as usize)
            })
            .unwrap_or(""),
        )
    };
    buffer.draw_box(
        x,
        y,
        width,
        height,
        chars,
        border_sides,
        color_from_ptr(border_color),
        color_from_ptr(background_color),
        should_fill,
        title,
        title_alignment,
    );
}

#[unsafe(no_mangle)]
pub extern "C" fn bufferColorMatrix(
    buffer: *mut NativeOptimizedBuffer,
    matrix_ptr: *const f32,
    cell_mask_ptr: *const f32,
    cell_mask_count: usize,
    strength: f32,
    target: u8,
) {
    if buffer.is_null() || matrix_ptr.is_null() || cell_mask_ptr.is_null() || cell_mask_count == 0 {
        return;
    }
    let buffer = unsafe { &mut *buffer };
    let matrix = unsafe { &*(matrix_ptr as *const [f32; 16]) };
    let cell_mask = unsafe { std::slice::from_raw_parts(cell_mask_ptr, cell_mask_count * 3) };
    buffer.color_matrix(matrix, cell_mask, strength, target);
}

#[unsafe(no_mangle)]
pub extern "C" fn bufferColorMatrixUniform(
    buffer: *mut NativeOptimizedBuffer,
    matrix_ptr: *const f32,
    strength: f32,
    target: u8,
) {
    if buffer.is_null() || matrix_ptr.is_null() {
        return;
    }
    let buffer = unsafe { &mut *buffer };
    let matrix = unsafe { &*(matrix_ptr as *const [f32; 16]) };
    buffer.color_matrix_uniform(matrix, strength, target);
}

#[unsafe(no_mangle)]
pub extern "C" fn bufferDrawPackedBuffer(
    buffer: *mut NativeOptimizedBuffer,
    data_ptr: *const u8,
    data_len: usize,
    pos_x: u32,
    pos_y: u32,
    terminal_width_cells: u32,
    terminal_height_cells: u32,
) {
    if buffer.is_null() || data_ptr.is_null() {
        return;
    }
    let buffer = unsafe { &mut *buffer };
    let data = unsafe { std::slice::from_raw_parts(data_ptr, data_len) };
    buffer.draw_packed_buffer(
        data,
        pos_x,
        pos_y,
        terminal_width_cells,
        terminal_height_cells,
    );
}

#[unsafe(no_mangle)]
pub extern "C" fn bufferDrawSuperSampleBuffer(
    buffer: *mut NativeOptimizedBuffer,
    x: u32,
    y: u32,
    pixel_ptr: *const u8,
    pixel_len: usize,
    format: u8,
    aligned_bytes_per_row: u32,
) {
    if buffer.is_null() || pixel_ptr.is_null() {
        return;
    }
    let buffer = unsafe { &mut *buffer };
    let pixel_data = unsafe { std::slice::from_raw_parts(pixel_ptr, pixel_len) };
    buffer.draw_super_sample_buffer(
        x as usize,
        y as usize,
        pixel_data,
        format,
        aligned_bytes_per_row as usize,
    );
}

#[unsafe(no_mangle)]
pub extern "C" fn bufferDrawGrayscaleBuffer(
    buffer: *mut NativeOptimizedBuffer,
    pos_x: i32,
    pos_y: i32,
    intensities_ptr: *const f32,
    src_width: u32,
    src_height: u32,
    fg: *const f32,
    bg: *const f32,
) {
    if buffer.is_null() || intensities_ptr.is_null() || pos_x < 0 || pos_y < 0 {
        return;
    }
    let buffer = unsafe { &mut *buffer };
    let intensities = unsafe {
        std::slice::from_raw_parts(
            intensities_ptr,
            (src_width as usize) * (src_height as usize),
        )
    };
    let fg = if fg.is_null() {
        [1.0; 4]
    } else {
        color_from_ptr(fg)
    };
    let bg = if bg.is_null() {
        [0.0, 0.0, 0.0, 1.0]
    } else {
        color_from_ptr(bg)
    };
    buffer.draw_grayscale(
        pos_x as usize,
        pos_y as usize,
        intensities,
        src_width as usize,
        src_height as usize,
        fg,
        bg,
    );
}

#[unsafe(no_mangle)]
pub extern "C" fn bufferDrawGrayscaleBufferSupersampled(
    buffer: *mut NativeOptimizedBuffer,
    pos_x: i32,
    pos_y: i32,
    intensities_ptr: *const f32,
    src_width: u32,
    src_height: u32,
    fg: *const f32,
    bg: *const f32,
) {
    if buffer.is_null() || intensities_ptr.is_null() || pos_x < 0 || pos_y < 0 {
        return;
    }
    let buffer = unsafe { &mut *buffer };
    let intensities = unsafe {
        std::slice::from_raw_parts(
            intensities_ptr,
            (src_width as usize) * (src_height as usize),
        )
    };
    let fg = if fg.is_null() {
        [1.0; 4]
    } else {
        color_from_ptr(fg)
    };
    let bg = if bg.is_null() {
        [0.0, 0.0, 0.0, 1.0]
    } else {
        color_from_ptr(bg)
    };
    buffer.draw_grayscale_supersampled(
        pos_x as usize,
        pos_y as usize,
        intensities,
        src_width as usize,
        src_height as usize,
        fg,
        bg,
    );
}

#[unsafe(no_mangle)]
pub extern "C" fn bufferDrawTextBufferView(
    buffer: *mut NativeOptimizedBuffer,
    view: *mut NativeTextBufferView,
    x: i32,
    y: i32,
) {
    if buffer.is_null() || view.is_null() || x < 0 {
        return;
    }

    let buffer = unsafe { &mut *buffer };
    let view = unsafe { &*view };
    let (selection_bg, selection_fg) = view.selection_colors();
    let default_fg = view.default_fg().unwrap_or([1.0, 1.0, 1.0, 1.0]);
    let default_bg = view.default_bg().unwrap_or([0.0, 0.0, 0.0, 0.0]);
    let default_attributes = view.default_attributes().unwrap_or(0);
    let syntax_style = view.syntax_style();

    for line in view.visible_lines() {
        let row = y + i32::try_from(line.viewport_row).unwrap_or(i32::MAX);
        if row < 0 || row >= i32::try_from(buffer.height()).unwrap_or(i32::MAX) {
            continue;
        }
        let spans = view.line_spans(line.source_line as usize);
        if view.wrap_mode() == 0 {
            if view.truncate() {
                draw_truncated_line(
                    buffer,
                    |start, end| view.rendered_text_for_offsets(start, end),
                    &line,
                    spans,
                    syntax_style,
                    usize::try_from(x).unwrap_or(0),
                    row as usize,
                    default_fg,
                    default_bg,
                    default_attributes,
                    selection_bg,
                    selection_fg,
                    view.tab_width(),
                    view.viewport_width(),
                );
            } else {
                let clipped = clip_visible_line(&line, view.viewport_x(), view.viewport_width());
                draw_visible_line(
                    buffer,
                    |start, end| view.rendered_text_for_offsets(start, end),
                    &clipped,
                    spans,
                    syntax_style,
                    usize::try_from(x).unwrap_or(0),
                    row as usize,
                    default_fg,
                    default_bg,
                    default_attributes,
                    selection_bg,
                    selection_fg,
                    view.tab_width(),
                );
            }
        } else {
            draw_visible_line(
                buffer,
                |start, end| view.rendered_text_for_offsets(start, end),
                &line,
                spans,
                syntax_style,
                usize::try_from(x).unwrap_or(0),
                row as usize,
                default_fg,
                default_bg,
                default_attributes,
                selection_bg,
                selection_fg,
                view.tab_width(),
            );
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn bufferDrawEditorView(
    buffer: *mut NativeOptimizedBuffer,
    view: *mut NativeEditorView,
    x: i32,
    y: i32,
) {
    if buffer.is_null() || view.is_null() || x < 0 {
        return;
    }

    let buffer = unsafe { &mut *buffer };
    let view = unsafe { &*view };
    let (selection_bg, selection_fg) = view.selection_colors();
    let default_fg = view.default_fg().unwrap_or([1.0, 1.0, 1.0, 1.0]);
    let default_bg = view.default_bg().unwrap_or([0.0, 0.0, 0.0, 0.0]);
    let default_attributes = view.default_attributes().unwrap_or(0);
    let syntax_style = view.syntax_style();

    if let Some(placeholder) = view.placeholder_text() {
        for (row_index, line) in placeholder.split('\n').enumerate() {
            let row = y + i32::try_from(row_index).unwrap_or(i32::MAX);
            if row < 0 || row >= i32::try_from(buffer.height()).unwrap_or(i32::MAX) {
                continue;
            }
            let row = row as usize;
            let _ = buffer.draw_text(
                usize::try_from(x).unwrap_or(0),
                row,
                line,
                default_fg,
                default_bg,
                default_attributes,
            );
        }
        return;
    }

    for line in view.visible_lines() {
        let row = y + i32::try_from(line.viewport_row).unwrap_or(i32::MAX);
        if row < 0 || row >= i32::try_from(buffer.height()).unwrap_or(i32::MAX) {
            continue;
        }
        let spans = view.line_spans(line.source_line as usize);
        if view.wrap_mode() == 0 {
            if view.truncate() {
                draw_truncated_line(
                    buffer,
                    |start, end| view.rendered_text_for_offsets(start, end),
                    &line,
                    spans,
                    syntax_style,
                    usize::try_from(x).unwrap_or(0),
                    row as usize,
                    default_fg,
                    default_bg,
                    default_attributes,
                    selection_bg,
                    selection_fg,
                    view.tab_width(),
                    view.viewport_width(),
                );
            } else {
                let clipped = clip_visible_line(&line, view.viewport_x(), view.viewport_width());
                draw_visible_line(
                    buffer,
                    |start, end| view.rendered_text_for_offsets(start, end),
                    &clipped,
                    spans,
                    syntax_style,
                    usize::try_from(x).unwrap_or(0),
                    row as usize,
                    default_fg,
                    default_bg,
                    default_attributes,
                    selection_bg,
                    selection_fg,
                    view.tab_width(),
                );
            }
        } else {
            draw_visible_line(
                buffer,
                |start, end| view.rendered_text_for_offsets(start, end),
                &line,
                spans,
                syntax_style,
                usize::try_from(x).unwrap_or(0),
                row as usize,
                default_fg,
                default_bg,
                default_attributes,
                selection_bg,
                selection_fg,
                view.tab_width(),
            );
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn editorViewSetLocalSelection(
    view: *mut NativeEditorView,
    anchor_x: i32,
    anchor_y: i32,
    focus_x: i32,
    focus_y: i32,
    bg_color: *const f32,
    fg_color: *const f32,
    update_cursor: bool,
    _follow_cursor: bool,
) -> bool {
    if view.is_null() {
        return false;
    }

    let view = unsafe { &mut *view };
    view.set_local_selection(
        anchor_x,
        anchor_y,
        focus_x,
        focus_y,
        (!bg_color.is_null()).then(|| color_from_ptr(bg_color)),
        (!fg_color.is_null()).then(|| color_from_ptr(fg_color)),
        update_cursor,
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn editorViewUpdateLocalSelection(
    view: *mut NativeEditorView,
    anchor_x: i32,
    anchor_y: i32,
    focus_x: i32,
    focus_y: i32,
    bg_color: *const f32,
    fg_color: *const f32,
    update_cursor: bool,
    _follow_cursor: bool,
) -> bool {
    if view.is_null() {
        return false;
    }

    let view = unsafe { &mut *view };
    view.update_local_selection(
        anchor_x,
        anchor_y,
        focus_x,
        focus_y,
        (!bg_color.is_null()).then(|| color_from_ptr(bg_color)),
        (!fg_color.is_null()).then(|| color_from_ptr(fg_color)),
        update_cursor,
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn editorViewResetLocalSelection(view: *mut NativeEditorView) {
    if view.is_null() {
        return;
    }

    let view = unsafe { &mut *view };
    view.reset_local_selection();
}

#[unsafe(no_mangle)]
pub extern "C" fn editorViewGetVisualCursor(
    view: *const NativeEditorView,
    out_ptr: *mut NativeVisualCursor,
) {
    if view.is_null() || out_ptr.is_null() {
        return;
    }

    let view = unsafe { &*view };
    unsafe {
        *out_ptr = view.visual_cursor();
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn editorViewMoveUpVisual(view: *mut NativeEditorView) {
    if view.is_null() {
        return;
    }

    let view = unsafe { &mut *view };
    view.move_up_visual();
}

#[unsafe(no_mangle)]
pub extern "C" fn editorViewMoveDownVisual(view: *mut NativeEditorView) {
    if view.is_null() {
        return;
    }

    let view = unsafe { &mut *view };
    view.move_down_visual();
}

#[unsafe(no_mangle)]
pub extern "C" fn editorViewDeleteSelectedText(view: *mut NativeEditorView) {
    if view.is_null() {
        return;
    }

    let view = unsafe { &mut *view };
    view.delete_selected_text();
}

#[unsafe(no_mangle)]
pub extern "C" fn editorViewSetCursorByOffset(view: *mut NativeEditorView, offset: u32) {
    if view.is_null() {
        return;
    }

    let view = unsafe { &mut *view };
    view.set_cursor_by_offset(offset);
}

#[unsafe(no_mangle)]
pub extern "C" fn editorViewGetNextWordBoundary(
    view: *const NativeEditorView,
    out_ptr: *mut NativeVisualCursor,
) {
    if view.is_null() || out_ptr.is_null() {
        return;
    }

    let view = unsafe { &*view };
    unsafe {
        *out_ptr = view.next_word_boundary();
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn editorViewGetPrevWordBoundary(
    view: *const NativeEditorView,
    out_ptr: *mut NativeVisualCursor,
) {
    if view.is_null() || out_ptr.is_null() {
        return;
    }

    let view = unsafe { &*view };
    unsafe {
        *out_ptr = view.prev_word_boundary();
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn editorViewGetEOL(
    view: *const NativeEditorView,
    out_ptr: *mut NativeVisualCursor,
) {
    if view.is_null() || out_ptr.is_null() {
        return;
    }

    let view = unsafe { &*view };
    unsafe {
        *out_ptr = view.eol();
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn editorViewGetVisualSOL(
    view: *const NativeEditorView,
    out_ptr: *mut NativeVisualCursor,
) {
    if view.is_null() || out_ptr.is_null() {
        return;
    }

    let view = unsafe { &*view };
    unsafe {
        *out_ptr = view.visual_sol();
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn editorViewGetVisualEOL(
    view: *const NativeEditorView,
    out_ptr: *mut NativeVisualCursor,
) {
    if view.is_null() || out_ptr.is_null() {
        return;
    }

    let view = unsafe { &*view };
    unsafe {
        *out_ptr = view.visual_eol();
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn createNativeSpanFeed(
    options_ptr: *const NativeSpanFeedOptions,
) -> *mut NativeSpanFeedStream {
    let options = if options_ptr.is_null() {
        default_native_span_feed_options()
    } else {
        unsafe { *options_ptr }
    };

    match NativeSpanFeedStream::create(options) {
        Ok(stream) => Box::into_raw(Box::new(stream)),
        Err(_) => core::ptr::null_mut(),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn attachNativeSpanFeed(stream: *mut NativeSpanFeedStream) -> i32 {
    if stream.is_null() {
        return native_span_feed::status::ERR_INVALID;
    }

    let stream = unsafe { &mut *stream };
    stream
        .attach()
        .map(|()| native_span_feed::status::OK)
        .unwrap_or_else(error_to_status)
}

#[unsafe(no_mangle)]
pub extern "C" fn destroyNativeSpanFeed(stream: *mut NativeSpanFeedStream) {
    if stream.is_null() {
        return;
    }

    let mut stream = unsafe { Box::from_raw(stream) };
    let _ = stream.close();
}

#[unsafe(no_mangle)]
pub extern "C" fn streamSetCallback(
    stream: *mut NativeSpanFeedStream,
    callback: Option<NativeSpanFeedCallbackFn>,
) {
    if stream.is_null() {
        return;
    }

    let stream = unsafe { &mut *stream };
    stream.set_callback(callback);
}

#[unsafe(no_mangle)]
pub extern "C" fn streamWrite(
    stream: *mut NativeSpanFeedStream,
    src_ptr: *const u8,
    len: usize,
) -> i32 {
    if stream.is_null() || src_ptr.is_null() {
        return native_span_feed::status::ERR_INVALID;
    }

    let stream = unsafe { &mut *stream };
    let data = unsafe { std::slice::from_raw_parts(src_ptr, len) };
    stream
        .write(data)
        .map(|()| native_span_feed::status::OK)
        .unwrap_or_else(error_to_status)
}

#[unsafe(no_mangle)]
pub extern "C" fn streamCommit(stream: *mut NativeSpanFeedStream) -> i32 {
    if stream.is_null() {
        return native_span_feed::status::ERR_INVALID;
    }

    let stream = unsafe { &mut *stream };
    stream
        .commit()
        .map(|()| native_span_feed::status::OK)
        .unwrap_or_else(error_to_status)
}

#[unsafe(no_mangle)]
pub extern "C" fn streamDrainSpans(
    stream: *mut NativeSpanFeedStream,
    out_ptr: *mut NativeSpanFeedSpanInfo,
    max_spans: u32,
) -> u32 {
    if stream.is_null() || out_ptr.is_null() || max_spans == 0 {
        return 0;
    }

    let stream = unsafe { &mut *stream };
    let out = unsafe { std::slice::from_raw_parts_mut(out_ptr, max_spans as usize) };
    stream.drain_spans(out)
}

#[unsafe(no_mangle)]
pub extern "C" fn streamClose(stream: *mut NativeSpanFeedStream) -> i32 {
    if stream.is_null() {
        return native_span_feed::status::ERR_INVALID;
    }

    let stream = unsafe { &mut *stream };
    stream
        .close()
        .map(|()| native_span_feed::status::OK)
        .unwrap_or_else(error_to_status)
}

#[unsafe(no_mangle)]
pub extern "C" fn streamReserve(
    stream: *mut NativeSpanFeedStream,
    min_len: u32,
    out_ptr: *mut NativeSpanFeedReserveInfo,
) -> i32 {
    if stream.is_null() || out_ptr.is_null() {
        return native_span_feed::status::ERR_INVALID;
    }

    let stream = unsafe { &mut *stream };
    match stream.reserve(min_len) {
        Ok(info) => {
            unsafe {
                *out_ptr = info;
            }
            native_span_feed::status::OK
        }
        Err(error) => error_to_status(error),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn streamCommitReserved(stream: *mut NativeSpanFeedStream, len: u32) -> i32 {
    if stream.is_null() {
        return native_span_feed::status::ERR_INVALID;
    }

    let stream = unsafe { &mut *stream };
    stream
        .commit_reserved(len)
        .map(|()| native_span_feed::status::OK)
        .unwrap_or_else(error_to_status)
}

#[unsafe(no_mangle)]
pub extern "C" fn streamSetOptions(
    stream: *mut NativeSpanFeedStream,
    options_ptr: *const NativeSpanFeedOptions,
) -> i32 {
    if stream.is_null() || options_ptr.is_null() {
        return native_span_feed::status::ERR_INVALID;
    }

    let stream = unsafe { &mut *stream };
    let options = unsafe { *options_ptr };
    stream
        .set_options(options)
        .map(|()| native_span_feed::status::OK)
        .unwrap_or_else(error_to_status)
}

#[unsafe(no_mangle)]
pub extern "C" fn streamGetStats(
    stream: *mut NativeSpanFeedStream,
    stats_ptr: *mut NativeSpanFeedStats,
) -> i32 {
    if stream.is_null() || stats_ptr.is_null() {
        return native_span_feed::status::ERR_INVALID;
    }

    let stream = unsafe { &mut *stream };
    unsafe {
        *stats_ptr = stream.get_stats();
    }
    native_span_feed::status::OK
}

#[cfg(test)]
mod tests {
    use std::ffi::CStr;

    use super::{
        ABI_SYMBOL_COUNT, OpentuiRustFoundationAbiInfo, attachNativeSpanFeed, createNativeSpanFeed,
        createSyntaxStyle, destroyNativeSpanFeed, destroySyntaxStyle,
        opentui_rust_foundation_abi_hash, opentui_rust_foundation_abi_info, streamClose,
        streamCommit, streamDrainSpans, streamGetStats, streamWrite, syntaxStyleGetStyleCount,
        syntaxStyleRegister, syntaxStyleResolveByName,
    };
    use crate::native_span_feed::{
        SpanInfo, Stats, default_options as default_native_span_feed_options,
        status as native_span_feed_status,
    };

    #[test]
    fn abi_info_rejects_null_output() {
        assert!(!opentui_rust_foundation_abi_info(core::ptr::null_mut()));
    }

    #[test]
    fn abi_info_embeds_manifest_metadata() {
        let mut info = OpentuiRustFoundationAbiInfo {
            abi_symbol_count: 0,
            abi_manifest_hash: core::ptr::null(),
            crate_version: core::ptr::null(),
            build_profile: core::ptr::null(),
        };

        assert!(opentui_rust_foundation_abi_info(&mut info));
        assert_eq!(info.abi_symbol_count, ABI_SYMBOL_COUNT);

        let abi_hash = unsafe { CStr::from_ptr(info.abi_manifest_hash) }
            .to_str()
            .unwrap();
        let direct_hash = unsafe { CStr::from_ptr(opentui_rust_foundation_abi_hash()) }
            .to_str()
            .unwrap();
        let crate_version = unsafe { CStr::from_ptr(info.crate_version) }
            .to_str()
            .unwrap();
        let build_profile = unsafe { CStr::from_ptr(info.build_profile) }
            .to_str()
            .unwrap();

        assert_eq!(abi_hash, direct_hash);
        assert_eq!(abi_hash.len(), 64);
        assert_eq!(crate_version, env!("CARGO_PKG_VERSION"));
        assert_eq!(build_profile, env!("OPENTUI_BUILD_PROFILE"));
    }

    #[test]
    fn syntax_style_ffi_round_trip_works() {
        let style = createSyntaxStyle();
        assert_ne!(style, core::ptr::null_mut());
        assert_eq!(syntaxStyleGetStyleCount(style), 0);

        let name = b"keyword";
        let fg = [1.0_f32, 0.0, 0.0, 1.0];
        let first = syntaxStyleRegister(
            style,
            name.as_ptr(),
            name.len(),
            fg.as_ptr(),
            core::ptr::null(),
            1,
        );
        let second = syntaxStyleRegister(
            style,
            name.as_ptr(),
            name.len(),
            core::ptr::null(),
            core::ptr::null(),
            2,
        );

        assert_eq!(first, second);
        assert_eq!(
            syntaxStyleResolveByName(style, name.as_ptr(), name.len()),
            first
        );
        assert_eq!(syntaxStyleGetStyleCount(style), 1);

        destroySyntaxStyle(style);
    }

    #[test]
    fn syntax_style_ffi_is_defensive_about_nulls() {
        let name = b"missing";

        assert_eq!(
            syntaxStyleRegister(
                core::ptr::null_mut(),
                name.as_ptr(),
                name.len(),
                core::ptr::null(),
                core::ptr::null(),
                0
            ),
            0
        );
        assert_eq!(
            syntaxStyleResolveByName(core::ptr::null(), name.as_ptr(), name.len()),
            0
        );
        assert_eq!(syntaxStyleGetStyleCount(core::ptr::null()), 0);
        destroySyntaxStyle(core::ptr::null_mut());
    }

    #[test]
    fn native_span_feed_ffi_round_trip_works() {
        let stream = createNativeSpanFeed(&default_native_span_feed_options());
        assert_ne!(stream, core::ptr::null_mut());
        assert_eq!(attachNativeSpanFeed(stream), native_span_feed_status::OK);
        assert_eq!(
            streamWrite(stream, b"hello".as_ptr(), 5),
            native_span_feed_status::OK
        );
        assert_eq!(streamCommit(stream), native_span_feed_status::OK);

        let mut spans = [SpanInfo::default(); 4];
        assert_eq!(streamDrainSpans(stream, spans.as_mut_ptr(), 4), 1);
        assert_eq!(spans[0].len, 5);

        let mut stats = Stats::default();
        assert_eq!(
            streamGetStats(stream, &mut stats),
            native_span_feed_status::OK
        );
        assert_eq!(stats.spans_committed, 1);

        assert_eq!(streamClose(stream), native_span_feed_status::OK);
        destroyNativeSpanFeed(stream);
    }
}
