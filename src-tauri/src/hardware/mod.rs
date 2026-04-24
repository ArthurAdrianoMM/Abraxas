//! Hardware detection and backend selection.
//!
//! Fase 2.1 ships `system` + `cpu_features`. `gpu` (2.2) and `selector` (2.3)
//! are still stubs; they will plug into a top-level `HardwareReport` that
//! composes `SystemInfo` with a `GpuInfo` and a chosen backend.

pub mod cpu_features;
pub mod gpu;
pub mod selector;
pub mod system;
