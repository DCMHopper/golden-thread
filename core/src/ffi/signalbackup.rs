use std::ffi::{CStr, CString};
use std::path::Path;

use crate::error::CoreError;
use zeroize::Zeroizing;

#[link(name = "signalbackup_tools_static", kind = "static")]
extern "C" {
    fn gt_decode_backup(
        backup_path: *const libc::c_char,
        passphrase: *const libc::c_char,
        out_db_path: *const libc::c_char,
        out_frames_dir: *const libc::c_char,
        overwrite: libc::c_int,
        err_buf: *mut libc::c_char,
        err_len: usize,
    ) -> libc::c_int;
}

pub struct DecodeOutput {
    pub db_path: String,
    pub frames_dir: String,
}

pub fn decode_backup(
    backup_path: &Path,
    passphrase: &str,
    out_db_path: &Path,
    out_frames_dir: &Path,
    overwrite: bool,
) -> Result<DecodeOutput, CoreError> {
    let backup_c = CString::new(backup_path.to_string_lossy().as_bytes())
        .map_err(|_| CoreError::InvalidArgument("invalid backup path".to_string()))?;
    let passphrase = Zeroizing::new(passphrase.to_string());
    let pass_c = CString::new(passphrase.as_str())
        .map_err(|_| CoreError::InvalidPassphrase("invalid passphrase".to_string()))?;
    let db_c = CString::new(out_db_path.to_string_lossy().as_bytes())
        .map_err(|_| CoreError::InvalidArgument("invalid db path".to_string()))?;
    let frames_c = CString::new(out_frames_dir.to_string_lossy().as_bytes())
        .map_err(|_| CoreError::InvalidArgument("invalid frames dir".to_string()))?;

    let mut err_buf = vec![0i8; 1024];

    let code = unsafe {
        gt_decode_backup(
            backup_c.as_ptr(),
            pass_c.as_ptr(),
            db_c.as_ptr(),
            frames_c.as_ptr(),
            if overwrite { 1 } else { 0 },
            err_buf.as_mut_ptr(),
            err_buf.len(),
        )
    };

    if code != 0 {
        let cstr = unsafe { CStr::from_ptr(err_buf.as_ptr()) };
        let msg = cstr.to_string_lossy().to_string();
        let msg = if msg.is_empty() {
            format!("signalbackup decode failed (code {})", code)
        } else {
            msg
        };
        return Err(CoreError::InvalidArgument(msg));
    }

    Ok(DecodeOutput {
        db_path: out_db_path.to_string_lossy().to_string(),
        frames_dir: out_frames_dir.to_string_lossy().to_string(),
    })
}
