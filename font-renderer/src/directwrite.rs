// pathfinder/font-renderer/src/directwrite.rs
//
// Copyright © 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::collections::BTreeMap;
use winapi::IDWriteFontFace;
use std::ops::Deref;
use winapi::FALSE;
use winapi::TRUE;
use winapi::BOOL;
use winapi::IDWriteFontFile;
use winapi::E_INVALIDARG;
use winapi::E_BOUNDS;
use winapi::FILETIME;
use winapi::UINT64;
use winapi::ULONG;
use winapi::IUnknownVtbl;
use winapi::IDWriteFontFileStreamVtbl;
use winapi::IDWriteFontFileLoaderVtbl;
use winapi::IDWriteFontFileEnumeratorVtbl;
use dwrite;
use kernel32;
use std::mem;
use std::os::raw::c_void;
use std::ptr;
use std::slice;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use uuid;
use winapi::winerror::{self, E_NOINTERFACE, E_POINTER, S_OK};
use winapi::{self, GUID, HRESULT, IDWriteFactory, IDWriteFontCollectionLoader};
use winapi::{IDWriteFontCollectionLoaderVtbl, IDWriteFontFileEnumerator, IDWriteFontFileLoader};
use winapi::{IDWriteFontFileStream};
use winapi::{IUnknown, REFIID, UINT32};

use FontKey;

DEFINE_GUID! {
    IID_IDWriteFactory, 0xb859ee5a, 0xd838, 0x4b5b, 0xa2, 0xe8, 0x1a, 0xdc, 0x7d, 0x93, 0xdb, 0x48
}
DEFINE_GUID! {
    IID_IDWriteFontCollectionLoader,
    0xcca920e4, 0x52f0, 0x492b, 0xbf, 0xa8, 0x29, 0xc7, 0x2e, 0xe0, 0xa4, 0x68
}
DEFINE_GUID! {
    IID_IDWriteFontFileEnumerator,
    0x72755049, 0x5ff7, 0x435d, 0x83, 0x48, 0x4b, 0xe9, 0x7c, 0xfa, 0x6c, 0x7c
}
DEFINE_GUID! {
    IID_IDWriteFontFile, 0x739d886a, 0xcef5, 0x47dc, 0x87, 0x69, 0x1a, 0x8b, 0x41, 0xbe, 0xbb, 0xb0
}
DEFINE_GUID! {
    IID_IDWriteFontFileLoader,
    0x727cad4e, 0xd6af, 0x4c9e, 0x8a, 0x08, 0xd6, 0x95, 0xb1, 0x1c, 0xaa, 0x49
}
DEFINE_GUID! {
    IID_IDWriteFontFileStream,
    0x6d4865fe, 0x0ab8, 0x4d91, 0x8f, 0x62, 0x5d, 0xd6, 0xbe, 0x34, 0xa3, 0xe0
}

static PATHFINDER_FONT_FILE_KEY: [u8; 6] = *b"MEMORY";

pub struct FontContext {
    dwrite_factory: PathfinderComPtr<IDWriteFactory>,
    dwrite_font_faces: BTreeMap<FontKey, PathfinderComPtr<IDWriteFontFace>>,
}

impl FontContext {
    pub fn new() -> Result<FontContext, ()> {
        unsafe {
            let mut factory: *mut IDWriteFactory = ptr::null_mut();
            if !winerror::SUCCEEDED(dwrite::DWriteCreateFactory(winapi::DWRITE_FACTORY_TYPE_SHARED,
                                                                &IID_IDWriteFactory,
                                                                &mut factory as *mut *mut _ as
                                                                *mut *mut IUnknown)) {
                return Err(())
            }
            Ok(FontContext {
                dwrite_factory: PathfinderComPtr::new(factory),
                dwrite_font_faces: BTreeMap::new(),
            })
        }
    }

