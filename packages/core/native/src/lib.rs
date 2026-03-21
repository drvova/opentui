#![deny(unsafe_op_in_unsafe_fn)]

use core::ffi::{c_char, c_uint};

mod syntax_style;

use syntax_style::{Rgba, SyntaxStyleState};

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

#[cfg(test)]
mod tests {
    use std::ffi::CStr;

    use super::{
        ABI_SYMBOL_COUNT, OpentuiRustFoundationAbiInfo, createSyntaxStyle, destroySyntaxStyle,
        opentui_rust_foundation_abi_hash, opentui_rust_foundation_abi_info,
        syntaxStyleGetStyleCount, syntaxStyleRegister, syntaxStyleResolveByName,
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
}
