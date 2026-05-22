//! Backend for unreal-merge: spawn UE commandlet, diff snapshots, resolve conflicts.

pub mod diff;
pub mod schema;
pub mod sidecar;

pub use diff::{PropertyChange, diff_properties};
pub use schema::{Asset, AssetSnapshot, ErrorResponse, Package, Property, PropertyValue};
pub use sidecar::{Sidecar, SidecarConfig, extract_json_objects};
