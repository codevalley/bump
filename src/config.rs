//! Configuration management for the Bump service.
//!
//! This module provides configuration handling via environment variables with sensible defaults.
//! All configuration parameters can be customized through environment variables with the BUMP_ prefix.
//!
//! # Environment Variables
//! - BUMP_MAX_DISTANCE_METERS: Maximum distance for matching (default: 5.0)
//! - BUMP_MAX_TIME_DIFF_MS: Maximum time difference (default: 500)
//! - BUMP_DEFAULT_TTL_MS: Default request TTL (default: 500)
//! - BUMP_TEMPORAL_WEIGHT: Weight for time-based matching (default: 0.7)
//! - BUMP_SPATIAL_WEIGHT: Weight for location-based matching (default: 0.3)
//! - BUMP_CLEANUP_INTERVAL_MS: Queue cleanup interval (default: 1000)
//! - BUMP_MAX_QUEUE_SIZE: Maximum requests per queue (default: 1000)

use serde::Deserialize;
use std::env;

/// Prefix for all Bump service environment variables.
/// All config env vars must start with "BUMP_".
const ENV_PREFIX: &str = "BUMP_";

/// Configuration parameters for the Bump matching service.
/// 
/// This struct holds all configurable parameters that affect:
/// - Request matching behavior
/// - Queue management
/// - Performance characteristics
/// - Resource limits
#[derive(Debug, Clone, Deserialize)]
pub struct MatchingConfig {
    /// Maximum allowed distance between devices for a match.
    /// Must be positive. Specified in meters.
    pub max_distance_meters: f64,
    
    /// Maximum allowed time difference between requests.
    /// Must be positive. Specified in milliseconds.
    pub max_time_diff_ms: i64,
    
    /// Default time-to-live for requests if not specified by client.
    /// Must be positive. Specified in milliseconds.
    pub default_ttl_ms: u32,
    
    /// Weight given to temporal proximity in match scoring.
    /// Must be between 0.0 and 1.0, and sum with spatial_weight to 1.0.
    pub temporal_weight: f64,
    
    /// Weight given to spatial proximity in match scoring.
    /// Must be between 0.0 and 1.0, and sum with temporal_weight to 1.0.
    pub spatial_weight: f64,
    
    /// Interval at which expired requests are removed from queues.
    /// Must be positive. Specified in milliseconds.
    pub cleanup_interval_ms: u64,
    
    /// Earth's radius used in geospatial distance calculations.
    /// Constant value in meters. Do not modify unless needed for testing.
    pub earth_radius_meters: f64,

    /// Maximum number of requests allowed in each queue (send/receive).
    /// Must be positive. Prevents memory exhaustion under high load.
    pub max_queue_size: usize,
}

impl MatchingConfig {
    /// Attempts to load configuration from environment variables.
    /// 
    /// # Environment Variables
    /// All variables must be prefixed with "BUMP_". For example:
    /// - BUMP_MAX_DISTANCE_METERS=10.0
    /// - BUMP_MAX_TIME_DIFF_MS=1000
    /// 
    /// # Returns
    /// - Ok(config) if all required variables are present and valid
    /// - Err(message) if any variables are missing or invalid
    pub fn from_env() -> Result<Self, String> {
        // Load .env file if it exists for local development
        dotenv::dotenv().ok();

        // Filter and transform environment variables
        let env_vars: std::collections::HashMap<String, String> = env::vars()
            .filter(|(k, _)| k.starts_with(ENV_PREFIX))
            .map(|(k, v)| (k.trim_start_matches(ENV_PREFIX).to_string(), v))
            .collect();

        // Parse and validate configuration
        match envy::from_iter::<_, Self>(env_vars.into_iter()) {
            Ok(config) => {
                config.validate()?
                Ok(config)
            }
            Err(e) => Err(format!("Failed to parse environment variables: {}", e)),
        }
    }

    /// Loads configuration from environment variables, falling back to defaults
    /// if environment variables are not set or are invalid.
    pub fn from_env_or_default() -> Self {
        Self::from_env().unwrap_or_default()
    }
}

/// Default configuration values optimized for typical use cases.
impl Default for MatchingConfig {
    fn default() -> Self {
        Self {
            max_distance_meters: 5.0,     // 5 meter radius
            max_time_diff_ms: 500,        // 500ms window
            default_ttl_ms: 500,          // 500ms TTL
            temporal_weight: 0.7,         // Prioritize temporal matching
            spatial_weight: 0.3,          // Secondary spatial matching
            cleanup_interval_ms: 1000,    // Clean every second
            earth_radius_meters: 6_371_000.0, // Standard Earth radius
            max_queue_size: 1000,         // 1000 requests per queue
        }
    }
}

impl MatchingConfig {
    /// Creates a new configuration with custom settings.
    /// 
    /// # Arguments
    /// * `max_distance_meters` - Maximum matching distance in meters
    /// * `max_time_diff_ms` - Maximum time difference in milliseconds
    /// * `default_ttl_ms` - Default request TTL in milliseconds
    /// * `temporal_weight` - Weight for temporal matching (0.0-1.0)
    /// * `spatial_weight` - Weight for spatial matching (0.0-1.0)
    /// * `cleanup_interval_ms` - Queue cleanup interval in milliseconds
    /// * `max_queue_size` - Maximum requests per queue
    /// 
    /// Earth radius is set to the standard value and cannot be modified
    /// through this constructor to prevent errors in distance calculations.
    pub fn new(
        max_distance_meters: f64,
        max_time_diff_ms: i64,
        default_ttl_ms: u32,
        temporal_weight: f64,
        spatial_weight: f64,
        cleanup_interval_ms: u64,
        max_queue_size: usize,
    ) -> Self {
        Self {
            max_distance_meters,
            max_time_diff_ms,
            default_ttl_ms,
            temporal_weight,
            spatial_weight,
            cleanup_interval_ms,
            earth_radius_meters: 6_371_000.0, // Standard Earth radius
            max_queue_size,
        }
    }

    /// Validates all configuration parameters to ensure they are within
    /// acceptable ranges and maintain required invariants.
    /// 
    /// # Validation Rules
    /// - All numeric values must be positive
    /// - Weights must be non-negative and sum to 1.0
    /// - Queue size must be positive
    /// 
    /// # Returns
    /// - Ok(()) if all validation passes
    /// - Err(message) with description of the first validation failure
    pub fn validate(&self) -> Result<(), String> {
        if self.max_distance_meters <= 0.0 {
            return Err("max_distance_meters must be positive".to_string());
        }
        if self.max_time_diff_ms <= 0 {
            return Err("max_time_diff_ms must be positive".to_string());
        }
        if self.default_ttl_ms == 0 {
            return Err("default_ttl_ms must be positive".to_string());
        }
        if self.temporal_weight < 0.0 || self.spatial_weight < 0.0 {
            return Err("weights must be non-negative".to_string());
        }
        if (self.temporal_weight + self.spatial_weight - 1.0).abs() > f64::EPSILON {
            return Err("weights must sum to 1.0".to_string());
        }
        if self.cleanup_interval_ms == 0 {
            return Err("cleanup_interval_ms must be positive".to_string());
        }
        if self.max_queue_size == 0 {
            return Err("max_queue_size must be positive".to_string());
        }
        Ok(())
    }
}
