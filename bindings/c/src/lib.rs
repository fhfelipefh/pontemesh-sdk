use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_void};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::PathBuf;
use std::ptr;

use pontemesh_sdk_core::{p2p::P2pConfig, ErrorCode, PontemeshClientConfig, SyncObjectRequest};

pub struct PontemeshClient {
    inner: pontemesh_sdk_core::PontemeshClient,
    last_error: Option<CString>,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PontemeshStatus {
    PontemeshOk = 0,
    PontemeshInvalidArgument = 1,
    PontemeshOriginRequestFailed = 2,
    PontemeshAccessDenied = 3,
    PontemeshHashMismatch = 4,
    PontemeshNoSourceAvailable = 5,
    PontemeshIoError = 6,
    PontemeshCancelled = 7,
    PontemeshInternalError = 255,
}

pub type PontemeshProgressCallback = Option<
    unsafe extern "C" fn(
        fragment_index: u32,
        bytes_downloaded: u64,
        total_bytes: u64,
        source_type: *const c_char,
        user_data: *mut c_void,
    ),
>;

#[no_mangle]
/// # Safety
///
/// `origin_url` and `application_token` must be valid null-terminated UTF-8 C strings.
/// `out_client` must be a valid writable pointer. The returned client must be released with
/// `pontemesh_client_free`.
pub unsafe extern "C" fn pontemesh_client_create(
    origin_url: *const c_char,
    application_token: *const c_char,
    out_client: *mut *mut PontemeshClient,
) -> PontemeshStatus {
    ffi_boundary(|| {
        if out_client.is_null() {
            return PontemeshStatus::PontemeshInvalidArgument;
        }
        *out_client = ptr::null_mut();
        let origin_url = match read_string(origin_url) {
            Ok(value) => value,
            Err(status) => return status,
        };
        let application_token = match read_string(application_token) {
            Ok(value) => value,
            Err(status) => return status,
        };
        if origin_url.trim().is_empty() || application_token.trim().is_empty() {
            return PontemeshStatus::PontemeshInvalidArgument;
        }
        let client = PontemeshClient {
            inner: pontemesh_sdk_core::PontemeshClient::new(PontemeshClientConfig {
                origin_url,
                application_token,
                p2p: P2pConfig::default(),
            }),
            last_error: None,
        };
        *out_client = Box::into_raw(Box::new(client));
        PontemeshStatus::PontemeshOk
    })
}

#[no_mangle]
/// # Safety
///
/// `client` must be a pointer returned by `pontemesh_client_create` and not yet freed.
/// `listen_addr` may be null; when provided it must be a valid null-terminated UTF-8 C string.
pub unsafe extern "C" fn pontemesh_client_enable_p2p(
    client: *mut PontemeshClient,
    listen_addr: *const c_char,
) -> PontemeshStatus {
    ffi_boundary(|| {
        let client = match client.as_mut() {
            Some(client) => client,
            None => return PontemeshStatus::PontemeshInvalidArgument,
        };
        let listen_addr = if listen_addr.is_null() {
            None
        } else {
            match read_string(listen_addr) {
                Ok(value) => Some(value),
                Err(status) => return set_error(client, "listen_addr is invalid", status),
            }
        };
        match client.inner.enable_p2p(listen_addr.as_deref()) {
            Ok(()) => {
                client.last_error = None;
                PontemeshStatus::PontemeshOk
            }
            Err(error) => set_error(client, &error.to_string(), status_from_code(error.code())),
        }
    })
}

#[no_mangle]
/// # Safety
///
/// `client` must be a pointer returned by `pontemesh_client_create` and not yet freed.
/// `bucket`, `key`, and `destination` must be valid null-terminated UTF-8 C strings.
pub unsafe extern "C" fn pontemesh_client_sync_object(
    client: *mut PontemeshClient,
    bucket: *const c_char,
    key: *const c_char,
    destination: *const c_char,
) -> PontemeshStatus {
    pontemesh_client_sync_object_with_progress(
        client,
        bucket,
        key,
        destination,
        None,
        ptr::null_mut(),
    )
}

#[no_mangle]
/// # Safety
///
/// `client` must be a pointer returned by `pontemesh_client_create` and not yet freed.
/// `bucket`, `key`, and `destination` must be valid null-terminated UTF-8 C strings.
/// If provided, `callback` must remain valid for the duration of the call and must tolerate
/// `user_data` being passed back unchanged.
pub unsafe extern "C" fn pontemesh_client_sync_object_with_progress(
    client: *mut PontemeshClient,
    bucket: *const c_char,
    key: *const c_char,
    destination: *const c_char,
    callback: PontemeshProgressCallback,
    user_data: *mut c_void,
) -> PontemeshStatus {
    ffi_boundary(|| {
        let client = match client.as_mut() {
            Some(client) => client,
            None => return PontemeshStatus::PontemeshInvalidArgument,
        };
        let bucket = match read_string(bucket) {
            Ok(value) => value,
            Err(status) => return set_error(client, "bucket is invalid", status),
        };
        let key = match read_string(key) {
            Ok(value) => value,
            Err(status) => return set_error(client, "key is invalid", status),
        };
        let destination = match read_string(destination) {
            Ok(value) => value,
            Err(status) => return set_error(client, "destination is invalid", status),
        };
        let request = SyncObjectRequest {
            bucket,
            key,
            destination: PathBuf::from(destination),
        };
        let result = if let Some(callback) = callback {
            let mut progress =
                |fragment_index, bytes_downloaded, total_bytes, source_type: &str| {
                    if let Ok(source_type) = CString::new(source_type) {
                        callback(
                            fragment_index,
                            bytes_downloaded,
                            total_bytes,
                            source_type.as_ptr(),
                            user_data,
                        );
                    }
                };
            client
                .inner
                .sync_object_with_progress(request, Some(&mut progress))
        } else {
            client.inner.sync_object(request)
        };
        match result {
            Ok(()) => {
                client.last_error = None;
                PontemeshStatus::PontemeshOk
            }
            Err(error) => set_error(client, &error.to_string(), status_from_code(error.code())),
        }
    })
}

#[no_mangle]
/// # Safety
///
/// `client` must be a pointer returned by `pontemesh_client_create` and not yet freed.
/// `buffer` must point to a writable memory region of at least `buffer_len` bytes.
pub unsafe extern "C" fn pontemesh_client_get_last_error(
    client: *mut PontemeshClient,
    buffer: *mut c_char,
    buffer_len: usize,
) -> PontemeshStatus {
    ffi_boundary(|| {
        if client.is_null() || buffer.is_null() || buffer_len == 0 {
            return PontemeshStatus::PontemeshInvalidArgument;
        }
        let client = &mut *client;
        let message = client
            .last_error
            .as_ref()
            .map(|error| error.as_bytes())
            .unwrap_or(b"");
        let copy_len = message.len().min(buffer_len.saturating_sub(1));
        ptr::copy_nonoverlapping(message.as_ptr(), buffer.cast::<u8>(), copy_len);
        *buffer.add(copy_len) = 0;
        PontemeshStatus::PontemeshOk
    })
}

#[no_mangle]
/// # Safety
///
/// `client` must be null or a pointer previously returned by `pontemesh_client_create`.
/// Passing the same non-null pointer more than once is undefined behavior.
pub unsafe extern "C" fn pontemesh_client_free(client: *mut PontemeshClient) {
    if !client.is_null() {
        drop(Box::from_raw(client));
    }
}

unsafe fn read_string(value: *const c_char) -> Result<String, PontemeshStatus> {
    if value.is_null() {
        return Err(PontemeshStatus::PontemeshInvalidArgument);
    }
    CStr::from_ptr(value)
        .to_str()
        .map(|value| value.to_string())
        .map_err(|_| PontemeshStatus::PontemeshInvalidArgument)
}

fn set_error(
    client: &mut PontemeshClient,
    message: &str,
    status: PontemeshStatus,
) -> PontemeshStatus {
    client.last_error = Some(cstring_lossy(message));
    status
}

fn cstring_lossy(message: &str) -> CString {
    CString::new(message)
        .unwrap_or_else(|_| CString::new("error contained nul byte").expect("static string"))
}

fn status_from_code(code: ErrorCode) -> PontemeshStatus {
    match code {
        ErrorCode::InvalidArgument => PontemeshStatus::PontemeshInvalidArgument,
        ErrorCode::OriginRequestFailed => PontemeshStatus::PontemeshOriginRequestFailed,
        ErrorCode::AccessDenied => PontemeshStatus::PontemeshAccessDenied,
        ErrorCode::HashMismatch => PontemeshStatus::PontemeshHashMismatch,
        ErrorCode::NoSourceAvailable | ErrorCode::PeerTransportNotEnabled => {
            PontemeshStatus::PontemeshNoSourceAvailable
        }
        ErrorCode::IoError => PontemeshStatus::PontemeshIoError,
        ErrorCode::Cancelled => PontemeshStatus::PontemeshCancelled,
        ErrorCode::InternalError => PontemeshStatus::PontemeshInternalError,
    }
}

fn ffi_boundary(callback: impl FnOnce() -> PontemeshStatus) -> PontemeshStatus {
    catch_unwind(AssertUnwindSafe(callback)).unwrap_or(PontemeshStatus::PontemeshInternalError)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ffi_create_rejects_invalid_argument_and_free_is_safe() {
        let mut client: *mut PontemeshClient = std::ptr::null_mut();
        let status =
            unsafe { pontemesh_client_create(std::ptr::null(), std::ptr::null(), &mut client) };
        assert_eq!(status, PontemeshStatus::PontemeshInvalidArgument);
        assert!(client.is_null());
        unsafe { pontemesh_client_free(client) };
    }
}