    pub fn add_font_from_memory(&mut self, font_key: &FontKey, bytes: Arc<Vec<u8>>, _: u32)
                                -> Result<(), ()> {
        unsafe {
            let font_collection_loader = PathfinderFontCollectionLoader::new(bytes);

            let mut font_collection = ptr::null_mut();
            let result = (**self.dwrite_factory).CreateCustomFontCollection(
                font_collection_loader.clone().into_raw() as *mut IDWriteFontCollectionLoader,
                PATHFINDER_FONT_FILE_KEY.as_ptr() as *const c_void,
                PATHFINDER_FONT_FILE_KEY.len() as UINT32,
                &mut font_collection);
            if !winerror::SUCCEEDED(result) {
                return Err(())
            }
            let font_collection = PathfinderComPtr::new(font_collection);

            let mut font_family = ptr::null_mut();
            let result = (**font_collection).GetFontFamily(0, &mut font_family);
            if !winerror::SUCCEEDED(result) {
                return Err(())
            }
            let font_family = PathfinderComPtr::new(font_family);

            let mut font = ptr::null_mut();
            let result = (**font_family).GetFont(0, &mut font);
            if !winerror::SUCCEEDED(result) {
                return Err(())
            }
            let font = PathfinderComPtr::new(font);

            let mut font_face = ptr::null_mut();
            let result = (**font).CreateFontFace(&mut font_face);
            if !winerror::SUCCEEDED(result) {
                return Err(())
            }
            let font_face = PathfinderComPtr::new(font_face);

            self.dwrite_font_faces.insert(*font_key, font_face);
            Ok(())
        }
    }
}

struct PathfinderFontCollectionLoader {
    object: PathfinderComObject<PathfinderFontCollectionLoader>,
    buffer: Arc<Vec<u8>>,
}

static PATHFINDER_FONT_COLLECTION_LOADER_VTABLE:
       IDWriteFontCollectionLoaderVtbl = IDWriteFontCollectionLoaderVtbl {
    parent: IUnknownVtbl {
        AddRef: PathfinderComObject::<PathfinderFontCollectionLoader>::AddRef,
        Release: PathfinderComObject::<PathfinderFontCollectionLoader>::Release,
        QueryInterface: PathfinderComObject::<PathfinderFontCollectionLoader>::QueryInterface,
    },
    CreateEnumeratorFromKey: PathfinderFontCollectionLoader::CreateEnumeratorFromKey,
};

impl PathfinderCoclass for PathfinderFontCollectionLoader {
    type InterfaceVtable = IDWriteFontCollectionLoaderVtbl;
    fn interface_guid() -> &'static GUID { &IID_IDWriteFontCollectionLoader }
    fn vtable() -> &'static IDWriteFontCollectionLoaderVtbl {
        &PATHFINDER_FONT_COLLECTION_LOADER_VTABLE
    }
}

impl PathfinderFontCollectionLoader {
    #[inline]
    fn new(buffer: Arc<Vec<u8>>) -> PathfinderComPtr<PathfinderFontCollectionLoader> {
        unsafe {
            PathfinderComPtr::new(Box::into_raw(Box::new(PathfinderFontCollectionLoader {
                object: PathfinderComObject::construct(),
                buffer: buffer,
            })))
        }
    }

    unsafe extern "system" fn CreateEnumeratorFromKey(
            this: *mut IDWriteFontCollectionLoader,
            factory: *mut IDWriteFactory,
            collection_key: *const c_void,
            collection_key_size: UINT32,
            font_file_enumerator: *mut *mut IDWriteFontFileEnumerator)
            -> HRESULT {
        let this = this as *mut PathfinderFontCollectionLoader;
        let font_file_loader = PathfinderFontFileLoader::new((*this).buffer.clone());

        let mut font_file = ptr::null_mut();
        let result = (*factory).CreateCustomFontFileReference(
            PATHFINDER_FONT_FILE_KEY.as_ptr() as *const c_void,
            PATHFINDER_FONT_FILE_KEY.len() as UINT32,
            font_file_loader.into_raw() as *mut IDWriteFontFileLoader,
            &mut font_file);
        if !winerror::SUCCEEDED(result) {
            return result
        }

        let font_file = PathfinderComPtr::new(font_file);
        let new_font_file_enumerator = PathfinderFontFileEnumerator::new(font_file);
        *font_file_enumerator = new_font_file_enumerator.into_raw() as
            *mut IDWriteFontFileEnumerator;
        S_OK
    }
}

