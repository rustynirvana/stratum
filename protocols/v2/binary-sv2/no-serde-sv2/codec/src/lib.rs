//! ```txt
//! SERDE    <-> Sv2
//! bool     <-> BOOL
//! u8       <-> U8
//! u16      <-> U16
//! U24      <-> U24
//! u32      <-> u32
//! f32      <-> f32 // not in the spec but used
//! u64      <-> u64 // not in the spec but used
//! U256     <-> U256
//! Str0255  <-> STRO_255
//! Signature<-> SIGNATURE
//! B032     <-> B0_32 // not in the spec but used
//! B0255    <-> B0_255
//! B064K    <-> B0_64K
//! B016M    <-> B0_16M
//! [u8]     <-> BYTES
//! Pubkey   <-> PUBKEY
//! Seq0255  <-> SEQ0_255[T]
//! Seq064K  <-> SEQ0_64K[T]
//! ```
#[cfg(not(feature = "no_std"))]
use std::io::{Error as E, ErrorKind};

mod codec;
mod datatypes;
pub use datatypes::{
    PubKey, Seq0255, Seq064K, Signature, Str0255, U32AsRef, B016M, B0255, B032, B064K, U24, U256,
};

pub use crate::codec::{
    decodable::Decodable,
    encodable::{Encodable, EncodableField},
    GetSize, SizeHint,
};

#[allow(clippy::wrong_self_convention)]
pub fn to_bytes<T: Encodable + GetSize>(src: T) -> Result<Vec<u8>, Error> {
    let mut result = vec![0_u8; src.get_size()];
    src.to_bytes(&mut result)?;
    Ok(result)
}

#[allow(clippy::wrong_self_convention)]
pub fn to_writer<T: Encodable>(src: T, dst: &mut [u8]) -> Result<(), Error> {
    src.to_bytes(dst)?;
    Ok(())
}

pub fn from_bytes<'a, T: Decodable<'a>>(data: &'a mut [u8]) -> Result<T, Error> {
    T::from_bytes(data)
}

pub mod decodable {
    pub use crate::codec::decodable::{Decodable, DecodableField, FieldMarker};
    //pub use crate::codec::decodable::PrimitiveMarker;
}

pub mod encodable {
    pub use crate::codec::encodable::{Encodable, EncodableField};
}

#[macro_use]
extern crate alloc;

#[derive(Debug)]
pub enum Error {
    OutOfBound,
    NotABool(u8),
    /// -> (expected size, actual size)
    WriteError(usize, usize),
    U24TooBig(u32),
    InvalidSignatureSize(usize),
    InvalidU256(usize),
    InvalidU24(u32),
    InvalidB0255Size(usize),
    InvalidB064KSize(usize),
    InvalidB016MSize(usize),
    InvalidSeq0255Size(usize),
    /// Error when trying to encode a non-primitive data type
    NonPrimitiveTypeCannotBeEncoded,
    PrimitiveConversionError,
    DecodableConversionError,
    UnInitializedDecoder,
    #[cfg(not(feature = "no_std"))]
    IoError(E),
    ReadError(usize, usize),
    VoidFieldMarker,
    /// Error when `Inner` type value exceeds max size.
    /// (ISFIXED, SIZE, HEADERSIZE, MAXSIZE, bad value vec, bad value length)
    ValueExceedsMaxSize(bool, usize, usize, usize, Vec<u8>, usize),
    /// Error when sequence value (`Seq0255`, `Seq064K`) exceeds max size
    SeqExceedsMaxSize,
    NoDecodableFieldPassed,
    ValueIsNotAValidProtocol(u8),
    UnknownMessageType(u8),
}

#[cfg(not(feature = "no_std"))]
impl From<E> for Error {
    fn from(v: E) -> Self {
        match v.kind() {
            ErrorKind::UnexpectedEof => Error::OutOfBound,
            _ => Error::IoError(v),
        }
    }
}

