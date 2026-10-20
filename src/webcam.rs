//! Webcam use, found through open `/dev/video*` files of all visible processes.

use std::fs;

/// True when a process has a video device open.
///
/// Only processes that the current user can inspect are checked. On a
/// desktop these are the processes of the logged in user.
pub fn in_use() -> bool {
    let Ok(procs) = fs::read_dir("/proc") else {
        return false;
    };
    for proc in procs.flatten() {
        let name = proc.file_name();
        if !name.as_encoded_bytes().iter().all(u8::is_ascii_digit) {
            continue;
        }
        let Ok(fds) = fs::read_dir(proc.path().join("fd")) else {
            continue;
        };
        for fd in fds.flatten() {
            if let Ok(target) = fs::read_link(fd.path()) {
                if target
                    .as_os_str()
                    .as_encoded_bytes()
                    .starts_with(b"/dev/video")
                {
                    return true;
                }
            }
        }
    }
    false
}
