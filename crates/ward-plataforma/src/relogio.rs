//! Relógio da trava por inatividade (Fase 24, R23).
//!
//! [`tempo_desde_boot`] lê `CLOCK_BOOTTIME`: monotônico (não volta com uma mudança da
//! hora do sistema, ao contrário do `SystemTime`) e que **anda durante a suspensão** (ao
//! contrário do `Instant`, que usa `CLOCK_MONOTONIC`). Assim uma suspensão que o
//! `PrepareForSleep` não pegou ainda conta como tempo sem uso.

use std::io;
use std::time::Duration;

use crate::sys;

/// Tempo desde o boot, contando a suspensão.
pub fn tempo_desde_boot() -> io::Result<Duration> {
    sys::relogio_boot()
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn nao_volta_e_bate_com_o_uptime() {
        let a = tempo_desde_boot().unwrap();
        std::thread::sleep(Duration::from_millis(20));
        let b = tempo_desde_boot().unwrap();
        assert!(b >= a + Duration::from_millis(20), "{a:?} → {b:?}");
        // `/proc/uptime` também sai do `CLOCK_BOOTTIME` (com duas casas).
        let up = std::fs::read_to_string("/proc/uptime").unwrap();
        let seg: f64 = up.split_whitespace().next().unwrap().parse().unwrap();
        let c = tempo_desde_boot().unwrap().as_secs_f64();
        assert!((c - seg).abs() < 2.0, "boottime {c} × uptime {seg}");
    }
}
