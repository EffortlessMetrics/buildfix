#![no_main]

//! Fuzz target for receipt JSON parsing.
//!
//! This fuzzes the `ReceiptEnvelope` deserialization with arbitrary JSON bytes
//! to ensure the parser handles malformed input gracefully.

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Try to parse arbitrary bytes as UTF-8 JSON into a ReceiptEnvelope.
    let Ok(s) = std::str::from_utf8(data) else {
        return;
    };

    // Attempt to parse as ReceiptEnvelope - should never panic.
    let _ = serde_json::from_str::<buildfix_types::receipt::ReceiptEnvelope>(s);

    // Also try parsing individual receipt components.
    let _ = serde_json::from_str::<buildfix_types::receipt::ToolInfo>(s);
    let _ = serde_json::from_str::<buildfix_types::receipt::RunInfo>(s);
    let _ = serde_json::from_str::<buildfix_types::receipt::Verdict>(s);
    let _ = serde_json::from_str::<buildfix_types::receipt::Finding>(s);
    let _ = serde_json::from_str::<buildfix_types::receipt::Location>(s);

    // Try parsing as a generic JSON value first, then attempting re-serialization.
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(s) {
        // If we can parse it as generic JSON, try to deserialize typed.
        let _ = serde_json::from_value::<buildfix_types::receipt::ReceiptEnvelope>(val);
    }
});
