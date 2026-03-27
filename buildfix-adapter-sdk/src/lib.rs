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
//! use buildfix_adapter_sdk::{Adapter, AdapterError, ReceiptBuilder};
//! use buildfix_types::receipt::{ReceiptEnvelope, Severity, VerdictStatus};
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
//!         // Convert to ReceiptEnvelope using ReceiptBuilder
//!         let envelope = ReceiptBuilder::new("my-sensor")
//!             .with_status(VerdictStatus::Fail)
//!             .build();
//!
//!         Ok(envelope)
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

pub use harness::{AdapterTestHarness, MetadataValidationError, ValidationResult};

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

/// Metadata trait for adapter self-description.
///
/// Adapters should implement this trait to provide information about
/// their capabilities and version compatibility. This enables runtime
/// discovery and validation of adapter compatibility with the buildfix
/// system.
///
/// # Example
///
/// ```
/// use buildfix_adapter_sdk::AdapterMetadata;
///
/// pub struct CargoDenyAdapter;
///
/// impl AdapterMetadata for CargoDenyAdapter {
///     fn name(&self) -> &str {
///         "cargo-deny"
///     }
///
///     fn version(&self) -> &str {
///         env!("CARGO_PKG_VERSION")
///     }
///
///     fn supported_schemas(&self) -> &[&str] {
///         &["cargo-deny.report.v1", "cargo-deny.report.v2"]
///     }
/// }
/// ```
pub trait AdapterMetadata {
    /// Returns the adapter name (e.g., "cargo-deny", "sarif").
    ///
    /// This should be a unique, stable identifier for the adapter type.
    /// Convention is to use kebab-case matching the sensor tool name.
    fn name(&self) -> &str;

    /// Returns the adapter version (e.g., env!("CARGO_PKG_VERSION")).
    ///
    /// This should return the semantic version of the adapter crate,
    /// typically using the `CARGO_PKG_VERSION` environment variable.
    fn version(&self) -> &str;

    /// Returns the list of schema versions this adapter supports.
    ///
    /// Format: "sensor.report.v1" style strings. This allows the system
    /// to validate that a receipt schema is compatible with this adapter.
    ///
    /// Adapters should list all schema versions they can successfully
    /// parse, enabling backward compatibility checks.
    fn supported_schemas(&self) -> &[&str];
}

/// Sealed trait marker for internal use.
///
/// This prevents external implementations of [`AdapterExt`] while allowing
/// blanket implementations for all types that meet the requirements.
mod sealed {
    pub trait Sealed {}
}

use sealed::Sealed;

impl<T: Adapter + AdapterMetadata> Sealed for T {}

/// Extension trait for adapters with metadata.
///
/// This trait provides additional functionality for adapters that implement
/// both [`Adapter`] and [`AdapterMetadata`]. It is automatically implemented
/// for all qualifying types via a blanket implementation.
///
/// # Example
///
/// ```ignore
/// use buildfix_adapter_sdk::{Adapter, AdapterMetadata, AdapterExt};
///
/// fn validate_adapter<A>(adapter: &A) -> bool
/// where
///     A: Adapter + AdapterMetadata,
/// {
///     // Check if adapter supports the required schema
///     adapter.supports_schema("cargo-deny.report.v1")
/// }
/// ```
pub trait AdapterExt: Adapter + AdapterMetadata + Sealed {
    /// Validates that this adapter supports the given schema.
    ///
    /// # Arguments
    ///
    /// * `schema` - The schema version string to check (e.g., "cargo-deny.report.v1")
    ///
    /// # Returns
    ///
    /// `true` if the schema is in the adapter's supported list, `false` otherwise.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let adapter = CargoDenyAdapter::new();
    /// assert!(adapter.supports_schema("cargo-deny.report.v1"));
    /// assert!(!adapter.supports_schema("unknown.schema.v1"));
    /// ```
    fn supports_schema(&self, schema: &str) -> bool {
        self.supported_schemas().contains(&schema)
    }
}

impl<T: Adapter + AdapterMetadata> AdapterExt for T {}
