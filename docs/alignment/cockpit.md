# Cockpit Alignment Manifesto

This document outlines the strategy for keeping `buildfix` aligned with the Cockpit ecosystem.

## The Kernel Pin Strategy

To ensure stability and auditability, `buildfix` aligns with specific versions of the Cockpit "kernel" (contracts and schemas).

- **Current Kernel Pin**: `vendor/cockpit-contracts` (checked in)
- **Rollout Goal**: buildfix v0.2 foundations conforms to cockpitctl v1.0 contracts.

## Conformance Guarantees

1. **ABI Stability**: The `buildfix.report.v1` output MUST strictly conform to `sensor.report.v1`.
2. **Postmark Logic**: `report.json` is the fixed "postmark". All tool-specific evolution lives in `data.buildfix.*` or via `artifacts[]` pointers.
3. **Auditability**:
   - `plan.json` MUST contain file digests (preconditions) and routing keys.
   - `apply.json` MUST reference the plan hash and record actual changes.

## Evolution Space

- Everything under `data.buildfix` is considered "local evolution space".
- Buildfix can move fast here without breaking the Cockpit bus.
- Stable promotion keys (e.g., `data.buildfix.plan.fix_available`) are maintained for Cockpit UI consumption.
