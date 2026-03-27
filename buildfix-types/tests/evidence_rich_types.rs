use buildfix_types::receipt::{
    AnalysisDepth, Evidence, Finding, FindingContext, Provenance, WorkspaceContext,
};

#[test]
fn provenance_default_has_empty_values() {
    let provenance = Provenance::default();
    assert!(provenance.method.is_empty());
    assert!(provenance.tools.is_empty());
    assert!(!provenance.agreement);
    assert!(provenance.evidence_chain.is_empty());
}

#[test]
fn provenance_serializes_with_skip_empty() {
    let provenance = Provenance {
        method: "dead_code_analysis".to_string(),
        tools: vec!["cargo-machete".to_string()],
        agreement: true,
        evidence_chain: Vec::new(),
    };

    let value = serde_json::to_value(&provenance).expect("serialize provenance");
    assert_eq!(value["method"], "dead_code_analysis");
    assert_eq!(value["tools"], serde_json::json!(["cargo-machete"]));
    assert_eq!(value["agreement"], true);
    // evidence_chain should be skipped when empty
    assert!(!value.as_object().unwrap().contains_key("evidence_chain"));
}

#[test]
fn evidence_default_has_null_value() {
    let evidence = Evidence::default();
    assert!(evidence.source.is_empty());
    assert!(evidence.value.is_null());
    assert!(!evidence.validated);
}

#[test]
fn evidence_serializes_correctly() {
    let evidence = Evidence {
        source: "lockfile".to_string(),
        value: serde_json::json!("1.2.3"),
        validated: true,
    };

    let value = serde_json::to_value(&evidence).expect("serialize evidence");
    assert_eq!(value["source"], "lockfile");
    assert_eq!(value["value"], "1.2.3");
    assert_eq!(value["validated"], true);
}

#[test]
fn finding_context_default_has_none_values() {
    let ctx = FindingContext::default();
    assert!(ctx.workspace.is_none());
    assert!(ctx.analysis_depth.is_none());
}

#[test]
fn workspace_context_default_values() {
    let ws = WorkspaceContext::default();
    assert!(ws.consensus_value.is_none());
    assert_eq!(ws.consensus_count, 0);
    assert_eq!(ws.total_crates, 0);
    assert!(ws.outliers.is_empty());
    assert!(ws.outlier_crates.is_empty());
    assert!(!ws.all_crates_agree);
}

#[test]
fn workspace_context_serializes_with_skip_empty() {
    let ws = WorkspaceContext {
        consensus_value: Some(serde_json::json!("2021")),
        consensus_count: 5,
        total_crates: 5,
        outliers: Vec::new(),
        outlier_crates: Vec::new(),
        all_crates_agree: true,
    };

    let value = serde_json::to_value(&ws).expect("serialize workspace context");
    assert_eq!(value["consensus_value"], "2021");
    assert_eq!(value["consensus_count"], 5);
    assert_eq!(value["total_crates"], 5);
    assert_eq!(value["all_crates_agree"], true);
    // Empty arrays should be skipped
    assert!(!value.as_object().unwrap().contains_key("outliers"));
    assert!(!value.as_object().unwrap().contains_key("outlier_crates"));
}

#[test]
fn analysis_depth_default_is_full() {
    assert_eq!(AnalysisDepth::default(), AnalysisDepth::Full);
}

#[test]
fn analysis_depth_serializes_snake_case() {
    let shallow = AnalysisDepth::Shallow;
    let full = AnalysisDepth::Full;
    let deep = AnalysisDepth::Deep;

    assert_eq!(
        serde_json::to_value(shallow).unwrap(),
        serde_json::json!("shallow")
    );
    assert_eq!(
        serde_json::to_value(full).unwrap(),
        serde_json::json!("full")
    );
    assert_eq!(
        serde_json::to_value(deep).unwrap(),
        serde_json::json!("deep")
    );
}

#[test]
fn analysis_depth_deserializes_snake_case() {
    let shallow: AnalysisDepth = serde_json::from_value(serde_json::json!("shallow")).unwrap();
    let full: AnalysisDepth = serde_json::from_value(serde_json::json!("full")).unwrap();
    let deep: AnalysisDepth = serde_json::from_value(serde_json::json!("deep")).unwrap();

    assert_eq!(shallow, AnalysisDepth::Shallow);
    assert_eq!(full, AnalysisDepth::Full);
    assert_eq!(deep, AnalysisDepth::Deep);
}

#[test]
fn finding_is_high_confidence_returns_false_when_none() {
    let finding = Finding {
        confidence: None,
        ..Default::default()
    };
    assert!(!finding.is_high_confidence());
}

