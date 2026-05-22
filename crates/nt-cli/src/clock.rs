//! Wall-clock read port.
//!
//! Production wires `SystemClock`. Tests substitute `FixedClock` so
//! time-dependent code paths (session expiry, `startedAt` stamping) can be
//! verified deterministically without `std::thread::sleep`.

use time::OffsetDateTime;

pub trait Clock {
    #[allow(dead_code)] // wired in by Task 4 `session show` (GREEN)
    fn now(&self) -> OffsetDateTime;
}

pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> OffsetDateTime {
        OffsetDateTime::now_utc()
    }
}
