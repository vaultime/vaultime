// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! A clock that keeps counting while the PC sleeps. Next to the monotonic
//! clock it tells a suspend apart from a change of the system clock.

/// Milliseconds since boot, time asleep included.
#[cfg(target_os = "linux")]
pub fn since_boot_ms() -> Option<i64> {
    let mut time = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: clock_gettime only writes to the timespec it is given.
    let result = unsafe { libc::clock_gettime(libc::CLOCK_BOOTTIME, &raw mut time) };
    (result == 0).then(|| time.tv_sec * 1000 + time.tv_nsec / 1_000_000)
}

/// Milliseconds since boot, time asleep included.
#[cfg(windows)]
pub fn since_boot_ms() -> Option<i64> {
    // SAFETY: GetTickCount64 takes no arguments and cannot fail.
    let ticks = unsafe { windows_sys::Win32::System::SystemInformation::GetTickCount64() };
    i64::try_from(ticks).ok()
}

#[cfg(not(any(target_os = "linux", windows)))]
pub fn since_boot_ms() -> Option<i64> {
    None
}

#[cfg(test)]
mod tests {
    #[test]
    #[cfg(any(target_os = "linux", windows))]
    fn counts_forward() {
        let first = super::since_boot_ms().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(30));
        assert!(super::since_boot_ms().unwrap() > first);
    }
}
