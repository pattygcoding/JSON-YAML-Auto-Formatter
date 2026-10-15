//! WebAssembly bridge.
//!
//! `wasm-bindgen` is deliberately not used: the module exports three plain
//! functions, so building is just `cargo build --target wasm32-unknown-unknown`
//! with no extra tooling and no version pairing between a crate and a CLI.
//! `formatter.js` is the JavaScript half of the bridge.
//!
//! The host writes the two strings into memory it obtains from [`alloc`], calls
//! [`format_input`], and reads the answer out of the buffer that call returns. The
//! answer buffer stays owned by the module and is reused by the next call, so the
//! host must copy what it needs before formatting again.

use core::cell::RefCell;
use core::slice;
use core::str;

use crate::format;

thread_local! {
    /// The most recent result: four little-endian length bytes followed by UTF-8.
    static RESULT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

/// Reserves `length` bytes for the host to write a string into.
#[no_mangle]
pub extern "C" fn alloc(length: usize) -> *mut u8 {
    if length == 0 {
        return core::ptr::null_mut();
    }
    let mut buffer = Vec::<u8>::with_capacity(length);
    let pointer = buffer.as_mut_ptr();
    core::mem::forget(buffer);
    pointer
}

/// Releases a buffer handed out by [`alloc`].
///
/// # Safety
///
/// `pointer` must come from [`alloc`] with the same `length`, and must not be used
/// afterwards.
#[no_mangle]
pub unsafe extern "C" fn dealloc(pointer: *mut u8, length: usize) {
    if pointer.is_null() || length == 0 {
        return;
    }
    drop(Vec::from_raw_parts(pointer, 0, length));
}

/// Formats `input` with `mode` and returns a pointer to the result buffer.
///
/// The result is four little-endian length bytes followed by the formatted text,
/// or by an error message when the input was rejected. The pointer stays valid
/// until the next call.
///
/// # Safety
///
/// Both pointers must address buffers of their stated length that were created by
/// [`alloc`] and filled with UTF-8, and they must not be freed before this returns.
#[no_mangle]
pub unsafe extern "C" fn format_input(
    input: *const u8,
    input_length: usize,
    mode: *const u8,
    mode_length: usize,
) -> *const u8 {
    let input = utf8_slice(input, input_length);
    let mode = utf8_slice(mode, mode_length);
    let text = match format::format(input, mode) {
        Ok(formatted) => formatted.text,
        Err(error) => error,
    };
    RESULT.with(|slot| {
        let mut slot = slot.borrow_mut();
        slot.clear();
        slot.extend_from_slice(&(text.len() as u32).to_le_bytes());
        slot.extend_from_slice(text.as_bytes());
        slot.as_ptr()
    })
}

/// # Safety
///
/// `pointer` must address `length` readable bytes that are not freed during the
/// returned borrow.
unsafe fn utf8_slice<'a>(pointer: *const u8, length: usize) -> &'a str {
    if pointer.is_null() || length == 0 {
        return "";
    }
    let bytes = slice::from_raw_parts(pointer, length);
    str::from_utf8(bytes).unwrap_or("")
}
