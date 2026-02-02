# Architecture notes

buildfix is intentionally split into small crates to keep responsibilities clear:

- **Domain** (`buildfix-domain`) decides *what* should change.
- **Edit engine** (`buildfix-edit`) decides *how* to modify files.
- **CLI** (`buildfix`) coordinates IO and artifact emission.

The receipt format is tolerant and designed for incremental adoption: sensors can add fields without breaking buildfix.

## Safety model

Every planned fix has a safety class:

- `safe`: deterministic + low impact.
- `guarded`: deterministic but higher impact (requires explicit allow).
- `unsafe`: ambiguous without user input (not emitted by the built-in fixers today).

The planner can still *detect* issues without producing unsafe edits.

## Preconditions

Plans include sha256 preconditions for each touched file. Apply refuses to run when the repo has drifted (unless `require_clean_hashes` is disabled in the plan).