/// FFI-safe Error
#[repr(C)]
#[derive(Debug)]
pub enum CError {
    OutOfBound,
    NotABool(u8),
    /// -> (expected size, actual size)
    WriteError(usize, usize),
    U24TooBig(u32),
    InvalidSignatureSize(usize),
    InvalidU256(usize),
    InvalidU24(u32),
    InvalidB0255Size(usize),
    InvalidB064KSize(usize),
    InvalidB016MSize(usize),
    InvalidSeq0255Size(usize),
    /// Error when trying to encode a non-primitive data type
    NonPrimitiveTypeCannotBeEncoded,
    PrimitiveConversionError,
    DecodableConversionError,
    UnInitializedDecoder,
    #[cfg(not(feature = "no_std"))]
    IoError,
    ReadError(usize, usize),
    VoidFieldMarker,
    /// Error when `Inner` type value exceeds max size.
    /// (ISFIXED, SIZE, HEADERSIZE, MAXSIZE, bad value vec, bad value length)
    ValueExceedsMaxSize(bool, usize, usize, usize, CVec, usize),
    /// Error when sequence value (`Seq0255`, `Seq064K`) exceeds max size
    SeqExceedsMaxSize,
    NoDecodableFieldPassed,
    ValueIsNotAValidProtocol(u8),
    UnknownMessageType(u8),
}

impl From<Error> for CError {
    fn from(e: Error) -> CError {
        match e {
            Error::OutOfBound => CError::OutOfBound,
            Error::NotABool(u) => CError::NotABool(u),
            Error::WriteError(u1, u2) => CError::WriteError(u1, u2),
            Error::U24TooBig(u) => CError::U24TooBig(u),
            Error::InvalidSignatureSize(u) => CError::InvalidSignatureSize(u),
            Error::InvalidU256(u) => CError::InvalidU256(u),
            Error::InvalidU24(u) => CError::InvalidU24(u),
            Error::InvalidB0255Size(u) => CError::InvalidB0255Size(u),
            Error::InvalidB064KSize(u) => CError::InvalidB064KSize(u),
            Error::InvalidB016MSize(u) => CError::InvalidB016MSize(u),
            Error::InvalidSeq0255Size(u) => CError::InvalidSeq0255Size(u),
            Error::NonPrimitiveTypeCannotBeEncoded => CError::NonPrimitiveTypeCannotBeEncoded,
            Error::PrimitiveConversionError => CError::PrimitiveConversionError,
            Error::DecodableConversionError => CError::DecodableConversionError,
            Error::UnInitializedDecoder => CError::UnInitializedDecoder,
            Error::IoError(_) => CError::IoError,
            Error::ReadError(u1, u2) => CError::ReadError(u1, u2),
            Error::VoidFieldMarker => CError::VoidFieldMarker,
            Error::ValueExceedsMaxSize(isfixed, size, headersize, maxsize, bad_value, bad_len) => {
                let bv1: &[u8] = bad_value.as_ref();
                let bv: CVec = bv1.into();
                CError::ValueExceedsMaxSize(isfixed, size, headersize, maxsize, bv, bad_len)
            }
            Error::SeqExceedsMaxSize => CError::SeqExceedsMaxSize,
            Error::NoDecodableFieldPassed => CError::NoDecodableFieldPassed,
            Error::ValueIsNotAValidProtocol(u) => CError::ValueIsNotAValidProtocol(u),
            Error::UnknownMessageType(u) => CError::UnknownMessageType(u),
        }
    }
}

impl Drop for CError {
    fn drop(&mut self) {
        match self {
            Self::OutOfBound => (),
            Self::NotABool(_) => (),
            Self::WriteError(_, _) => (),
            Self::U24TooBig(_) => (),
            Self::InvalidSignatureSize(_) => (),
            Self::InvalidU256(_) => (),
            Self::InvalidU24(_) => (),
            Self::InvalidB0255Size(_) => (),
            Self::InvalidB064KSize(_) => (),
            Self::InvalidB016MSize(_) => (),
            Self::InvalidSeq0255Size(_) => (),
            Self::NonPrimitiveTypeCannotBeEncoded => (),
            Self::PrimitiveConversionError => (),
            Self::DecodableConversionError => (),
            Self::UnInitializedDecoder => (),
            Self::IoError => (),
            Self::ReadError(_, _) => (),
            Self::VoidFieldMarker => (),
            Self::ValueExceedsMaxSize(_, _, _, _, cvec, _) => free_vec(cvec),
            Self::SeqExceedsMaxSize => (),
            Self::NoDecodableFieldPassed => (),
            Self::ValueIsNotAValidProtocol(_) => (),
            Self::UnknownMessageType(_) => (),
        };
    }
}

