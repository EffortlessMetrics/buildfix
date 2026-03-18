# buildfix Roadmap

> **Post-v0.2.0 Productization Direction**
> PR #2 merged to main — bootstrap complete, now shipping.

## Strategic Vision

buildfix is the **actuator layer** for Cargo workspace hygiene — not a scanner, not a linter, and definitely not a tool that "rewrites your repo in place."

### What Makes buildfix Different

| Aspect | buildfix | Typical "Fix-It" Tools |
|--------|----------|------------------------|
| **Input** | Sensor receipts only | Inline analysis / heuristics |
| **Output** | Deterministic, reviewable patches | Direct filesystem mutation |
| **Safety** | Policy-aware (safe/guarded/unsafe) | Often all-or-nothing |
| **Audit** | Full artifact trail (plan.md, patch.diff, report.json) | Minimal or none |
| **Philosophy** | Evidence over guessing | Best-effort inference |

buildfix consumes findings from sensors (cargo-deny, cargo-machete, rustc, etc.) and emits **deterministic, reviewable repair plans**. Every operation is classified by safety, bounded by SHA256 preconditions, and fully auditable. We don't guess at fixes — we translate evidence into mechanical edits.

---

## Five Strategic Pillars

### 1. Release Operations Stability
The CLI must be installable and releasable without heroics. Publishing a new version should be boring.

### 2. Adapter Ergonomics
Adding new intake sources should be straightforward. Intake adapters are first-class microcrates with a clear SDK.

### 3. Explanation Integrity
What `buildfix explain` says must match what `buildfix apply` does. Drift between documentation and implementation is a bug.

### 4. CI Adoption
Integration into CI pipelines must be copy-pasteable. Users shouldn't need to read source code to use buildfix in automation.

### 5. Evidence-Rich Receipts Over Guessing
Safety improves via better evidence, not loosened standards. We prefer richer receipts over heuristics.

---

## "Properly Working" Standard

buildfix is considered properly working when:

- [ ] **CLI installable and releasable without heroics**
  - `cargo install buildfix` works
  - crates.io publish is automated
  - Version bumps are mechanical

- [ ] **plan/apply output deterministic and auditable**
  - Same inputs → byte-identical outputs
  - plan.md, patch.diff, report.json tell the full story
  - Preconditions verified before writes

- [ ] **Explanation surfaces match implementation**
  - `buildfix explain <fix>` describes actual behavior
  - Docs don't drift from code
  - Metadata tests catch divergence

- [ ] **Adding new intake sources is straightforward**
  - Clear adapter SDK
  - Test harness for new adapters
  - Example templates

- [ ] **CI integration is copy-pasteable**
  - Documented workflow templates
  - Exit codes documented and stable
  - Common patterns covered

- [ ] **Safety improves via better evidence, not loosened standards**
  - Fewer `unsafe` classifications over time
  - Richer receipts enable safer ops
  - No guessing — derive from repo or ask user

---

## Milestones

### v0.2.1 — Operational Hardening

**Theme:** Make releases boring, make docs trustworthy.

| Deliverable | Description |
|-------------|-------------|
| Publish workflow | Automated crates.io publishing on tag |
| Runbook | Step-by-step release process documentation |
| Docs cleanup | Remove bootstrap-era stale content |
| Explain/metadata drift tests | Automated checks that `explain` matches implementation |
| Exit code audit | Verify exit codes (0/1/2) are consistent and documented |

**Success Criteria:**
- Releasing v0.2.1 requires zero institutional knowledge
- `buildfix explain <fix>` is verified by tests
- All exit codes documented in `docs/reference/exit-codes.md`

---

### v0.3.0 — Adapter Productization

**Theme:** Make intake adapters a first-class extension point.