struct PathfinderFontFileEnumerator {
    object: PathfinderComObject<PathfinderFontFileEnumerator>,
    file: PathfinderComPtr<IDWriteFontFile>,
    state: PathfinderFontFileEnumeratorState,
}

static PATHFINDER_FONT_FILE_ENUMERATOR_VTABLE:
       IDWriteFontFileEnumeratorVtbl = IDWriteFontFileEnumeratorVtbl {
    parent: IUnknownVtbl {
        AddRef: PathfinderComObject::<PathfinderFontFileEnumerator>::AddRef,
        Release: PathfinderComObject::<PathfinderFontFileEnumerator>::Release,
        QueryInterface: PathfinderComObject::<PathfinderFontFileEnumerator>::QueryInterface,
    },
    GetCurrentFontFile: PathfinderFontFileEnumerator::GetCurrentFontFile,
    MoveNext: PathfinderFontFileEnumerator::MoveNext,
};

#[derive(Clone, Copy, PartialEq, Debug)]
enum PathfinderFontFileEnumeratorState {
    Start,
    AtBuffer,
    End,
}

impl PathfinderCoclass for PathfinderFontFileEnumerator {
    type InterfaceVtable = IDWriteFontFileEnumeratorVtbl;
    fn interface_guid() -> &'static GUID { &IID_IDWriteFontFileEnumerator }
    fn vtable() -> &'static IDWriteFontFileEnumeratorVtbl {
        &PATHFINDER_FONT_FILE_ENUMERATOR_VTABLE
    }
}

impl PathfinderFontFileEnumerator {
    #[inline]
    fn new(file: PathfinderComPtr<IDWriteFontFile>)
           -> PathfinderComPtr<PathfinderFontFileEnumerator> {
        unsafe {
            PathfinderComPtr::new(Box::into_raw(Box::new(PathfinderFontFileEnumerator {
                object: PathfinderComObject::construct(),
                file: file,
                state: PathfinderFontFileEnumeratorState::Start,
            })))
        }
    }

    unsafe extern "system" fn GetCurrentFontFile(this: *mut IDWriteFontFileEnumerator,
                                                 font_file: *mut *mut IDWriteFontFile)
                                                 -> HRESULT {
        let this = this as *mut PathfinderFontFileEnumerator;
        if (*this).state != PathfinderFontFileEnumeratorState::AtBuffer {
            *font_file = ptr::null_mut();
            return E_BOUNDS
        }
        *font_file = (*this).file.clone().into_raw();
        S_OK
    }

    unsafe extern "system" fn MoveNext(this: *mut IDWriteFontFileEnumerator,
                                       has_current_file: *mut BOOL)
                                       -> HRESULT {
        let this = this as *mut PathfinderFontFileEnumerator;
        match (*this).state {
            PathfinderFontFileEnumeratorState::Start => {
                (*this).state = PathfinderFontFileEnumeratorState::AtBuffer;
                *has_current_file = TRUE;
            }
            PathfinderFontFileEnumeratorState::AtBuffer => {
                (*this).state = PathfinderFontFileEnumeratorState::End;
                *has_current_file = FALSE;
            }
            PathfinderFontFileEnumeratorState::End => *has_current_file = FALSE,
        }
        S_OK
    }
}

struct PathfinderFontFileLoader {
    object: PathfinderComObject<PathfinderFontFileLoader>,
    buffer: Arc<Vec<u8>>,
}

static PATHFINDER_FONT_FILE_LOADER_VTABLE: IDWriteFontFileLoaderVtbl = IDWriteFontFileLoaderVtbl {
    parent: IUnknownVtbl {
        AddRef: PathfinderComObject::<PathfinderFontFileLoader>::AddRef,
        Release: PathfinderComObject::<PathfinderFontFileLoader>::Release,
        QueryInterface: PathfinderComObject::<PathfinderFontFileLoader>::QueryInterface,
    },
    CreateStreamFromKey: PathfinderFontFileLoader::CreateStreamFromKey,
};

