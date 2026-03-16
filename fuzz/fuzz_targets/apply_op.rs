#![no_main]

use libfuzzer_sys::fuzz_target;
use buildfix_edit::apply_op_to_content;
use buildfix_types::ops::OpKind;

fuzz_target!(|data: &[u8]| {
    // Fuzz: try to parse arbitrary bytes as UTF-8 TOML and apply a no-op-ish operation.
    let Ok(s) = std::str::from_utf8(data) else { return };

    let op = OpKind::TomlTransform {
        rule_id: "ensure_workspace_resolver_v2".to_string(),
        args: None,
    };

    let _ = s.parse::<toml_edit::DocumentMut>().map(|d| d.to_string());
    let _ = apply_op_to_content(s, &op);
});
