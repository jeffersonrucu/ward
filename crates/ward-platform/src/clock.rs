//! Clock of the inactivity lock (Phase 24, R23).
//!
//! [`time_since_boot`] reads `CLOCK_BOOTTIME`: monotonic (does not go back with a change
//! of the system time, unlike `SystemTime`) and that **runs during suspend** (unlike
//! `Instant`, which uses `CLOCK_MONOTONIC`). So a suspend that `PrepareForSleep` did not
//! catch still counts as idle time.

use std::io;
use std::time::Duration;

use crate::sys;

/// Time since boot, counting suspend.
pub fn time_since_boot() -> io::Result<Duration> {
    sys::boot_clock()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn does_not_go_back_and_matches_uptime() {
        let a = time_since_boot().unwrap();
        std::thread::sleep(Duration::from_millis(20));
        let b = time_since_boot().unwrap();
        assert!(b >= a + Duration::from_millis(20), "{a:?} → {b:?}");
        // `/proc/uptime` also comes from `CLOCK_BOOTTIME` (with two decimals).
        let up = std::fs::read_to_string("/proc/uptime").unwrap();
        let secs: f64 = up.split_whitespace().next().unwrap().parse().unwrap();
        let c = time_since_boot().unwrap().as_secs_f64();
        assert!((c - secs).abs() < 2.0, "boottime {c} × uptime {secs}");
    }
}
