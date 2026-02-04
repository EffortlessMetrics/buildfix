#![no_main]

//! Fuzz target for plan.json parsing.
//!
//! This fuzzes the `BuildfixPlan` deserialization with arbitrary JSON bytes
//! to ensure the parser handles malformed input gracefully.

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Try to parse arbitrary bytes as UTF-8 JSON into a BuildfixPlan.
    let Ok(s) = std::str::from_utf8(data) else {
        return;
    };

    // Attempt to parse as BuildfixPlan - should never panic.
    let result = serde_json::from_str::<buildfix_types::plan::BuildfixPlan>(s);

    // If parsing succeeded, try serializing back to JSON.
    if let Ok(plan) = result {
        let _ = serde_json::to_string(&plan);
        let _ = serde_json::to_string_pretty(&plan);
    }

    // Also try parsing individual plan components.
    let _ = serde_json::from_str::<buildfix_types::plan::PlanPolicy>(s);
    let _ = serde_json::from_str::<buildfix_types::plan::PlanInput>(s);
    let _ = serde_json::from_str::<buildfix_types::plan::PlanSummary>(s);
    let _ = serde_json::from_str::<buildfix_types::plan::PlanOp>(s);
    let _ = serde_json::from_str::<buildfix_types::plan::FindingRef>(s);
    let _ = serde_json::from_str::<buildfix_types::plan::FilePrecondition>(s);

    // Also try parsing operations.
    let _ = serde_json::from_str::<buildfix_types::ops::OpKind>(s);
    let _ = serde_json::from_str::<Vec<buildfix_types::ops::OpKind>>(s);

    // Try parsing as generic JSON first, then attempting typed deserialization.
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(s) {
        let _ = serde_json::from_value::<buildfix_types::plan::BuildfixPlan>(val.clone());
        let _ = serde_json::from_value::<Vec<buildfix_types::plan::PlanOp>>(val);
    }
});
