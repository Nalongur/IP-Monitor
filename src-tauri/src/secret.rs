//! DPAPI-backed storage for the one secret the app holds: the custom proxy
//! password.
//!
//! Spec §10.2 asks for the Windows credential store or DPAPI rather than a
//! plain-text field in `settings.json`. DPAPI with the *current user* scope is
//! the smaller of the two: no extra dependency, no extra prompt, and the blob
//! can only be decrypted by the same Windows account on the same machine.
//!
//! The blob is kept as hex so `settings.json` stays human-readable and the
//! secret stays opaque. Everything here degrades to `None` on failure — a
//! password that cannot be decrypted must not take the app down with it.

/// Encrypt `plain` for the current Windows user.
#[cfg(windows)]
pub fn protect(plain: &str) -> Option<String> {
    let sealed = imp::protect(plain.as_bytes())?;
    Some(to_hex(&sealed))
}

/// Decrypt a blob produced by [`protect`].
#[cfg(windows)]
pub fn unprotect(blob: &str) -> Option<String> {
    let raw = from_hex(blob)?;
    let plain = imp::unprotect(&raw)?;
    String::from_utf8(plain).ok()
}

/// Without DPAPI there is nowhere safe to put a password, so nothing is stored.
#[cfg(not(windows))]
pub fn protect(_plain: &str) -> Option<String> {
    None
}

#[cfg(not(windows))]
pub fn unprotect(_blob: &str) -> Option<String> {
    None
}

fn to_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(char::from_digit((byte >> 4) as u32, 16).unwrap_or('0'));
        out.push(char::from_digit((byte & 0x0f) as u32, 16).unwrap_or('0'));
    }
    out
}

fn from_hex(text: &str) -> Option<Vec<u8>> {
    let bytes = text.as_bytes();
    if bytes.len() % 2 != 0 {
        return None;
    }
    let mut out = Vec::with_capacity(bytes.len() / 2);
    for pair in bytes.chunks_exact(2) {
        let high = (pair[0] as char).to_digit(16)?;
        let low = (pair[1] as char).to_digit(16)?;
        out.push(((high << 4) | low) as u8);
    }
    Some(out)
}

#[cfg(windows)]
mod imp {
    use std::ptr;
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{
        CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };

    fn data_blob(bytes: &mut [u8]) -> CRYPT_INTEGER_BLOB {
        CRYPT_INTEGER_BLOB {
            cbData: bytes.len() as u32,
            pbData: bytes.as_mut_ptr(),
        }
    }

    /// Copy the out-blob into a `Vec` and hand the allocation back to Windows.
    unsafe fn take(out: &mut CRYPT_INTEGER_BLOB) -> Option<Vec<u8>> {
        if out.pbData.is_null() || out.cbData == 0 {
            return None;
        }
        let bytes = std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec();
        LocalFree(out.pbData as *mut core::ffi::c_void);
        Some(bytes)
    }

    pub fn protect(plain: &[u8]) -> Option<Vec<u8>> {
        let mut input = plain.to_vec();
        let mut in_blob = data_blob(&mut input);
        let mut out = CRYPT_INTEGER_BLOB {
            cbData: 0,
            pbData: ptr::null_mut(),
        };
        // No prompt: this runs from a command, not from a UI that could answer.
        let ok = unsafe {
            CryptProtectData(
                &mut in_blob,
                ptr::null(),
                ptr::null(),
                ptr::null(),
                ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut out,
            )
        };
        if ok == 0 {
            return None;
        }
        unsafe { take(&mut out) }
    }

    pub fn unprotect(sealed: &[u8]) -> Option<Vec<u8>> {
        let mut input = sealed.to_vec();
        let mut in_blob = data_blob(&mut input);
        let mut out = CRYPT_INTEGER_BLOB {
            cbData: 0,
            pbData: ptr::null_mut(),
        };
        let ok = unsafe {
            CryptUnprotectData(
                &mut in_blob,
                ptr::null_mut(),
                ptr::null(),
                ptr::null(),
                ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut out,
            )
        };
        if ok == 0 {
            return None;
        }
        unsafe { take(&mut out) }
    }
}

#[cfg(test)]
mod tests {
    use super::{from_hex, to_hex};

    #[test]
    fn hex_round_trip() {
        let bytes = [0u8, 1, 15, 16, 255];
        assert_eq!(from_hex(&to_hex(&bytes)).as_deref(), Some(bytes.as_slice()));
    }

    #[test]
    fn rejects_odd_and_non_hex() {
        assert!(from_hex("abc").is_none());
        assert!(from_hex("zz").is_none());
    }
}
