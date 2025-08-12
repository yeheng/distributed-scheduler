use serde::{Deserialize, Serialize};
use crate::validation::{ConfigValidator, ValidationUtils};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub enabled: bool,
    pub jwt_secret: String,
    pub jwt_expiration_hours: u32,
    pub api_keys: std::collections::HashMap<String, ApiKeyConfig>,
}

impl Default for AuthConfig {
    fn default() -> Self {
        let mut api_keys = std::collections::HashMap::new();
        api_keys.insert(
            "default".to_string(),
            ApiKeyConfig {
                name: "Default API Key".to_string(),
                permissions: vec!["TaskRead".to_string(), "TaskWrite".to_string()],
                is_active: true,
            },
        );
        
        Self {
            enabled: false,
            jwt_secret: "your-secret-key-change-this-in-production".to_string(),
            jwt_expiration_hours: 24,
            api_keys,
        }
    }
}

impl ConfigValidator for AuthConfig {
    fn validate(&self) -> crate::ConfigResult<()> {
        if self.enabled {
            ValidationUtils::validate_not_empty(&self.jwt_secret, "api.auth.jwt_secret")?;
            
            if self.jwt_secret.len() < 32 {
                return Err(crate::ConfigError::Validation(
                    "api.auth.jwt_secret must be at least 32 characters long".to_string(),
                ));
            }
            
            if self.jwt_expiration_hours == 0 {
                return Err(crate::ConfigError::Validation(
                    "api.auth.jwt_expiration_hours must be greater than 0".to_string(),
                ));
            }
            
            if self.jwt_expiration_hours > 8760 {
                return Err(crate::ConfigError::Validation(
                    "api.auth.jwt_expiration_hours must be less than or equal to 8760 (1 year)".to_string(),
                ));
            }
            
            // Validate API keys
            for (key_name, key_config) in &self.api_keys {
                ValidationUtils::validate_not_empty(key_name, "api.auth.api_keys name")?;
                key_config.validate()?;
            }
        }
        
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyConfig {
    pub name: String,
    pub permissions: Vec<String>,
    pub is_active: bool,
}

impl ConfigValidator for ApiKeyConfig {
    fn validate(&self) -> crate::ConfigResult<()> {
        ValidationUtils::validate_not_empty(&self.name, "api.auth.api_keys.name")?;
        
        if self.permissions.is_empty() {
            return Err(crate::ConfigError::Validation(
                "api.auth.api_keys.permissions cannot be empty".to_string(),
            ));
        }
        
        // Validate permission names
        let valid_permissions = [
            "TaskRead", "TaskWrite", "TaskDelete",
            "WorkerRead", "WorkerWrite", "SystemRead", "SystemWrite", "Admin"
        ];
        
        for permission in &self.permissions {
            if !valid_permissions.contains(&permission.as_str()) {
                return Err(crate::ConfigError::Validation(
                    format!("Invalid permission: {permission}. Valid permissions: {valid_permissions:?}")
                ));
            }
        }
        
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitingConfig {
    pub enabled: bool,
    pub max_requests_per_minute: u32,
    pub refill_rate_per_second: f64,
    pub burst_size: u32,
    pub endpoint_limits: std::collections::HashMap<String, EndpointRateLimit>,
}

impl Default for RateLimitingConfig {
    fn default() -> Self {
        let mut endpoint_limits = std::collections::HashMap::new();
        endpoint_limits.insert(
            "/api/tasks".to_string(),
            EndpointRateLimit {
                max_requests_per_minute: 100,
                burst_size: 10,
            },
        );
        
        Self {
            enabled: false,
            max_requests_per_minute: 60,
            refill_rate_per_second: 1.0,
            burst_size: 10,
            endpoint_limits,
        }
    }
}

impl ConfigValidator for RateLimitingConfig {
    fn validate(&self) -> crate::ConfigResult<()> {
        if self.enabled {
            if self.max_requests_per_minute == 0 {
                return Err(crate::ConfigError::Validation(
                    "api.rate_limiting.max_requests_per_minute must be greater than 0".to_string(),
                ));
            }
            
            if self.refill_rate_per_second <= 0.0 {
                return Err(crate::ConfigError::Validation(
                    "api.rate_limiting.refill_rate_per_second must be greater than 0".to_string(),
                ));
            }
            
            if self.burst_size == 0 {
                return Err(crate::ConfigError::Validation(
                    "api.rate_limiting.burst_size must be greater than 0".to_string(),
                ));
            }
            
            if self.burst_size > self.max_requests_per_minute {
                return Err(crate::ConfigError::Validation(
                    "api.rate_limiting.burst_size must be less than or equal to max_requests_per_minute".to_string(),
                ));
            }
            
            // Validate endpoint limits
            for (endpoint, limit) in &self.endpoint_limits {
                ValidationUtils::validate_not_empty(endpoint, "api.rate_limiting.endpoint_limits endpoint")?;
                limit.validate()?;
            }
        }
        
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointRateLimit {
    pub max_requests_per_minute: u32,
    pub burst_size: u32,
}

impl ConfigValidator for EndpointRateLimit {
    fn validate(&self) -> crate::ConfigResult<()> {
        if self.max_requests_per_minute == 0 {
            return Err(crate::ConfigError::Validation(
                "endpoint_rate_limit.max_requests_per_minute must be greater than 0".to_string(),
            ));
        }
        
        if self.burst_size == 0 {
            return Err(crate::ConfigError::Validation(
                "endpoint_rate_limit.burst_size must be greater than 0".to_string(),
            ));
        }
        
        if self.burst_size > self.max_requests_per_minute {
            return Err(crate::ConfigError::Validation(
                "endpoint_rate_limit.burst_size must be less than or equal to max_requests_per_minute".to_string(),
            ));
        }
        
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    pub enabled: bool,
    pub bind_address: String,
    pub cors_enabled: bool,
    pub cors_origins: Vec<String>,
    pub request_timeout_seconds: u64,
    pub max_request_size_mb: u64,
    pub auth: AuthConfig,
    pub rate_limiting: RateLimitingConfig,
}

impl ConfigValidator for ApiConfig {
    fn validate(&self) -> crate::ConfigResult<()> {
        if self.enabled {
            ValidationUtils::validate_not_empty(&self.bind_address, "api.bind_address")?;
            
            // Validate bind address format (host:port)
            if !self.bind_address.contains(':') {
                return Err(crate::ConfigError::Validation(
                    "api.bind_address must be in format host:port".to_string(),
                ));
            }
            
            let parts: Vec<&str> = self.bind_address.split(':').collect();
            if parts.len() != 2 {
                return Err(crate::ConfigError::Validation(
                    "api.bind_address must be in format host:port".to_string(),
                ));
            }
            
            let port: u16 = parts[1].parse().map_err(|_| {
                crate::ConfigError::Validation(
                    "api.bind_address port must be a valid number".to_string(),
                )
            })?;
            ValidationUtils::validate_port(port)?;
            
            ValidationUtils::validate_timeout_seconds(self.request_timeout_seconds)?;
            
            if self.max_request_size_mb == 0 {
                return Err(crate::ConfigError::Validation(
                    "api.max_request_size_mb must be greater than 0".to_string(),
                ));
            }
            
            if self.max_request_size_mb > 100 {
                return Err(crate::ConfigError::Validation(
                    "api.max_request_size_mb must be less than or equal to 100".to_string(),
                ));
            }
            
            // Validate CORS origins
            if self.cors_enabled {
                if self.cors_origins.is_empty() {
                    return Err(crate::ConfigError::Validation(
                        "api.cors_origins cannot be empty when cors_enabled is true".to_string(),
                    ));
                }
                
                for origin in &self.cors_origins {
                    ValidationUtils::validate_not_empty(origin, "api.cors_origins")?;
                }
            }
            
            self.auth.validate()?;
            self.rate_limiting.validate()?;
        }
        
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservabilityConfig {
    pub tracing_enabled: bool,
    pub metrics_enabled: bool,
    pub metrics_endpoint: String,
    pub log_level: String,
    pub jaeger_endpoint: Option<String>,
}

impl ConfigValidator for ObservabilityConfig {
    fn validate(&self) -> crate::ConfigResult<()> {
        ValidationUtils::validate_not_empty(&self.metrics_endpoint, "observability.metrics_endpoint")?;
        ValidationUtils::validate_not_empty(&self.log_level, "observability.log_level")?;
        
        // Validate log level
        let valid_log_levels = ["trace", "debug", "info", "warn", "error"];
        if !valid_log_levels.contains(&self.log_level.as_str()) {
            return Err(crate::ConfigError::Validation(
                format!("Invalid log level: {}. Valid levels: {:?}", 
                        self.log_level, valid_log_levels)
            ));
        }
        
        // Validate Jaeger endpoint if provided
        if let Some(ref jaeger_endpoint) = self.jaeger_endpoint {
            ValidationUtils::validate_not_empty(jaeger_endpoint, "observability.jaeger_endpoint")?;
            
            if !jaeger_endpoint.starts_with("http://") && !jaeger_endpoint.starts_with("https://") {
                return Err(crate::ConfigError::Validation(
                    "observability.jaeger_endpoint must be a valid HTTP/HTTPS URL".to_string(),
                ));
            }
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_config_validation() {
        let config = AuthConfig::default();
        assert!(config.validate().is_ok());
        
        // Test enabled with short secret
        let mut invalid_config = config.clone();
        invalid_config.enabled = true;
        invalid_config.jwt_secret = "short".to_string();
        assert!(invalid_config.validate().is_err());
    }

    #[test]
    fn test_api_config_validation() {
        let config = ApiConfig {
            enabled: true,
            bind_address: "0.0.0.0:8080".to_string(),
            cors_enabled: true,
            cors_origins: vec!["*".to_string()],
            request_timeout_seconds: 30,
            max_request_size_mb: 10,
            auth: AuthConfig::default(),
            rate_limiting: RateLimitingConfig::default(),
        };
        
        assert!(config.validate().is_ok());
        
        // Test invalid bind address
        let mut invalid_config = config.clone();
        invalid_config.bind_address = "invalid".to_string();
        assert!(invalid_config.validate().is_err());
    }

    #[test]
    fn test_observability_config_validation() {
        let config = ObservabilityConfig {
            tracing_enabled: true,
            metrics_enabled: true,
            metrics_endpoint: "/metrics".to_string(),
            log_level: "info".to_string(),
            jaeger_endpoint: None,
        };
        
        assert!(config.validate().is_ok());
        
        // Test invalid log level
        let mut invalid_config = config.clone();
        invalid_config.log_level = "invalid".to_string();
        assert!(invalid_config.validate().is_err());
    }
}