impl PathfinderCoclass for PathfinderFontFileLoader {
    type InterfaceVtable = IDWriteFontFileLoaderVtbl;
    fn interface_guid() -> &'static GUID { &IID_IDWriteFontFileLoader }
    fn vtable() -> &'static IDWriteFontFileLoaderVtbl { &PATHFINDER_FONT_FILE_LOADER_VTABLE }
}

impl PathfinderFontFileLoader {
    #[inline]
    fn new(buffer: Arc<Vec<u8>>) -> PathfinderComPtr<PathfinderFontFileLoader> {
        unsafe {
            PathfinderComPtr::new(Box::into_raw(Box::new(PathfinderFontFileLoader {
                object: PathfinderComObject::construct(),
                buffer: buffer,
            })))
        }
    }

    unsafe extern "system" fn CreateStreamFromKey(
            this: *mut IDWriteFontFileLoader,
            font_file_reference_key: *const c_void,
            font_file_reference_key_size: UINT32,
            font_file_stream: *mut *mut IDWriteFontFileStream)
            -> HRESULT {
        let this = this as *mut PathfinderFontFileLoader;
        let font_file_reference = slice::from_raw_parts(font_file_reference_key as *const u8,
                                                        font_file_reference_key_size as usize);
        if font_file_reference != PATHFINDER_FONT_FILE_KEY {
            *font_file_stream = ptr::null_mut();
            return E_INVALIDARG
        }

        *font_file_stream = PathfinderFontFileStream::new((*this).buffer.clone()).into_raw() as
            *mut IDWriteFontFileStream;
        S_OK
    }
}

struct PathfinderFontFileStream {
    object: PathfinderComObject<PathfinderFontFileStream>,
    buffer: Arc<Vec<u8>>,
    creation_time: UINT64,
}

static PATHFINDER_FONT_FILE_STREAM_VTABLE: IDWriteFontFileStreamVtbl = IDWriteFontFileStreamVtbl {
    parent: IUnknownVtbl {
        AddRef: PathfinderComObject::<PathfinderFontFileStream>::AddRef,
        Release: PathfinderComObject::<PathfinderFontFileStream>::Release,
        QueryInterface: PathfinderComObject::<PathfinderFontFileStream>::QueryInterface,
    },
    GetFileSize: PathfinderFontFileStream::GetFileSize,
    GetLastWriteTime: PathfinderFontFileStream::GetLastWriteTime,
    ReadFileFragment: PathfinderFontFileStream::ReadFileFragment,
    ReleaseFileFragment: PathfinderFontFileStream::ReleaseFileFragment,
};

impl PathfinderCoclass for PathfinderFontFileStream {
    type InterfaceVtable = IDWriteFontFileStreamVtbl;
    fn interface_guid() -> &'static GUID { &IID_IDWriteFontFileStream }
    fn vtable() -> &'static IDWriteFontFileStreamVtbl { &PATHFINDER_FONT_FILE_STREAM_VTABLE }
}

impl PathfinderFontFileStream {
    #[inline]
    fn new(buffer: Arc<Vec<u8>>) -> PathfinderComPtr<PathfinderFontFileStream> {
        unsafe {
            let mut now = FILETIME {
                dwLowDateTime: 0,
                dwHighDateTime: 0,
            };
            kernel32::GetSystemTimeAsFileTime(&mut now);

            PathfinderComPtr::new(Box::into_raw(Box::new(PathfinderFontFileStream {
                object: PathfinderComObject::construct(),
                buffer: buffer,
                creation_time: ((now.dwHighDateTime as UINT64) << 32) |
                    (now.dwLowDateTime as UINT64),
            })))
        }
    }

    unsafe extern "system" fn GetFileSize(this: *mut IDWriteFontFileStream, file_size: *mut UINT64)
                                          -> HRESULT {
        let this = this as *mut PathfinderFontFileStream;
        *file_size = (*this).buffer.len() as UINT64;
        S_OK
    }

    unsafe extern "system" fn GetLastWriteTime(this: *mut IDWriteFontFileStream,
                                               last_write_time: *mut UINT64)
                                               -> HRESULT {
        let this = this as *mut PathfinderFontFileStream;
        *last_write_time = (*this).creation_time;
        S_OK
    }

