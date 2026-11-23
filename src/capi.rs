#![allow(clippy::missing_safety_doc)]

use crate::format::MAX_N;
use crate::index::CapCover;
use crate::vector::Vectors;
use std::cell::RefCell;
use std::ffi::{c_char, CStr, CString};

thread_local! {
    static LAST_ERROR: RefCell<CString> = RefCell::new(CString::new("").unwrap());
}

fn set_error(message: impl Into<String>) {
    let message = message.into();
    LAST_ERROR.with(|slot| {
        *slot.borrow_mut() = CString::new(message).unwrap_or_default();
    });
}

#[no_mangle]
pub extern "C" fn capcover_version() -> *const c_char {
    c"0.1.0".as_ptr()
}

#[no_mangle]
pub extern "C" fn capcover_last_error() -> *const c_char {
    LAST_ERROR.with(|slot| slot.borrow().as_ptr())
}

#[no_mangle]
pub unsafe extern "C" fn capcover_open(path: *const c_char) -> *mut CapCover {
    if path.is_null() {
        set_error("null path");
        return std::ptr::null_mut();
    }
    let path = CStr::from_ptr(path);
    let path = match path.to_str() {
        Ok(value) => value,
        Err(_) => {
            set_error("path is not valid UTF-8");
            return std::ptr::null_mut();
        }
    };
    match CapCover::open(std::path::Path::new(path)) {
        Ok(index) => Box::into_raw(Box::new(index)),
        Err(error) => {
            set_error(error.to_string());
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn capcover_close(handle: *mut CapCover) {
    if !handle.is_null() {
        drop(Box::from_raw(handle));
    }
}

#[no_mangle]
pub unsafe extern "C" fn capcover_dim(handle: *const CapCover) -> usize {
    if handle.is_null() {
        return 0;
    }
    (*handle).dim()
}

#[no_mangle]
pub unsafe extern "C" fn capcover_n(handle: *const CapCover) -> usize {
    if handle.is_null() {
        return 0;
    }
    (*handle).n()
}

#[no_mangle]
pub unsafe extern "C" fn capcover_k(handle: *const CapCover) -> usize {
    if handle.is_null() {
        return 0;
    }
    (*handle).k()
}

#[no_mangle]
pub unsafe extern "C" fn capcover_file_bytes(handle: *const CapCover) -> u64 {
    if handle.is_null() {
        return 0;
    }
    (*handle).file_bytes()
}

#[no_mangle]
pub unsafe extern "C" fn capcover_may_contain(
    handle: *const CapCover,
    query: *const f32,
    len: usize,
    similarity: f32,
) -> i32 {
    if handle.is_null() {
        set_error("null index");
        return -1;
    }
    if query.is_null() {
        set_error("null query");
        return -1;
    }
    let index = &*handle;
    if len != index.dim() {
        set_error(format!(
            "query dimension {len} does not match index dimension {}",
            index.dim()
        ));
        return -1;
    }
    let query = std::slice::from_raw_parts(query, len);
    match index.may_contain(query, similarity) {
        Ok(true) => 1,
        Ok(false) => 0,
        Err(error) => {
            set_error(error.to_string());
            -1
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn capcover_check_bitmap(
    handle: *const CapCover,
    queries: *const f32,
    count: usize,
    similarity: f32,
    out: *mut u8,
) -> i32 {
    if handle.is_null() {
        set_error("null index");
        return -1;
    }
    if queries.is_null() || out.is_null() {
        set_error("null query or output buffer");
        return -1;
    }
    if count > MAX_N as usize {
        set_error(format!("query count {count} exceeds {MAX_N}"));
        return -1;
    }
    let index = &*handle;
    let total = match count.checked_mul(index.dim()) {
        Some(value) => value,
        None => {
            set_error("query buffer size overflows");
            return -1;
        }
    };
    let data = std::slice::from_raw_parts(queries, total);
    let vectors = match Vectors::from_data(index.dim(), data.to_vec()) {
        Ok(value) => value,
        Err(error) => {
            set_error(error.to_string());
            return -1;
        }
    };
    let mut decisions = vec![false; count];
    match index.may_contain_batch(&vectors, similarity, &mut decisions) {
        Ok(()) => {}
        Err(error) => {
            set_error(error.to_string());
            return -1;
        }
    }
    for (offset, decision) in decisions.iter().enumerate() {
        *out.add(offset) = u8::from(*decision);
    }
    0
}
