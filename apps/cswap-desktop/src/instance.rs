//! One Claude Swap per data dir. The key is the data dir, not the exe, so a test copy
//! started with `CSWAP_DESKTOP_USERDATA` runs beside the installed one, while a second
//! launch of the same setup (Start menu, login item, a double-click) finds the first and
//! asks it to show its window (0.1.23's `requestSingleInstanceLock` + `second-instance`).
//!
//! Windows: a named mutex + an auto-reset event. macOS/Linux: an flock'ed file + a Unix
//! socket in the data dir; a second launch writes one line to the socket.

use std::path::Path;

/// FNV-1a over the lower-cased path: Windows paths are case-insensitive.
pub fn key_for(data_dir: &Path) -> String {
    let s = data_dir.to_string_lossy().trim_end_matches(['\\', '/']).to_lowercase();
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{h:016x}")
}

pub enum Claim {
    /// This is the first: `on_show` callbacks come through [`Primary::on_show`].
    Primary(Primary),
    /// Another copy runs; it has been asked to show its window.
    Secondary,
}

#[cfg(windows)]
mod imp {
    use std::path::Path;
    use std::sync::Arc;

    use windows_sys::Win32::Foundation::{CloseHandle, ERROR_ACCESS_DENIED, ERROR_ALREADY_EXISTS, GetLastError, HANDLE, WAIT_OBJECT_0};
    use windows_sys::Win32::System::Threading::{CreateEventW, CreateMutexW, EVENT_MODIFY_STATE, INFINITE, OpenEventW, SetEvent, WaitForSingleObject};

    use super::{Claim, key_for};

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    struct Handle(HANDLE);
    // SAFETY: kernel handles may be used from any thread.
    unsafe impl Send for Handle {}
    unsafe impl Sync for Handle {}
    impl Drop for Handle {
        fn drop(&mut self) {
            // SAFETY: a handle we own.
            unsafe { CloseHandle(self.0) };
        }
    }

    pub struct Primary {
        _mutex: Handle,
        event: Arc<Handle>,
    }

    pub fn claim(data_dir: &Path) -> std::io::Result<Claim> {
        let key = key_for(data_dir);
        let mutex_name = wide(&format!("Local\\ClaudeSwap-{key}"));
        let event_name = wide(&format!("Local\\ClaudeSwap-show-{key}"));
        // SAFETY: names are NUL-terminated; handles are owned by `Handle` on every path.
        unsafe {
            let m = CreateMutexW(std::ptr::null(), 0, mutex_name.as_ptr());
            let err = GetLastError();
            if m.is_null() {
                if err == ERROR_ACCESS_DENIED {
                    return Ok(Claim::Secondary);
                }
                return Err(std::io::Error::from_raw_os_error(err as i32));
            }
            let m = Handle(m);
            if err == ERROR_ALREADY_EXISTS {
                drop(m);
                let h = OpenEventW(EVENT_MODIFY_STATE, 0, event_name.as_ptr());
                if !h.is_null() {
                    // Windows refuses the foreground to a background process; this launch
                    // was started by the user and may pass that right on.
                    windows_sys::Win32::UI::WindowsAndMessaging::AllowSetForegroundWindow(
                        windows_sys::Win32::UI::WindowsAndMessaging::ASFW_ANY,
                    );
                    SetEvent(h);
                    CloseHandle(h);
                }
                return Ok(Claim::Secondary);
            }
            let e = CreateEventW(std::ptr::null(), 0, 0, event_name.as_ptr());
            if e.is_null() {
                return Err(std::io::Error::last_os_error());
            }
            Ok(Claim::Primary(Primary { _mutex: m, event: Arc::new(Handle(e)) }))
        }
    }

    impl Primary {
        pub fn on_show(&self, f: impl Fn() + Send + 'static) {
            let event = self.event.clone();
            let _ = std::thread::Builder::new().name("second-instance".into()).spawn(move || {
                loop {
                    // SAFETY: the Arc keeps the handle open for this thread's lifetime.
                    if unsafe { WaitForSingleObject(event.0, INFINITE) } != WAIT_OBJECT_0 {
                        return;
                    }
                    f();
                }
            });
        }
    }
}

#[cfg(unix)]
mod imp {
    use std::fs::File;
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::path::{Path, PathBuf};

    use super::Claim;

    pub struct Primary {
        _lock: File,
        socket: Option<UnixListener>,
        path: PathBuf,
    }

    pub fn claim(data_dir: &Path) -> std::io::Result<Claim> {
        std::fs::create_dir_all(data_dir)?;
        let lock = File::options().create(true).truncate(false).write(true).open(data_dir.join("instance.lock"))?;
        let sock = data_dir.join("instance.sock");
        if lock.try_lock().is_err() {
            if let Ok(mut s) = UnixStream::connect(&sock) {
                let _ = s.write_all(b"show\n");
            }
            return Ok(Claim::Secondary);
        }
        // Holding the lock: whatever socket file is there was left by a copy that died.
        let _ = std::fs::remove_file(&sock);
        let socket = UnixListener::bind(&sock).ok();
        Ok(Claim::Primary(Primary { _lock: lock, socket, path: sock }))
    }

    impl Primary {
        pub fn on_show(&self, f: impl Fn() + Send + 'static) {
            let Some(l) = self.socket.as_ref().and_then(|s| s.try_clone().ok()) else { return };
            let _ = std::thread::Builder::new().name("second-instance".into()).spawn(move || {
                for s in l.incoming().flatten() {
                    let mut line = String::new();
                    if BufReader::new(s).read_line(&mut line).is_ok() && line.trim() == "show" {
                        f();
                    }
                }
            });
        }
    }

    impl Drop for Primary {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

pub use imp::{Primary, claim};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_ignores_case_and_trailing_separator() {
        assert_eq!(key_for(Path::new(r"C:\Users\P\AppData\Roaming\cswap-desktop")), key_for(Path::new(r"c:\users\p\appdata\roaming\CSWAP-DESKTOP\")));
        assert_ne!(key_for(Path::new("/a")), key_for(Path::new("/b")));
    }

    #[test]
    fn second_claim_is_secondary_and_wakes_the_first() {
        let dir = std::env::temp_dir().join(format!("cswap-instance-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let Claim::Primary(first) = claim(&dir).unwrap() else { panic!("first claim") };
        let (tx, rx) = std::sync::mpsc::channel();
        first.on_show(move || {
            let _ = tx.send(());
        });
        assert!(matches!(claim(&dir).unwrap(), Claim::Secondary));
        rx.recv_timeout(std::time::Duration::from_secs(5)).expect("the first copy was asked to show");
        drop(first);
        assert!(matches!(claim(&dir).unwrap(), Claim::Primary(_)), "released on drop");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
