# Configuration Security Enhancement Setup Guide

This guide explains how to set up and use the enhanced configuration security features.

## Overview

The configuration security enhancement includes:

1. **Secret Management System** - Secure storage and rotation of secrets
2. **Environment-Specific Security Policies** - Different security settings per environment
3. **JWT Secret Rotation** - Automatic JWT secret rotation mechanism
4. **Encrypted Configuration Storage** - Encrypted storage for sensitive configuration
5. **Configuration Validation** - Comprehensive validation of configuration settings
6. **Production-Ready Templates** - Secure configuration templates

## Setup Instructions

### 1. Environment Variables

Set up the required environment variables:

```bash
# Secret Manager Key (32 bytes base64 encoded)
export SECRET_MANAGER_KEY="MTIzNDU2Nzg5MDEyMzQ1Njc4OTAxMjM0NTY3ODkwMTIzNDU2Nzg5"

# Configuration Encryption Key (32 bytes base64 encoded)
export CONFIG_ENCRYPTION_KEY="MTIzNDU2Nzg5MDEyMzQ1Njc4OTAxMjM0NTY3ODkwMTIzNDU2Nzg5"

# Database Password
export DB_PASSWORD="your_secure_database_password"

# Redis Password
export REDIS_PASSWORD="your_secure_redis_password"

# JWT Secret (will be managed by secret manager)
export JWT_SECRET="your_secure_jwt_secret"
```

### 2. Configuration Files

Copy the appropriate template to your configuration file:

```bash
# For production
cp config/production-template.toml config/production.toml

# For staging
cp config/staging-template.toml config/staging.toml

# For security settings
cp config/security-template.toml config/security.toml
```

### 3. Customize Configuration

Edit the configuration files to match your environment:

```toml
# config/production.toml
[database]
url = "postgresql://scheduler:${DB_PASSWORD}@your-db-host:5432/scheduler_prod?sslmode=require"

[api]
bind_address = "0.0.0.0:8443"
cors_enabled = false

[api.auth]
enabled = true
jwt_secret = "${JWT_SECRET}"
```

### 4. Initialize Secret Manager

The secret manager will automatically initialize when your application starts. It will:

- Load existing secrets from storage
- Validate secret integrity
- Set up encryption/decryption keys
- Prepare for secret rotation

### 5. Set Up JWT Secret Rotation

The JWT secret manager handles automatic rotation:

```rust
use scheduler_config::{SecretManager, JwtSecretManager, Environment};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize secret manager
    let secret_manager = Arc::new(SecretManager::new(Environment::Production).await?);
    
    // Initialize JWT secret manager
    let jwt_manager = JwtSecretManager::new(secret_manager, Environment::Production).await?;
    
    // Get current JWT secret (handles rotation automatically)
    let jwt_secret = jwt_manager.get_current_secret().await?;
    
    // Validate JWT tokens
    let validation = jwt_manager.validate_token_with_secrets("your.jwt.token").await?;
    
    Ok(())
}
```

### 6. Enable Encrypted Configuration Storage

Use encrypted configuration storage for sensitive data:

```rust
use scheduler_config::{EncryptedConfigStorage, Environment};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize encrypted storage
    let storage = EncryptedConfigStorage::new(Environment::Production)?;
    
    // Load encrypted configuration
    let config = storage.load_encrypted_config(Path::new("config/production.toml"))?;
    
    // Save configuration with encryption
    storage.save_encrypted_config(&config, Path::new("config/production.toml"))?;
    
    Ok(())
}
```

### 7. Validate Configuration

Use the comprehensive configuration validator:

```rust
use scheduler_config::{ConfigValidator, Environment};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize validator
    let validator = ConfigValidator::new(Environment::Production);
    
    // Validate configuration
    let result = validator.validate_config(&config);
    
    if !result.is_valid {
        eprintln!("Configuration validation failed:");
        for error in &result.errors {
            eprintln!("  ERROR: {}", error.message);
        }
        
        for warning in &result.warnings {
            eprintln!("  WARNING: {}", warning.message);
        }
        
        return Err("Invalid configuration".into());
    }
    
    println!("Configuration is valid (score: {}/100)", result.score.overall);
    Ok(())
}
```

