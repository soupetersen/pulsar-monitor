//! One Pulsar at a time. A second launch asks the running one to show its
//! window and exits.
//! - Windows: a named mutex marks the running instance, a named event wakes it.
//! - Linux and macOS: a Unix socket in the runtime directory.

/// Returns `false` when another instance is running (it has been asked to
/// show itself). Otherwise calls `on_show` whenever a later launch asks for
/// the window, from a background thread.
pub fn acquire(on_show: impl Fn() + Send + 'static) -> bool {
    imp::acquire(on_show)
}

#[cfg(windows)]
mod imp {
    use windows_sys::Win32::Foundation::{ERROR_ALREADY_EXISTS, GetLastError};
    use windows_sys::Win32::System::Threading::{
        CreateEventW, CreateMutexW, EVENT_MODIFY_STATE, INFINITE, OpenEventW, SetEvent, WaitForSingleObject,
    };

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }

    const MUTEX: &str = r"Local\com.petersen.pulsar";
    const EVENT: &str = r"Local\com.petersen.pulsar.show";

    pub fn acquire(on_show: impl Fn() + Send + 'static) -> bool {
        unsafe {
            // Kept for the life of the process.
            let mutex = CreateMutexW(std::ptr::null(), 0, wide(MUTEX).as_ptr());
            if mutex.is_null() {
                return true;
            }
            if GetLastError() == ERROR_ALREADY_EXISTS {
                let event = OpenEventW(EVENT_MODIFY_STATE, 0, wide(EVENT).as_ptr());
                if !event.is_null() {
                    SetEvent(event);
                }
                return false;
            }
            let event = CreateEventW(std::ptr::null(), 0, 0, wide(EVENT).as_ptr()) as usize;
            if event != 0 {
                std::thread::spawn(move || {
                    loop {
                        if WaitForSingleObject(event as _, INFINITE) != 0 {
                            break;
                        }
                        on_show();
                    }
                });
            }
            true
        }
    }
}

#[cfg(unix)]
mod imp {
    use std::io::Write;
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::path::PathBuf;

    fn socket() -> PathBuf {
        let dir = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
        let user = std::env::var("USER").unwrap_or_default();
        dir.join(format!("com.petersen.pulsar-{user}.sock"))
    }

    pub fn acquire(on_show: impl Fn() + Send + 'static) -> bool {
        let path = socket();
        if let Ok(mut stream) = UnixStream::connect(&path) {
            let _ = stream.write_all(b"show");
            return false;
        }
        // Nobody answered: a leftover from a crash, if anything.
        let _ = std::fs::remove_file(&path);
        let Ok(listener) = UnixListener::bind(&path) else { return true };
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                if stream.is_ok() {
                    on_show();
                }
            }
        });
        true
    }
}
