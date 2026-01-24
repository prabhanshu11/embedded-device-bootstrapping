//! Server load monitoring for adaptive behavior
//!
//! Monitors CPU, RAM, and I/O to:
//! - Throttle operations when overloaded
//! - Suggest clients handle heavy tasks locally
//! - Offload work to capable clients

use std::sync::Arc;
use sysinfo::System;
use tokio::sync::RwLock;
use tokio::time::{interval, Duration};

use pibox_core::protocol::{LoadHint, ServerLoad};
use pibox_core::ServerMessage;

use crate::state::AppState;

/// Load thresholds for adaptive behavior
const CPU_HIGH_THRESHOLD: f32 = 80.0;
const CPU_CRITICAL_THRESHOLD: f32 = 95.0;
const RAM_LOW_MB: u64 = 100;
const RAM_CRITICAL_MB: u64 = 50;

/// Main load monitoring loop
pub async fn monitor_loop(state: Arc<RwLock<AppState>>) {
    let mut sys = System::new_all();
    let interval_secs = {
        let s = state.read().await;
        s.load_report_interval
    };

    let mut ticker = interval(Duration::from_secs(interval_secs));

    loop {
        ticker.tick().await;

        // Refresh system info
        sys.refresh_cpu_usage();
        sys.refresh_memory();

        // Calculate metrics
        let cpu_percent = sys.global_cpu_usage();
        let ram_free_mb = sys.available_memory() / 1024 / 1024;

        // Determine hints based on load
        let hints = generate_hints(cpu_percent, ram_free_mb);

        // Check I/O busy (simplified - just check if CPU iowait is high)
        // In a real implementation, you'd check disk I/O specifically
        let io_busy = cpu_percent > CPU_HIGH_THRESHOLD;

        let load = ServerLoad {
            cpu_percent,
            ram_free_mb,
            io_busy,
            hints,
        };

        // Update state and broadcast
        {
            let mut s = state.write().await;
            s.load = load.clone();

            // Broadcast to all clients
            s.broadcast(ServerMessage::Load(load));
        }

        tracing::debug!(
            "Load: CPU {:.1}%, RAM free {}MB, {} transfers active",
            cpu_percent,
            ram_free_mb,
            state.read().await.active_transfers
        );
    }
}

/// Generate load hints based on current metrics
fn generate_hints(cpu_percent: f32, ram_free_mb: u64) -> Vec<LoadHint> {
    let mut hints = Vec::new();

    // CPU-based hints
    if cpu_percent >= CPU_CRITICAL_THRESHOLD {
        hints.push(LoadHint::ThrottleTransfers);
        hints.push(LoadHint::GenerateThumbnailsLocally);
        hints.push(LoadHint::SearchLocally);
        hints.push(LoadHint::Recovering);
    } else if cpu_percent >= CPU_HIGH_THRESHOLD {
        hints.push(LoadHint::ThrottleTransfers);
        hints.push(LoadHint::GenerateThumbnailsLocally);
    }

    // RAM-based hints
    if ram_free_mb <= RAM_CRITICAL_MB {
        hints.push(LoadHint::ThrottleTransfers);
        hints.push(LoadHint::Recovering);
    } else if ram_free_mb <= RAM_LOW_MB {
        hints.push(LoadHint::SearchLocally);
    }

    hints
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hints_normal_load() {
        let hints = generate_hints(50.0, 500);
        assert!(hints.is_empty());
    }

    #[test]
    fn test_hints_high_cpu() {
        let hints = generate_hints(85.0, 500);
        assert!(hints.contains(&LoadHint::ThrottleTransfers));
        assert!(hints.contains(&LoadHint::GenerateThumbnailsLocally));
    }

    #[test]
    fn test_hints_critical_load() {
        let hints = generate_hints(96.0, 40);
        assert!(hints.contains(&LoadHint::Recovering));
    }
}
