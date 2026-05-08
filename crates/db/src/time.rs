use std::time::{SystemTime, UNIX_EPOCH};

/// Supplies timestamps for applied migrations so production and tests can choose different time sources.
pub trait TimestampSource {
    /// Returns the current timestamp in Unix milliseconds.
    fn current_timestamp_millis(&self) -> i64;
}

/// Reads migration timestamps from the system clock.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemTimestampSource;

impl TimestampSource for SystemTimestampSource {
    /// Converts the system clock into the integer millisecond format stored in SQLite.
    fn current_timestamp_millis(&self) -> i64 {
        let duration = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(std::time::Duration::from_secs(0));

        duration.as_millis() as i64
    }
}
