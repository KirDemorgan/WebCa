use webca_core::{ControlId, DeviceError};

use crate::mapping::device_error;

pub fn platform_error(error: &windows::core::Error) -> DeviceError {
    DeviceError::Platform { code: error.code().0, message: error.message().trim().to_owned() }
}

pub fn control_error(id: ControlId, error: &windows::core::Error) -> DeviceError {
    device_error(id, error.code().0, &error.message())
}
