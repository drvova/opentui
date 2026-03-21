#![deny(unsafe_op_in_unsafe_fn)]

use core::ffi::{c_char, c_uint};

mod edit_buffer;
mod editor_view;
mod native_span_feed;
mod syntax_style;
mod text_buffer;
mod text_buffer_view;

pub type NativeEditBuffer = edit_buffer::EditBufferState;
pub type NativeEditorView = editor_view::EditorViewState;
pub type NativeLogicalCursor = edit_buffer::LogicalCursor;
pub type NativeSpanFeedCallbackFn = native_span_feed::CallbackFn;
pub type NativeSpanFeedOptions = native_span_feed::Options;
pub type NativeSpanFeedReserveInfo = native_span_feed::ReserveInfo;
pub type NativeSpanFeedSpanInfo = native_span_feed::SpanInfo;
pub type NativeSpanFeedStats = native_span_feed::Stats;
pub type NativeSpanFeedStream = native_span_feed::Stream;
pub type NativeStyledChunk = text_buffer::StyledChunk;
pub type NativeTextBuffer = text_buffer::TextBufferState;
pub type NativeTextBufferView = text_buffer_view::TextBufferViewState;

use edit_buffer::EditBufferState;
use editor_view::EditorViewState;
use native_span_feed::{default_options as default_native_span_feed_options, error_to_status};
use syntax_style::{Rgba, SyntaxStyleState};
use text_buffer::{TextBufferState, copy_bytes_to_out};
use text_buffer_view::{NO_SELECTION, TextBufferViewState, copy_selected_text};

const ABI_SYMBOL_COUNT: c_uint = parse_symbol_count();
const ABI_HASH_CSTR: &[u8] = concat!(env!("OPENTUI_ABI_SYMBOL_HASH"), "\0").as_bytes();
const BUILD_PROFILE_CSTR: &[u8] = concat!(env!("OPENTUI_BUILD_PROFILE"), "\0").as_bytes();
const CRATE_VERSION_CSTR: &[u8] = concat!(env!("CARGO_PKG_VERSION"), "\0").as_bytes();

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
    let color = unsafe { std::slice::from_raw_parts(ptr, 4) };
    [color[0], color[1], color[2], color[3]]
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
    _bg_color: *const f32,
    _fg_color: *const f32,
) {
    if view.is_null() {
        return;
    }

    let view = unsafe { &mut *view };
    view.set_selection(start, end);
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
    _bg_color: *const f32,
    _fg_color: *const f32,
) {
    if view.is_null() {
        return;
    }

    let view = unsafe { &mut *view };
    view.update_selection(end);
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
    _move_cursor: bool,
) {
    if view.is_null() {
        return;
    }

    let view = unsafe { &mut *view };
    view.set_viewport(x, y, width, height);
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
    _bg_color: *const f32,
    _fg_color: *const f32,
) {
    if view.is_null() {
        return;
    }

    let view = unsafe { &mut *view };
    view.set_selection(start, end);
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
    _bg_color: *const f32,
    _fg_color: *const f32,
) {
    if view.is_null() {
        return;
    }

    let view = unsafe { &mut *view };
    view.update_selection(end);
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
