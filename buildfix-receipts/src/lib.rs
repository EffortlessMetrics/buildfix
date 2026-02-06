//! Receipt ingestion utilities.
//!
//! buildfix consumes receipts produced by other tools. It intentionally does not enforce strict schema
//! validation here; the director/conformance harness should do that. buildfix is tolerant so it can
//! still plan fixes when a receipt contains extra fields or misses optional fields.

mod load;

pub use load::{LoadedReceipt, ReceiptLoadError, load_receipts};
