// pathfinder/font-renderer/src/directwrite.rs
//
// Copyright © 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use dwrite;
use libc::c_void;
use std::ptr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use uuid;
use winapi::winerror::{self, E_NOINTERFACE, E_POINTER, S_OK};
use winapi::{self, GUID, HRESULT, IDWriteFactory, IDWriteFontFileEnumerator, IUnknown};
use winapi::{REFIID, UINT32};

use FontKey;

DEFINE_GUID! {
    IID_DWriteFactory, 0xb859ee5a, 0xd838, 0x4b5b, 0xa2, 0xe8, 0x1a, 0xdc, 0x7d, 0x93, 0xdb, 0x48
}
DEFINE_GUID! {
    IID_DWriteFontCollectionLoader,
    0xcca920e4, 0x52f0, 0x492b, 0xbf, 0xa8, 0x29, 0xc7, 0x2e, 0xe0, 0xa4, 0x68
}

pub struct FontContext {
    dwrite_factory: *mut IDWriteFactory,
}

impl FontContext {
    pub fn new() -> Result<FontContext, ()> {
        unsafe {
            let mut factory: *mut IDWriteFactory = ptr::null_mut();
            if !winerror::SUCCEEDED(dwrite::DWriteCreateFactory(winapi::DWRITE_FACTORY_TYPE_SHARED,
                                                                &IID_DWriteFactory,
                                                                &mut factory as *mut *mut _ as
                                                                *mut *mut IUnknown)) {
                return Err(())
            }
            Ok(FontContext {
                dwrite_factory: factory,
            })
        }
    }

    pub fn add_font_from_memory(&mut self, font_key: &FontKey, bytes: Arc<Vec<u8>>, _: u32)
                                -> Result<(), ()> {
        // TODO(pcwalton)
        Err(())
    }
}

struct PathfinderFontCollectionLoader {
    ref_count: AtomicUsize,
}

impl PathfinderFontCollectionLoader {
    unsafe extern "C" fn AddRef(this: *mut PathfinderFontCollectionLoader) {
        (*this).ref_count.fetch_add(1, Ordering::SeqCst);
    }

    unsafe extern "C" fn Release(this: *mut PathfinderFontCollectionLoader) {
        if (*this).ref_count.fetch_sub(1, Ordering::SeqCst) == 1 {
            drop(Box::from_raw(this))
        }
    }

    unsafe extern "C" fn QueryInterface(this: *mut PathfinderFontCollectionLoader,
                                        riid: REFIID,
                                        object: *mut *mut c_void)
                                        -> HRESULT {
        if object.is_null() {
            return E_POINTER
        }
        if guids_are_equal(&*riid, &uuid::IID_IUnknown) ||
                guids_are_equal(&*riid, &dwrite::IID_IDWriteFontCollectionLoader) {
            *object = this as *mut c_void;
            return S_OK
        }
        *object = ptr::null_mut();
        E_NOINTERFACE
    }

    unsafe extern "C" fn CreateEnumeratorFromKey(
            this: *mut PathfinderFontCollectionLoader,
            factory: *mut IDWriteFactory,
            collection_key: *const c_void,
            collection_key_size: UINT32,
            font_file_enumerator: *mut *mut IDWriteFontFileEnumerator)
            -> HRESULT {
        let new_font_file_enumerator = Box::new(PathfinderFontFileEnumerator::new());
        *font_file_enumerator = new_font_file_enumerator.into_raw();
        winapi::S_OK
    }
}

fn guids_are_equal(a: &GUID, b: &GUID) -> bool {
    a.Data1 == b.Data1 && a.Data2 == b.Data2 && a.Data3 == b.Data3 && a.Data4 == b.Data4
}