#[test]
fn finding_is_high_confidence_returns_true_at_threshold() {
    let finding = Finding {
        confidence: Some(0.9),
        ..Default::default()
    };
    assert!(finding.is_high_confidence());
}

#[test]
fn finding_is_high_confidence_returns_true_above_threshold() {
    let finding = Finding {
        confidence: Some(0.95),
        ..Default::default()
    };
    assert!(finding.is_high_confidence());
}

#[test]
fn finding_is_high_confidence_returns_false_below_threshold() {
    let finding = Finding {
        confidence: Some(0.89),
        ..Default::default()
    };
    assert!(!finding.is_high_confidence());
}

#[test]
fn finding_has_full_consensus_returns_false_when_no_context() {
    let finding = Finding {
        context: None,
        ..Default::default()
    };
    assert!(!finding.has_full_consensus());
}

#[test]
fn finding_has_full_consensus_returns_false_when_no_workspace() {
    let finding = Finding {
        context: Some(FindingContext {
            workspace: None,
            analysis_depth: None,
        }),
        ..Default::default()
    };
    assert!(!finding.has_full_consensus());
}

#[test]
fn finding_has_full_consensus_returns_true_when_all_agree() {
    let finding = Finding {
        context: Some(FindingContext {
            workspace: Some(WorkspaceContext {
                all_crates_agree: true,
                ..Default::default()
            }),
            analysis_depth: None,
        }),
        ..Default::default()
    };
    assert!(finding.has_full_consensus());
}

#[test]
fn finding_has_full_consensus_returns_false_when_not_all_agree() {
    let finding = Finding {
        context: Some(FindingContext {
            workspace: Some(WorkspaceContext {
                all_crates_agree: false,
                ..Default::default()
            }),
            analysis_depth: None,
        }),
        ..Default::default()
    };
    assert!(!finding.has_full_consensus());
}

#[test]
fn finding_has_tool_agreement_returns_false_when_no_provenance() {
    let finding = Finding {
        provenance: None,
        ..Default::default()
    };
    assert!(!finding.has_tool_agreement());
}

#[test]
fn finding_has_tool_agreement_returns_true_when_agreement() {
    let finding = Finding {
        provenance: Some(Provenance {
            agreement: true,
            ..Default::default()
        }),
        ..Default::default()
    };
    assert!(finding.has_tool_agreement());
}

#[test]
fn finding_has_tool_agreement_returns_false_when_no_agreement() {
    let finding = Finding {
        provenance: Some(Provenance {
            agreement: false,
            ..Default::default()
        }),
        ..Default::default()
    };
    assert!(!finding.has_tool_agreement());
}

#[test]
fn finding_serializes_with_skip_none() {
    let finding = Finding {
        confidence: Some(0.95),
        provenance: None,
        context: None,
        ..Default::default()
    };

    let value = serde_json::to_value(&finding).expect("serialize finding");
    assert_eq!(value["confidence"], 0.95);
    // Optional fields should be skipped when None
    assert!(!value.as_object().unwrap().contains_key("provenance"));
    assert!(!value.as_object().unwrap().contains_key("context"));
}

#[test]
fn finding_with_all_evidence_fields_serializes_correctly() {
    let finding = Finding {
        confidence: Some(1.0),
        provenance: Some(Provenance {
            method: "license_detection".to_string(),
            tools: vec!["cargo-deny".to_string(), "cargo-about".to_string()],
            agreement: true,
            evidence_chain: vec![Evidence {
                source: "Cargo.toml".to_string(),
                value: serde_json::json!("MIT"),
                validated: true,
            }],
        }),
        context: Some(FindingContext {
            workspace: Some(WorkspaceContext {
                consensus_value: Some(serde_json::json!("MIT")),
                consensus_count: 10,
                total_crates: 10,
                outliers: Vec::new(),
                outlier_crates: Vec::new(),
                all_crates_agree: true,
            }),
            analysis_depth: Some(AnalysisDepth::Deep),
        }),
        ..Default::default()
    };

    let value = serde_json::to_value(&finding).expect("serialize finding");
    assert_eq!(value["confidence"], 1.0);
    assert_eq!(value["provenance"]["method"], "license_detection");
    assert_eq!(
        value["provenance"]["tools"],
        serde_json::json!(["cargo-deny", "cargo-about"])
    );
    assert_eq!(value["context"]["workspace"]["consensus_value"], "MIT");
    assert_eq!(value["context"]["analysis_depth"], "deep");

    // Verify helper methods work
    assert!(finding.is_high_confidence());
    assert!(finding.has_full_consensus());
    assert!(finding.has_tool_agreement());
}
