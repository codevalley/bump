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
            Ok(mut config) => {
                // Validate earth_radius_meters before setting default
                if config.earth_radius_meters < 0.0 {
                    return Err("earth_radius_meters must be positive".to_string());
                }
                // Set earth_radius_meters to default if not provided
                if config.earth_radius_meters == 0.0 {
                    config.earth_radius_meters = Self::default().earth_radius_meters;
                }
                
                // Early validation for critical parameters
                if config.max_distance_meters <= 0.0 {
                    return Err("max_distance_meters must be positive".to_string());
                }
                
                // Validate the rest of the config
                config.validate()?;
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
            max_distance_meters: 250.0,   // 250 meter radius
            max_time_diff_ms: 1500,       // 1500ms window (increased for server-assigned timestamps)
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
    #[allow(dead_code)]
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
        if self.earth_radius_meters <= 0.0 {
            return Err("earth_radius_meters must be positive".to_string());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    /// Helper function to clear environment variables after testing
    fn clear_test_env_vars() {
        env::remove_var("BUMP_MAX_DISTANCE_METERS");
        env::remove_var("BUMP_MAX_TIME_DIFF_MS");
        env::remove_var("BUMP_DEFAULT_TTL_MS");
        env::remove_var("BUMP_TEMPORAL_WEIGHT");
        env::remove_var("BUMP_SPATIAL_WEIGHT");
        env::remove_var("BUMP_CLEANUP_INTERVAL_MS");
        env::remove_var("BUMP_MAX_QUEUE_SIZE");
        env::remove_var("BUMP_EARTH_RADIUS_METERS");
    }

    #[test]
    fn test_default_config() {
        let config = MatchingConfig::default();
        assert_eq!(config.max_distance_meters, 5.0);
        assert_eq!(config.max_time_diff_ms, 1500);
        assert_eq!(config.default_ttl_ms, 500);
        assert_eq!(config.temporal_weight, 0.7);
        assert_eq!(config.spatial_weight, 0.3);
        assert_eq!(config.cleanup_interval_ms, 1000);
        assert_eq!(config.max_queue_size, 1000);
    }

    #[test]
    fn test_new_config() {
        let config = MatchingConfig::new(
            10.0,   // max_distance_meters
            1000,   // max_time_diff_ms
            800,    // default_ttl_ms
            0.6,    // temporal_weight
            0.4,    // spatial_weight
            2000,   // cleanup_interval_ms
            2000,   // max_queue_size
        );

        assert_eq!(config.max_distance_meters, 10.0);
        assert_eq!(config.max_time_diff_ms, 1000);
        assert_eq!(config.default_ttl_ms, 800);
        assert_eq!(config.temporal_weight, 0.6);
        assert_eq!(config.spatial_weight, 0.4);
        assert_eq!(config.cleanup_interval_ms, 2000);
        assert_eq!(config.max_queue_size, 2000);
        assert_eq!(config.earth_radius_meters, 6_371_000.0);
    }

    #[test]
    fn test_config_validation() {
        // Test valid configuration
        let valid_config = MatchingConfig::default();
        assert!(valid_config.validate().is_ok());

        // Test invalid max_distance_meters
        let mut invalid_config = MatchingConfig::default();
        invalid_config.max_distance_meters = 0.0;
        assert!(invalid_config.validate().is_err());

        // Test invalid weights
        let mut invalid_config = MatchingConfig::default();
        invalid_config.temporal_weight = 0.8;
        invalid_config.spatial_weight = 0.3;
        assert!(invalid_config.validate().is_err());

        // Test negative weights
        let mut invalid_config = MatchingConfig::default();
        invalid_config.temporal_weight = -0.1;
        assert!(invalid_config.validate().is_err());
    }

    #[test]
    #[serial_test::serial]
    fn test_from_env() {
        // First clear any existing environment variables
        clear_test_env_vars();
        
        // Set our own specific test environment variables
        env::set_var("BUMP_MAX_DISTANCE_METERS", "10.0");
        env::set_var("BUMP_MAX_TIME_DIFF_MS", "1000");
        env::set_var("BUMP_DEFAULT_TTL_MS", "800");
        env::set_var("BUMP_TEMPORAL_WEIGHT", "0.6");
        env::set_var("BUMP_SPATIAL_WEIGHT", "0.4");
        env::set_var("BUMP_CLEANUP_INTERVAL_MS", "2000");
        env::set_var("BUMP_MAX_QUEUE_SIZE", "2000");
        env::set_var("BUMP_EARTH_RADIUS_METERS", "6371000.0");
        
        // Verify environment variable was set correctly
        let distance_var = env::var("BUMP_MAX_DISTANCE_METERS").unwrap();
        assert_eq!(distance_var, "10.0", "Environment variable was not set correctly");
        
        // Now load from environment
        let config = match MatchingConfig::from_env() {
            Ok(c) => c,
            Err(e) => {
                println!("Failed to load config: {}", e);
                panic!("Config failed: {}", e);
            }
        };
        assert_eq!(config.max_distance_meters, 10.0);
        assert_eq!(config.max_time_diff_ms, 1000);
        assert_eq!(config.default_ttl_ms, 800);
        assert_eq!(config.temporal_weight, 0.6);
        assert_eq!(config.spatial_weight, 0.4);
        assert_eq!(config.cleanup_interval_ms, 2000);
        assert_eq!(config.max_queue_size, 2000);
        assert_eq!(config.earth_radius_meters, 6371000.0);
        clear_test_env_vars();

        // Test with missing environment variables
        let config = MatchingConfig::from_env_or_default();
        assert_eq!(config.max_distance_meters, 5.0); // Should use default values
    }

    #[test]
    #[serial_test::serial]
    fn test_from_env_invalid_values() {
        // Clear any existing environment variables
        clear_test_env_vars();
        
        // Test with invalid environment variables that will fail validation
        env::set_var("BUMP_MAX_DISTANCE_METERS", "-1.0");
        env::set_var("BUMP_MAX_TIME_DIFF_MS", "1000");
        env::set_var("BUMP_DEFAULT_TTL_MS", "800");
        env::set_var("BUMP_TEMPORAL_WEIGHT", "0.6");
        env::set_var("BUMP_SPATIAL_WEIGHT", "0.4");
        env::set_var("BUMP_CLEANUP_INTERVAL_MS", "2000");
        env::set_var("BUMP_MAX_QUEUE_SIZE", "2000");
        env::set_var("BUMP_EARTH_RADIUS_METERS", "6371000.0");
        
        // Verify the environment variable is set correctly
        let distance_var = env::var("BUMP_MAX_DISTANCE_METERS").unwrap();
        assert_eq!(distance_var, "-1.0", "Environment variable was not set correctly");
        
        let config = MatchingConfig::from_env();
        
        // Verify the configuration failed because of the negative distance
        assert!(config.is_err(), "Configuration validation should fail with negative distance");
        assert!(config.unwrap_err().contains("max_distance_meters must be positive"));
        
        // Clean up environment variables
        clear_test_env_vars();
    }
}
