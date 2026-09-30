// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! How long the PC has slept since it started. Between two ticks it tells a
//! suspend apart from a change of the system clock, the same way on every
//! system, whether or not the monotonic clock stops during sleep.

/// Milliseconds the PC spent asleep since boot: the clock that counts
/// through sleep minus the one that does not.
#[cfg(target_os = "linux")]
pub fn asleep_ms() -> Option<i64> {
    let read = |clock| {
        let mut time = libc::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        // SAFETY: clock_gettime only writes to the timespec it is given.
        let result = unsafe { libc::clock_gettime(clock, &raw mut time) };
        (result == 0).then(|| time.tv_sec * 1000 + time.tv_nsec / 1_000_000)
    };
    Some(read(libc::CLOCK_BOOTTIME)? - read(libc::CLOCK_MONOTONIC)?)
}

/// Milliseconds the PC spent asleep since boot: the interrupt time that
/// counts through sleep minus the one that does not.
#[cfg(windows)]
pub fn asleep_ms() -> Option<i64> {
    use windows_sys::Win32::System::WindowsProgramming::{
        QueryInterruptTime, QueryUnbiasedInterruptTime,
    };
    // Both count in units of 100 nanoseconds.
    const UNITS_PER_MS: u64 = 10_000;
    let (mut with_sleep, mut without_sleep) = (0_u64, 0_u64);
    // SAFETY: both only write to the value they are given.
    let read = unsafe {
        QueryInterruptTime(&raw mut with_sleep);
        QueryUnbiasedInterruptTime(&raw mut without_sleep)
    };
    if read == 0 {
        return None;
    }
    i64::try_from(with_sleep.saturating_sub(without_sleep) / UNITS_PER_MS).ok()
}

#[cfg(not(any(target_os = "linux", windows)))]
pub fn asleep_ms() -> Option<i64> {
    None
}

#[cfg(test)]
mod tests {
    #[test]
    #[cfg(any(target_os = "linux", windows))]
    fn awake_time_adds_no_sleep() {
        let before = super::asleep_ms().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(30));
        let after = super::asleep_ms().unwrap();
        assert!(before >= 0);
        // Timer resolution can move the difference by a few milliseconds.
        assert!((after - before).abs() < 100);
    }
}
