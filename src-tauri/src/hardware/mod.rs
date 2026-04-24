//! Hardware detection and backend selection.
//!
//! Fase 2.1 ships `system` + `cpu_features`; Fase 2.2 adds `gpu`. `selector`
//! (2.3) remains a stub — it will compose `SystemInfo` with `GpuBackend` to
//! produce a chosen inference backend + justification.

pub mod cpu_features;
pub mod gpu;
pub mod selector;
pub mod system;
