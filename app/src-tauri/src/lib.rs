//! Backend for unreal-merge.

pub mod app_mode;
pub mod cli;
pub mod diff;
pub mod git;
pub mod graph_diff;
pub mod installer;
pub mod ipc;
pub mod merge;
pub mod schema;
pub mod sidecar;

pub use diff::{PropertyChange, diff_properties};
pub use schema::{Asset, AssetSnapshot, ErrorResponse, Package, Property, PropertyValue};
pub use sidecar::{Sidecar, SidecarConfig, extract_json_objects};


