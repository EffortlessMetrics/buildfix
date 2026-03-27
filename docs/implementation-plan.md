# buildfix — Implementation Plan

> **Post-v0.2.0 Milestone-Based Plan**
> 
> The bootstrap release (v0.2.0) is complete. This plan shifts from bootstrap phases to release-shaped milestones focused on operational hardening and productization.
>
> For strategic context, see [ROADMAP.md](../ROADMAP.md).

---

## Milestone Overview

| Version | Theme | Status |
|---------|-------|--------|
| v0.2.1 | Operational Hardening | In Progress |
| v0.3.0 | Adapter Productization | Substantially Complete |
| v0.4.0 | Evidence-Rich Unsafe Reduction | Planned |

---

## v0.2.1 — Operational Hardening

### Objective

Make releases boring and documentation trustworthy. Establish the operational foundation for sustainable productization.

### Dependencies

- **Prerequisite**: v0.2.0 bootstrap release completed ✅

### Tasks

#### Release Automation

- [x] Re-enable post-bootstrap release automation
  - [x] Enable tag-triggered `publish.yml` workflow
  - [x] Verify `cargo publish` dry-run passes
  - [ ] Test workflow on a release branch before merging
- [x] Create release runbook (`docs/release-runbook.md`)
  - [x] Document version bump procedure
  - [x] Document changelog update process
  - [x] Document tag and publish steps
  - [x] Include rollback procedure

#### Documentation Cleanup

- [x] Rewrite stale docs to reflect current architecture
  - [x] Audit `docs/architecture.md` for accuracy
  - [x] Update `docs/design.md` with current patterns
  - [ ] Verify all code examples compile and run
  - [x] Remove or archive bootstrap-era notes
- [x] Ensure exit codes are documented
  - [x] Verify `docs/reference/exit-codes.md` is complete
  - [x] Cross-reference exit codes in CLI help text

#### Quality Gates

- [x] Add tests to prevent `explain` / metadata drift
  - [x] Create test that validates `buildfix explain <fix>` output matches fixer implementation
  - [x] Add metadata consistency test for fix registry
  - [x] Ensure fix keys are documented and discoverable
- [ ] Restore and sort parked stash
  - [ ] Review git stash for parked changes
  - [ ] File relevant items into docs cleanup or non-release notes
  - [ ] Discard obsolete items

#### Repository Hygiene

- [ ] Reset local `main` to `origin/main`
  - [ ] Ensure clean sync with remote
  - [ ] Verify no stray commits

### Success Criteria

- [x] Releasing v0.2.1 requires zero institutional knowledge (follow runbook)
- [x] `buildfix explain <fix>` output is verified by automated tests
- [x] All exit codes documented in `docs/reference/exit-codes.md`
- [x] No stale bootstrap-era content in docs

### Effort Guidance

- **Estimated effort**: 1-2 days
- **Risk level**: Low
- **Blockers**: None

---

## v0.3.0 — Adapter Productization

### Objective

Make intake adapters a first-class extension point. Enable third-party contributors to add new sensor integrations without modifying buildfix core.

### Dependencies

- **Prerequisite**: v0.2.1 completed (release automation and docs trustworthy)

### Tasks

#### Receipt Model Documentation

- [x] Document the receipt model with schema and versioning notes
  - [x] Create `docs/reference/receipt-schema.md`
  - [x] Document receipt envelope structure
  - [x] Document versioning strategy for receipt formats
  - [x] Add examples for common receipt patterns

#### Adapter SDK and Test Harness

- [x] Add shared adapter test harness
  - [x] Create `buildfix-adapter-sdk` crate with test utilities
  - [x] Define `AdapterTestHarness` trait or struct
  - [x] Provide golden test macros for adapter validation
  - [x] Document harness usage patterns
- [x] Create adapter authoring guide
  - [x] Create `docs/how-to/write-adapter.md`
  - [x] Document input → normalized finding → receipt flow
  - [x] Include worked example with SARIF
  - [x] Document error handling best practices

#### First Adapter Microcrates

- [x] Publish `buildfix-receipts-sarif`
  - [x] Create microcrate structure
  - [x] Implement SARIF parsing
  - [x] Add golden tests with sample SARIF outputs
  - [x] Document supported SARIF producers
- [x] Publish `buildfix-receipts-cargo-deny`
  - [x] Create microcrate structure
  - [x] Implement cargo-deny JSON output parsing
  - [x] Add golden tests with sample outputs
  - [x] Document mapping from cargo-deny findings to receipts