    unsafe extern "system" fn ReadFileFragment(this: *mut IDWriteFontFileStream,
                                               fragment_start: *mut *const c_void,
                                               file_offset: UINT64,
                                               fragment_size: UINT64,
                                               fragment_context: *mut *mut c_void)
                                               -> HRESULT {
        let this = this as *mut PathfinderFontFileStream;
        let buffer_length = (*this).buffer.len() as u64;
        if file_offset > buffer_length || file_offset + fragment_size > buffer_length {
            return E_BOUNDS
        }

        let ptr = (*(*this).buffer).as_ptr().offset(file_offset as isize) as *const c_void;
        *fragment_start = ptr;
        *fragment_context = ptr as *mut c_void;
        (*(this as *mut IUnknown)).AddRef();
        S_OK
    }

    unsafe extern "system" fn ReleaseFileFragment(this: *mut IDWriteFontFileStream,
                                                  fragment_context: *mut c_void) {
        let this = this as *mut PathfinderFontFileStream;
        (*(this as *mut IUnknown)).Release();
    }
}

// ---

struct PathfinderComPtr<T> {
    ptr: *mut T,
}

impl<T> PathfinderComPtr<T> {
    #[inline]
    unsafe fn new(ptr: *mut T) -> PathfinderComPtr<T> {
        PathfinderComPtr {
            ptr: ptr,
        }
    }

    #[inline]
    fn into_raw(self) -> *mut T {
        let ptr = self.ptr;
        mem::forget(self);
        ptr
    }
}

impl<T> Clone for PathfinderComPtr<T> {
    #[inline]
    fn clone(&self) -> PathfinderComPtr<T> {
        unsafe {
            (*(self.ptr as *mut IUnknown)).AddRef();
        }
        PathfinderComPtr {
            ptr: self.ptr,
        }
    }
}

impl<T> Drop for PathfinderComPtr<T> {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            (*(self.ptr as *mut IUnknown)).Release();
        }
    }
}

impl<T> Deref for PathfinderComPtr<T> {
    type Target = *mut T;
    fn deref(&self) -> &*mut T {
        &self.ptr
    }
}

trait PathfinderCoclass {
    type InterfaceVtable: 'static;
    fn interface_guid() -> &'static GUID;
    fn vtable() -> &'static Self::InterfaceVtable;
}

struct PathfinderComObject<DerivedClass> where DerivedClass: PathfinderCoclass {
    vtable: &'static DerivedClass::InterfaceVtable,
    ref_count: AtomicUsize,
}

impl<DerivedClass> PathfinderComObject<DerivedClass> where DerivedClass: PathfinderCoclass {
    #[inline]
    unsafe fn construct() -> PathfinderComObject<DerivedClass> {
        PathfinderComObject {
            vtable: DerivedClass::vtable(),
            ref_count: AtomicUsize::new(1),
        }
    }

    unsafe extern "system" fn AddRef(this: *mut IUnknown) -> ULONG {
        let this = this as *mut PathfinderComObject<DerivedClass>;
        ((*this).ref_count.fetch_add(1, Ordering::SeqCst) + 1) as ULONG
    }

    unsafe extern "system" fn Release(this: *mut IUnknown) -> ULONG {
        let this = this as *mut PathfinderComObject<DerivedClass>;
        let new_ref_count = (*this).ref_count.fetch_sub(1, Ordering::SeqCst) - 1;
        if new_ref_count == 0 {
            drop(Box::from_raw(this))
        }
        new_ref_count as ULONG
    }

    unsafe extern "system" fn QueryInterface(this: *mut IUnknown,
                                             riid: REFIID,
                                             object: *mut *mut c_void)
                                             -> HRESULT {
        if object.is_null() {
            return E_POINTER
        }
        if guids_are_equal(&*riid, &uuid::IID_IUnknown) ||
                guids_are_equal(&*riid, DerivedClass::interface_guid()) {
            *object = this as *mut c_void;
            return S_OK
        }
        *object = ptr::null_mut();
        E_NOINTERFACE
    }
}

fn guids_are_equal(a: &GUID, b: &GUID) -> bool {
    a.Data1 == b.Data1 && a.Data2 == b.Data2 && a.Data3 == b.Data3 && a.Data4 == b.Data4
}
