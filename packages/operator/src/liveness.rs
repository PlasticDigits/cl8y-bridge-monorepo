//! Process liveness for `/health` (staleness detection).

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static LAST_ACTIVITY_UNIX_SECS: AtomicU64 = AtomicU64::new(0);

fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Mark that a watcher successfully completed a poll cycle (any chain).
pub fn touch_activity() {
    LAST_ACTIVITY_UNIX_SECS.store(now_unix_secs(), Ordering::Relaxed);
}

pub fn last_activity_unix_secs() -> u64 {
    LAST_ACTIVITY_UNIX_SECS.load(Ordering::Relaxed)
}

/// Max seconds without a successful poll before `/health` returns unhealthy (default 8 hours).
pub fn max_health_idle_secs() -> u64 {
    std::env::var("HEALTH_MAX_IDLE_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(8 * 3600)
}

/// Grace period after process start before requiring watcher heartbeats (default 5 minutes).
pub fn health_startup_grace_secs() -> u64 {
    std::env::var("HEALTH_STARTUP_GRACE_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(300)
}

/// True if `/health` should report stale / unavailable.
pub fn health_is_stale(since_process_start_secs: u64) -> bool {
    let last = last_activity_unix_secs();
    let idle = now_unix_secs().saturating_sub(last);
    let grace = health_startup_grace_secs();
    if since_process_start_secs <= grace {
        return false;
    }
    if last == 0 {
        return true;
    }
    idle > max_health_idle_secs()
}
