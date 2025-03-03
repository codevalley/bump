//! Configuration settings for the Bump service
//! Contains all tunable parameters that affect the matching algorithm and request handling

use serde::Deserialize;
use std::env;

/// Environment variable prefixes
const ENV_PREFIX: &str = "BUMP_";

/// Configuration for the matching service
#[derive(Debug, Clone, Deserialize)]
pub struct MatchingConfig {
    /// Maximum allowed distance between devices for a match (in meters)
    pub max_distance_meters: f64,
    
    /// Maximum allowed time difference between requests (in milliseconds)
    pub max_time_diff_ms: i64,
    
    /// Default TTL for requests if not specified (in milliseconds)
    pub default_ttl_ms: u32,
    
    /// Weight for temporal difference in match scoring (0.0 to 1.0)
    pub temporal_weight: f64,
    
    /// Weight for spatial difference in match scoring (0.0 to 1.0)
    pub spatial_weight: f64,
    
    /// How often to clean up expired requests (in milliseconds)
    pub cleanup_interval_ms: u64,
    
    /// Earth radius in meters (for distance calculations)
    pub earth_radius_meters: f64,

    /// Maximum number of requests in each queue (send/receive)
    pub max_queue_size: usize,
}

impl MatchingConfig {
    /// Loads configuration from environment variables
    pub fn from_env() -> Result<Self, String> {
        // Load .env file if it exists
        dotenv::dotenv().ok();

        // Create a map of environment variables
        let env_vars: std::collections::HashMap<String, String> = env::vars()
            .filter(|(k, _)| k.starts_with(ENV_PREFIX))
            .map(|(k, v)| (k.trim_start_matches(ENV_PREFIX).to_string(), v))
            .collect();

        // Try to parse environment variables
        match envy::from_iter::<_, Self>(env_vars.into_iter()) {
            Ok(config) => {
                config.validate()?;
                Ok(config)
            }
            Err(e) => Err(format!("Failed to parse environment variables: {}", e)),
        }
    }

    /// Loads configuration from environment variables or uses defaults
    pub fn from_env_or_default() -> Self {
        Self::from_env().unwrap_or_default()
    }
}

impl Default for MatchingConfig {
    fn default() -> Self {
        Self {
            max_distance_meters: 5.0,
            max_time_diff_ms: 500,
            default_ttl_ms: 500,
            temporal_weight: 0.7,
            spatial_weight: 0.3,
            cleanup_interval_ms: 1000,
            earth_radius_meters: 6_371_000.0,
            max_queue_size: 1000, // Default to 1000 requests per queue
        }
    }
}

impl MatchingConfig {
    /// Creates a new configuration with custom settings
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
            earth_radius_meters: 6_371_000.0, // This is a physical constant
            max_queue_size,
        }
    }

    /// Validates the configuration settings
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