## Security Policies

### Development Environment

- Authentication disabled
- CORS allowed for localhost
- Debug logging enabled
- Encryption disabled
- No secret rotation

### Testing Environment

- Authentication enabled
- CORS restricted to test domains
- Info logging enabled
- Encryption enabled
- Weekly secret rotation

### Staging Environment

- Authentication required
- CORS restricted to staging domains
- Warn logging enabled
- Encryption enabled
- Bi-weekly secret rotation

### Production Environment

- Authentication required
- CORS disabled
- Warn logging enabled
- Encryption enabled
- Monthly secret rotation
- TLS required
- Rate limiting enabled

## Secret Management

### Storing Secrets

```rust
use scheduler_config::{SecretManager, SecretType, Environment};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let secret_manager = SecretManager::new(Environment::Production).await?;
    
    // Store a secret
    secret_manager.store_secret(
        "database_password",
        "your_secure_password",
        SecretType::DatabasePassword,
        Some(90), // Expires in 90 days
    ).await?;
    
    Ok(())
}
```

### Retrieving Secrets

```rust
// Retrieve a secret
let password = secret_manager.get_secret("database_password").await?;
```

### Rotating Secrets

```rust
// Rotate a secret
let new_password = secret_manager.rotate_secret("database_password").await?;
```

## Configuration Validation

The validator checks for:

- **Required fields** - Ensures all necessary fields are present
- **Security violations** - Detects insecure settings
- **Environment mismatches** - Validates environment-specific settings
- **Performance issues** - Identifies performance-impacting configurations
- **Best practices** - Ensures following security best practices

### Validation Scores

- **Overall** (0-100) - General configuration quality
- **Security** (0-100) - Security-specific score
- **Performance** (0-100) - Performance impact score
- **Reliability** (0-100) - System reliability score
- **Maintainability** (0-100) - Configuration maintainability score

## Migration Guide

### From Plain Configuration

1. **Set up environment variables** for sensitive data
2. **Replace hardcoded secrets** with secret manager references
3. **Enable encryption** for configuration storage
4. **Update configuration files** to use secure templates
5. **Add validation** to configuration loading

### Example Migration

**Before:**

```toml
[database]
url = "postgresql://user:password123@localhost/db"
```

**After:**

```toml
[database]
url = "postgresql://scheduler:${DB_PASSWORD}@localhost/db?sslmode=require"
```

## Monitoring and Alerts

### Secret Rotation Alerts

- Secrets expiring soon
- Secret rotation failures
- Secret validation errors

### Configuration Validation Alerts

- Validation failures
- Security violations
- Environment mismatches

### Performance Alerts

- Configuration changes impacting performance
- Resource utilization issues
- Timeout or connection issues

## Best Practices

1. **Always use environment variables** for sensitive data
2. **Enable encryption** in production environments
3. **Rotate secrets regularly** (30-90 days)
4. **Validate configuration** on startup
5. **Use secure templates** for different environments
6. **Monitor security events** and set up alerts
7. **Keep secrets out of version control**
8. **Use strong, randomly generated secrets**
9. **Enable audit logging** for security events
10. **Regular security reviews** of configuration

## Troubleshooting

### Common Issues

**Secret Manager Not Initializing**

- Check SECRET_MANAGER_KEY environment variable
- Verify key is 32 bytes base64 encoded
- Ensure proper file permissions

**Configuration Validation Failing**

- Review error messages for specific issues
- Check environment-specific requirements
- Verify all required fields are present

**JWT Token Validation Failing**

- Check JWT secret is properly stored
- Verify token format and expiration
- Ensure secret rotation is working

**Encryption/Decryption Failing**

- Verify CONFIG_ENCRYPTION_KEY is set
- Check key format and length
- Ensure proper file permissions

### Debug Mode

For troubleshooting, you can enable debug mode:

```bash
export RUST_LOG=debug
export SCHEDULER_DEBUG=1
```

This will provide detailed logging of the configuration security systems.

## Support

For issues or questions:

1. Check the troubleshooting section
2. Review validation error messages
3. Consult the security policy documentation
4. Contact the security team for production issues
