use crate::{raw, Error, Result};

const COINIT_MULTITHREADED: u32 = 0;
const RPC_E_CHANGED_MODE: i32 = 0x8001_0106_u32 as i32;

/// Guard returned by [`initialize_mta`].
#[derive(Debug)]
pub struct ComGuard {
    should_uninitialize: bool,
}

impl Drop for ComGuard {
    fn drop(&mut self) {
        if self.should_uninitialize {
            raw::co_uninitialize();
        }
    }
}

/// Initializes COM as MTA on the current thread.
pub fn initialize_mta() -> Result<ComGuard> {
    let hr = raw::co_initialize_ex(std::ptr::null_mut(), COINIT_MULTITHREADED);
    match hr {
        0 | 1 => Ok(ComGuard {
            should_uninitialize: hr == 0,
        }),
        RPC_E_CHANGED_MODE => Err(Error::ComInitialization {
            code: hr,
            message: "current thread is already initialized with an incompatible COM apartment"
                .to_owned(),
        }),
        code => Err(Error::ComInitialization {
            code,
            message: "CoInitializeEx failed".to_owned(),
        }),
    }
}

pub(crate) fn try_initialize_mta() -> Result<Option<ComGuard>> {
    match initialize_mta() {
        Ok(guard) => Ok(Some(guard)),
        Err(Error::ComInitialization { code, .. }) if code == RPC_E_CHANGED_MODE => Ok(None),
        Err(error) => Err(error),
    }
}
