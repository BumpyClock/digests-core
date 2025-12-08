// ABOUTME: C FFI bindings for the digests parsing core.
// ABOUTME: Exposes arena-allocated feed parsing results to Swift/Kotlin consumers.

/// FFI version constant for ABI compatibility checking.
const DIGESTS_FFI_VERSION: u32 = 1;

/// Returns the FFI ABI version number.
/// Consumers should check this matches their expected version.
#[no_mangle]
pub extern "C" fn digests_ffi_version() -> u32 {
    DIGESTS_FFI_VERSION
}

/// UTF-8 string slice for FFI. Not null-terminated.
/// Consumer must not mutate or free; memory owned by arena.
#[repr(C)]
pub struct DString {
    pub data: *const u8,
    pub len: usize,
}

impl DString {
    /// Creates an empty DString.
    pub const fn empty() -> Self {
        DString {
            data: std::ptr::null(),
            len: 0,
        }
    }
}

impl Default for DString {
    fn default() -> Self {
        Self::empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ffi_version() {
        assert_eq!(digests_ffi_version(), 1);
    }

    #[test]
    fn test_dstring_empty() {
        let s = DString::empty();
        assert!(s.data.is_null());
        assert_eq!(s.len, 0);
    }
}