| Deliverable | Description |
|-------------|-------------|
| Adapter SDK | `buildfix-adapter-sdk` crate with traits and test utilities |
| Adapter harness | Test framework for validating new adapters |
| First intake microcrates | See [Planned Intake Adapters](#planned-intake-adapters) |
| CI templates | Copy-pasteable GitHub Actions workflows |
| Adapter documentation | How-to guide for writing new adapters |

**Success Criteria:**
- Adding a new adapter requires no buildfix core changes
- CI integration is one copy-paste away
- At least 2 intake microcrates published

---

### v0.4.0 — Evidence-Rich Unsafe Reduction

**Theme:** Reduce `unsafe` classifications through better evidence.

| Deliverable | Description |
|-------------|-------------|
| Richer receipt schemas | More context in sensor outputs |
| Unsafe → guarded promotion | Cases that were unsafe due to missing evidence become guarded |
| Guarded → safe promotion | Cases that were guarded due to ambiguity become safe |
| No guessing policy | Document and enforce "derive from repo or ask user" |

**Success Criteria:**
- Measurable reduction in `unsafe` classifications
- All promotions justified by receipt improvements
- Policy documented and tested

---

## Planned Intake Adapters

Intake adapters are microcrates that translate sensor outputs into buildfix receipts. Each adapter is a separate crate with minimal dependencies.

| Microcrate | Sensor | Purpose |
|------------|--------|---------|
| `buildfix-receipts-sarif` | SARIF producers | Generic SARIF intake for tools emitting Static Analysis Results Interchange Format |
| `buildfix-receipts-cargo-deny` | cargo-deny | License, advisory, and ban findings |
| `buildfix-receipts-cargo-udeps` | cargo-udeps | Unused dependency detection |
| `buildfix-receipts-cargo-machete` | cargo-machete | Unused dependency detection (alternative) |
| `buildfix-receipts-rustc-json` | rustc JSON messages | Edition, MSRV, and compilation findings |

### Adapter Architecture

```
┌─────────────────┐     ┌──────────────────────┐     ┌─────────────────┐
│  Sensor Output  │────▶│  Intake Microcrate   │────▶│  buildfix-core  │
│  (JSON/SARIF)   │     │  (buildfix-receipts-│     │  (planner)      │
└─────────────────┘     │   <sensor>)          │     └─────────────────┘
                        └──────────────────────┘
                                 │
                                 ▼
                        ┌──────────────────────┐
                        │  Normalized Receipt  │
                        │  (buildfix-types)    │
                        └──────────────────────┘
```

Each microcrate:
- Depends only on `buildfix-types` and `buildfix-adapter-sdk`
- Exposes a single `fn load(path: &Path) -> Result<Vec<Receipt>>`
- Includes golden tests for sample sensor outputs
- Is independently versionable

---

## Design Principles

These principles remain invariant across all milestones:

1. **Receipt-driven**: All fixes triggered by sensor findings, never invented
2. **Deterministic**: Same inputs always produce byte-identical outputs
3. **Safety-first**: Conservative classification, explicit approval for risky changes
4. **Reversible**: Backups and preconditions ensure recovery
5. **Transparent**: Full audit trail in JSON artifacts
6. **No guessing**: Derive from repo truth or require explicit user parameters

---

## Out of Scope

buildfix will NOT:

- Perform inline code analysis or linting
- Rewrite repositories in place without review
- Infer values that aren't explicitly provided or derivable
- Bypass safety classifications for convenience

---

## Contributing

Feature requests and sensor integration ideas are welcome. Please open an issue to discuss before implementing.

When proposing new fixers or adapters, include:
- Receipt format from the triggering sensor
- Safety classification rationale
- Example input/output transformation
- Evidence requirements (what's needed to promote from unsafe)

---

## Version History

| Version | Theme | Status |
|---------|-------|--------|
| v0.2.0 | Bootstrap release | ✅ Merged to main |
| v0.2.1 | Operational hardening | 🔄 Planned |
| v0.3.0 | Adapter productization | 📋 Planned |
| v0.4.0 | Evidence-rich unsafe reduction | 📋 Planned |
