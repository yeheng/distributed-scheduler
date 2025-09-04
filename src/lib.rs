pub mod app;
pub mod common;
pub mod shutdown;
pub mod embedded;

#[cfg(test)]
pub mod embedded_api_integration_tests;
#[cfg(test)]
pub mod embedded_zero_config_tests;
#[cfg(test)]
pub mod embedded_integration_tests;
#[cfg(test)]
pub mod embedded_basic_tests;
#[cfg(test)]
pub mod embedded_shutdown_test;