#### CI Integration Examples

- [x] Ship CI integration examples teams can copy
  - [x] Create `.github/workflows-templates/` directory
  - [x] PR lane workflow: plan only, upload artifacts, optional PR comment
  - [x] Main lane workflow: apply safe fixes, optional bot commit, no CI loop
  - [x] Document workflow customization points
- [ ] Update `docs/how-to/ci-integration.md` with templates

#### Configuration Profiles

- [x] Add `buildfix.toml` profile examples
  - [x] Create `examples/profiles/conservative.toml` (safe ops only)
  - [x] Create `examples/profiles/balanced.toml` (safe + guarded with review)
  - [x] Create `examples/profiles/aggressive-but-reviewed.toml` (all ops, human review required)
  - [x] Document profile selection guidance

### Success Criteria

- [x] Adding a new adapter requires no buildfix core changes
- [x] CI integration is one copy-paste away
- [x] At least 2 intake microcrates published to crates.io
- [x] Adapter authoring guide is complete with worked example

### Effort Guidance

- **Estimated effort**: 3-5 days
- **Risk level**: Medium (new crate infrastructure)
- **Blockers**: None expected

---

## v0.4.0 — Evidence-Rich Unsafe Reduction

### Objective

Reduce `unsafe` classifications through better evidence, not loosened standards. Maintain the "no guessing" rule while enabling more operations to be classified as safe or guarded.

### Dependencies

- **Prerequisite**: v0.3.0 completed (adapter infrastructure in place)

### Tasks

#### Receipt Enrichment

- [ ] Enrich receipts with feature/target context
  - [ ] Extend receipt schema to include feature flags
  - [ ] Extend receipt schema to include target triples
  - [ ] Update adapter SDK to support enrichment
  - [ ] Document enrichment requirements for adapters
- [ ] Add confidence level metadata to receipts
  - [ ] Define confidence levels (high, medium, low)
  - [ ] Add source sensor metadata field
  - [ ] Document how confidence affects safety classification

#### Safety Classification Review

- [ ] Reduce unnecessary `unsafe` classifications
  - [ ] Audit current `unsafe` classifications
  - [ ] Identify cases where additional receipt evidence enables promotion
  - [ ] Implement promotion logic: unsafe → guarded where justified
  - [ ] Implement promotion logic: guarded → safe where justified
- [ ] Keep the "no guessing" rule intact
  - [ ] Document the rule in `docs/safety-model.md`
  - [ ] Add tests that verify no guessing in fixers
  - [ ] Code review checklist for new fixers

#### Scope Documentation

- [ ] Document workspace-wide vs crate-local scope
  - [ ] Define which operations are workspace-wide
  - [ ] Define which operations are crate-local
  - [ ] Document scope in fixer registry
  - [ ] Update `docs/reference/fixes.md` with scope info

### Success Criteria

- [ ] Measurable reduction in `unsafe` classifications (target: 20%+ reduction)
- [ ] All promotions justified by receipt improvements (documented rationale)
- [ ] "No guessing" policy documented and tested
- [ ] Scope clearly documented for all fixers

### Effort Guidance

- **Estimated effort**: 2-4 days
- **Risk level**: Medium (requires careful safety analysis)
- **Blockers**: v0.3.0 adapter infrastructure

---

## Future Milestones (Post-v0.4.0)

The following milestones are planned but not yet detailed:

| Version | Theme | Notes |
|---------|-------|-------|
| v0.5.0 | Fixer Catalog Expansion | Additional fixers based on user demand |
| v0.6.0 | Performance Optimization | Large workspace support, incremental planning |
| v1.0.0 | Stable API | API stability guarantees, semantic versioning |

See [ROADMAP.md](../ROADMAP.md) for strategic direction.

---

## Implementation Principles

These principles remain invariant across all milestones:

1. **Receipt-driven**: All fixes triggered by sensor findings, never invented
2. **Deterministic**: Same inputs always produce byte-identical outputs
3. **Safety-first**: Conservative classification, explicit approval for risky changes
4. **Reversible**: Backups and preconditions ensure recovery
5. **Transparent**: Full audit trail in JSON artifacts
6. **No guessing**: Derive from repo truth or require explicit user parameters

---

## Contributing

When implementing tasks from this plan:

1. Create a feature branch from `main`
2. Reference the task in commit messages (e.g., `v0.2.1: add release runbook`)
3. Update the checkbox in this file when complete
4. Ensure all tests pass before merging

For larger changes, open an issue first to discuss approach.
