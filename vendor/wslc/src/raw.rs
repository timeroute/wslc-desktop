use wslc_sys::runtime;

use crate::{Error, Result};

pub(crate) use runtime::{CaptureRegistration, Sdk};

pub(crate) fn sdk() -> Result<&'static Sdk> {
    runtime::sdk().map_err(map_error)
}

pub(crate) fn co_initialize_ex(reserved: *mut std::ffi::c_void, coinit: u32) -> wslc_sys::HRESULT {
    runtime::co_initialize_ex(reserved, coinit)
}

pub(crate) fn co_uninitialize() {
    runtime::co_uninitialize();
}

pub(crate) fn wait_for_single_object(handle: wslc_sys::HANDLE, timeout_ms: u32) -> u32 {
    runtime::wait_for_single_object(handle, timeout_ms)
}

pub(crate) fn image_name_to_string(
    name: &[std::ffi::c_char; wslc_sys::WSLC_IMAGE_NAME_LENGTH],
) -> Result<String> {
    runtime::image_name_to_string(name).map_err(map_error)
}

pub(crate) fn map_result<T>(result: runtime::Result<T>) -> Result<T> {
    result.map_err(map_error)
}

fn map_error(error: runtime::Error) -> Error {
    match error {
        runtime::Error::UnsupportedPlatform(value) => Error::UnsupportedPlatform(value),
        runtime::Error::SdkNotFound(value) => Error::SdkNotFound(value),
        runtime::Error::ComInitialization { code, message } => {
            Error::ComInitialization { code, message }
        }
        runtime::Error::HResult { code, message } => Error::HResult { code, message },
        runtime::Error::InvalidInput(value) => Error::InvalidInput(value),
        runtime::Error::Utf8(value) => Error::Utf8(value),
    }
}
