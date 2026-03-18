//! SDK for building intake adapters that convert sensor outputs to buildfix receipts.
//!
//! This crate provides the `Adapter` trait and utilities for implementing new sensor
//! intake adapters. An adapter transforms a sensor's native output format into the
//! standardized `ReceiptEnvelope` format that buildfix expects.
//!
//! # Creating a New Adapter
//!
//! To create a new adapter, implement the `Adapter` trait for your sensor-specific
//! adapter struct. The adapter is responsible for:
//!
//! 1. **Identifying the sensor** via `sensor_id()` - returns a unique string like
//!    `"cargo-deny"` or `"clippy"`
//!
//! 2. **Loading sensor output** via `load()` - reads and parses the sensor's
//!    output file into a `ReceiptEnvelope`
//!
//! ## Example
//!
//! ```ignore
//! use buildfix_adapter_sdk::{Adapter, AdapterError};
//! use buildfix_types::receipt::ReceiptEnvelope;
//! use std::path::Path;
//!
//! pub struct MySensorAdapter {
//!     sensor_id: String,
//! }
//!
//! impl MySensorAdapter {
//!     pub fn new() -> Self {
//!         Self {
//!             sensor_id: "my-sensor".to_string(),
//!         }
//!     }
//! }
//!
//! impl Adapter for MySensorAdapter {
//!     fn sensor_id(&self) -> &str {
//!         &self.sensor_id
//!     }
//!
//!     fn load(&self, path: &Path) -> Result<ReceiptEnvelope, AdapterError> {
//!         // Parse your sensor's output format and convert to ReceiptEnvelope
//!         let output = std::fs::read_to_string(path)
//!             .map_err(AdapterError::Io)?;
//!         
//!         let parsed = serde_json::from_str::<serde_json::Value>(&output)
//!             .map_err(AdapterError::Json)?;
//!
//!         // Convert to ReceiptEnvelope...
//!         # todo!()
//!     }
//! }
//! ```
//!
//! # Testing Adapters
//!
//! Use `AdapterTestHarness` to validate your adapter implementation:
//!
//! ```ignore
//! use buildfix_adapter_sdk::AdapterTestHarness;
//! use my_adapter::MySensorAdapter;
//!
//! #[test]
//! fn test_adapter_loads_receipt() {
//!     let harness = AdapterTestHarness::new(MySensorAdapter::new());
//!     harness.validate_receipt_fixture("tests/fixtures/my-sensor/report.json")
//!         .expect("receipt should load correctly");
//! }
//! ```

pub mod receipt_builder;

pub use receipt_builder::ReceiptBuilder;

mod harness;

pub use harness::{AdapterTestHarness, ValidationResult};

use buildfix_types::receipt::ReceiptEnvelope;
use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AdapterError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Invalid sensor output: {0}")]
    InvalidFormat(String),

    #[error("Required field missing: {0}")]
    MissingField(String),
}

pub trait Adapter: Send + Sync {
    fn sensor_id(&self) -> &str;

    fn load(&self, path: &Path) -> Result<ReceiptEnvelope, AdapterError>;
}

impl<T: Adapter + ?Sized> Adapter for &T {
    fn sensor_id(&self) -> &str {
        (*self).sensor_id()
    }

    fn load(&self, path: &Path) -> Result<ReceiptEnvelope, AdapterError> {
        (*self).load(path)
    }
}

impl<T: Adapter + ?Sized> Adapter for Box<T> {
    fn sensor_id(&self) -> &str {
        (**self).sensor_id()
    }

    fn load(&self, path: &Path) -> Result<ReceiptEnvelope, AdapterError> {
        (**self).load(path)
    }
}
