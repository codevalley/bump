// Re-export all necessary types and interfaces
mod types;
mod matching;
mod q_core;
mod q_matching;
mod q_impl;

// Public exports from the queue module
pub use types::*;
pub use q_core::UnifiedQueue;

// Constants
/// Default time difference between requests in milliseconds
pub const DEFAULT_MAX_TIME_DIFF_MS: i64 = 500;

/// Default score threshold without custom key match
pub const DEFAULT_MIN_SCORE_WITHOUT_KEY: i32 = 150;

/// Default score threshold with custom key match
pub const DEFAULT_MIN_SCORE_WITH_KEY: i32 = 100;

/// Default custom key match score bonus
pub const DEFAULT_CUSTOM_KEY_MATCH_BONUS: i32 = 200;

/// Default maximum distance for matching (in meters)
pub const DEFAULT_MAX_DISTANCE_METERS: f64 = 5.0;

/// Earth radius in meters (standard value)
pub const EARTH_RADIUS_METERS: f64 = 6_371_000.0;