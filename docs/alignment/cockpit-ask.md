# Ask to Cockpit Team

This is the checklist of specific answers needed to lock the buildfix-cockpit integration.

## 1. The Kernel Pin
**What tag/commit is the “kernel” for this rollout?**
We need one canonical reference to say: “buildfix v0.2 foundations conforms to cockpitctl **vX.Y.Z** contracts.”

## 2. The final `sensor.report.v1` shape
We need authoritative answers to:
- **Capabilities location**: `run.capabilities` vs top-level (current schema says top-level).
- **Artifacts pointer shape**: Should it be an array of objects or a fixed object (current schema says fixed object)?
- **Required fields**: What are the absolute minimums for a valid receipt?
- **Counts semantics**: Confirm `verdict.counts` reflects findings only.
- **Token regex**: Confirm naming rules for `check_id` and `code`.

## 3. Reserved IDs and Paths
- **Reserved sensor IDs**: (e.g., `cockpit`, `buildfix`)
- **Reserved artifact IDs**: (e.g., `comment`, `sarif`, `handoff`)
- **Reserved directories**: beyond `cockpit/` and `buildfix/` under `artifacts/`.

## 4. Status Model for Presence
How does Cockpit distinguish:
- `missing receipt`
- `receipt exists with verdict skip`
- `invalid receipt`
And should buildfix be a row, a sub-panel, or both?

## 5. Is `buildfix.plan.v1` cockpit-owned ABI?
- Is it in the cockpit contracts pack?
- Required fields for finding refs and preconditions?

## 6. Suggestions Consumption
- Is `data._cockpit` validated/used in v1?
- Should we include rendering hints?
