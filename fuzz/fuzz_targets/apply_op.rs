#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Fuzz: try to parse arbitrary bytes as UTF-8 TOML and apply a no-op-ish operation.
    let Ok(s) = std::str::from_utf8(data) else { return };

    let op = buildfix_types::ops::Operation::EnsureWorkspaceResolverV2 {
        manifest: "Cargo.toml".into(),
    };

    // apply_op_to_content is not public; fuzz basic parsing and formatting via toml_edit.
    let _ = s.parse::<toml_edit::DocumentMut>().map(|d| d.to_string());
    let _ = op;
});
