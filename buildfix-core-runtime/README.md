# buildfix-core-runtime

Small runtime primitives for buildfix core embedding:
- port traits (ReceiptSource, GitPort, WritePort)
- filesystem/in-memory adapters
- plan/apply settings models

This crate keeps host-facing I/O and configuration concerns separate from
pipeline orchestration so downstream binaries and embedders can re-use them
without depending on the full orchestration layer.