/// Vec<u8> is used as the Sv2 type Bytes
impl GetSize for Vec<u8> {
    fn get_size(&self) -> usize {
        self.len()
    }
}

// Only needed for implement encodable for Frame never called
impl<'a> From<Vec<u8>> for EncodableField<'a> {
    fn from(_v: Vec<u8>) -> Self {
        unreachable!()
    }
}

#[cfg(feature = "with_buffer_pool")]
impl<'a> From<buffer_sv2::Slice> for EncodableField<'a> {
    fn from(_v: buffer_sv2::Slice) -> Self {
        unreachable!()
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CVec {
    data: *mut u8,
    len: usize,
    capacity: usize,
}

impl CVec {
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        unsafe { core::slice::from_raw_parts_mut(self.data, self.len) }
    }

    /// Used when we need to fill a buffer allocated in rust from C.
    ///
    /// # Safety
    ///
    /// This function construct a CVec without taking ownership of the pointed buffer so if the
    /// owner drop them the CVec will point to garbage.
    #[allow(clippy::wrong_self_convention)]
    pub fn as_shared_buffer(v: &mut [u8]) -> Self {
        let (data, len) = (v.as_mut_ptr(), v.len());
        Self {
            data,
            len,
            capacity: len,
        }
    }
}

impl From<&[u8]> for CVec {
    fn from(v: &[u8]) -> Self {
        let mut buffer: Vec<u8> = vec![0; v.len()];
        buffer.copy_from_slice(v);

        // Get the length, first, then the pointer (doing it the other way around **currently** doesn't cause UB, but it may be unsound due to unclear (to me, at least) guarantees of the std lib)
        let len = buffer.len();
        let ptr = buffer.as_mut_ptr();
        std::mem::forget(buffer);

        CVec {
            data: ptr,
            len,
            capacity: len,
        }
    }
}

/// Given a C allocated buffer return a rust allocated CVec
///
/// # Safety
///
#[no_mangle]
pub unsafe extern "C" fn cvec_from_buffer(data: *const u8, len: usize) -> CVec {
    let input = std::slice::from_raw_parts(data, len);

    let mut buffer: Vec<u8> = vec![0; len];
    buffer.copy_from_slice(input);

    // Get the length, first, then the pointer (doing it the other way around **currently** doesn't cause UB, but it may be unsound due to unclear (to me, at least) guarantees of the std lib)
    let len = buffer.len();
    let ptr = buffer.as_mut_ptr();
    std::mem::forget(buffer);

    CVec {
        data: ptr,
        len,
        capacity: len,
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CVec2 {
    data: *mut CVec,
    len: usize,
    capacity: usize,
}

impl CVec2 {
    pub fn as_mut_slice(&mut self) -> &mut [CVec] {
        unsafe { core::slice::from_raw_parts_mut(self.data, self.len) }
    }
}
impl From<CVec2> for Vec<CVec> {
    fn from(v: CVec2) -> Self {
        unsafe { Vec::from_raw_parts(v.data, v.len, v.capacity) }
    }
}

pub fn free_vec(buf: &mut CVec) {
    let _: Vec<u8> = unsafe { Vec::from_raw_parts(buf.data, buf.len, buf.capacity) };
}

pub fn free_vec_2(buf: &mut CVec2) {
    let vs: Vec<CVec> = unsafe { Vec::from_raw_parts(buf.data, buf.len, buf.capacity) };
    for mut s in vs {
        free_vec(&mut s)
    }
}

impl<'a, const A: bool, const B: usize, const C: usize, const D: usize>
    From<datatypes::Inner<'a, A, B, C, D>> for CVec
{
    fn from(v: datatypes::Inner<'a, A, B, C, D>) -> Self {
        let (ptr, len, cap): (*mut u8, usize, usize) = match v {
            datatypes::Inner::Ref(inner) => {
                // Data is copied in a vector that then will be forgetted from the allocator,
                // cause the owner of the data is going to be dropped by rust
                let mut inner: Vec<u8> = inner.into();

                // Get the length, first, then the pointer (doing it the other way around **currently** doesn't cause UB, but it may be unsound due to unclear (to me, at least) guarantees of the std lib)
                let len = inner.len();
                let cap = inner.capacity();
                let ptr = inner.as_mut_ptr();
                std::mem::forget(inner);

                (ptr, len, cap)
            }
            datatypes::Inner::Owned(mut inner) => {
                // Get the length, first, then the pointer (doing it the other way around **currently** doesn't cause UB, but it may be unsound due to unclear (to me, at least) guarantees of the std lib)
                let len = inner.len();
                let cap = inner.capacity();
                let ptr = inner.as_mut_ptr();
                std::mem::forget(inner);

                (ptr, len, cap)
            }
        };
        Self {
            data: ptr,
            len,
            capacity: cap,
        }
    }
}

/// # Safety
///
#[no_mangle]
pub unsafe extern "C" fn init_cvec2() -> CVec2 {
    let mut buffer = Vec::<CVec>::new();

    // Get the length, first, then the pointer (doing it the other way around **currently** doesn't cause UB, but it may be unsound due to unclear (to me, at least) guarantees of the std lib)
    let len = buffer.len();
    let ptr = buffer.as_mut_ptr();
    std::mem::forget(buffer);

    CVec2 {
        data: ptr,
        len,
        capacity: len,
    }
}

/// The caller is reponsible for NOT adding duplicate cvecs to the cvec2 structure,
/// as this can lead to double free errors when the message is dropped.
/// # Safety
///
#[no_mangle]
pub unsafe extern "C" fn cvec2_push(cvec2: &mut CVec2, cvec: CVec) {
    let mut buffer: Vec<CVec> = Vec::from_raw_parts(cvec2.data, cvec2.len, cvec2.capacity);
    buffer.push(cvec);

    let len = buffer.len();
    let ptr = buffer.as_mut_ptr();
    std::mem::forget(buffer);

    cvec2.data = ptr;
    cvec2.len = len;
    cvec2.capacity = len;
}

impl<'a, T: Into<CVec>> From<Seq0255<'a, T>> for CVec2 {
    fn from(v: Seq0255<'a, T>) -> Self {
        let mut v: Vec<CVec> = v.0.into_iter().map(|x| x.into()).collect();
        // Get the length, first, then the pointer (doing it the other way around **currently** doesn't cause UB, but it may be unsound due to unclear (to me, at least) guarantees of the std lib)
        let len = v.len();
        let capacity = v.capacity();
        let data = v.as_mut_ptr();
        std::mem::forget(v);
        Self {
            data,
            len,
            capacity,
        }
    }
}
impl<'a, T: Into<CVec>> From<Seq064K<'a, T>> for CVec2 {
    fn from(v: Seq064K<'a, T>) -> Self {
        let mut v: Vec<CVec> = v.0.into_iter().map(|x| x.into()).collect();
        // Get the length, first, then the pointer (doing it the other way around **currently** doesn't cause UB, but it may be unsound due to unclear (to me, at least) guarantees of the std lib)
        let len = v.len();
        let capacity = v.capacity();
        let data = v.as_mut_ptr();
        std::mem::forget(v);
        Self {
            data,
            len,
            capacity,
        }
    }
}

#[no_mangle]
pub extern "C" fn _c_export_u24(_a: U24) {}
#[no_mangle]
pub extern "C" fn _c_export_cvec(_a: CVec) {}
#[no_mangle]
pub extern "C" fn _c_export_cvec2(_a: CVec2) {}
