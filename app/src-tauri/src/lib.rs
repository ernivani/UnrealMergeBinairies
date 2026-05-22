//! Backend for unreal-merge: spawn UE commandlet, diff snapshots, resolve conflicts.

pub mod schema;

pub use schema::{Asset, AssetSnapshot, ErrorResponse, Package, Property, PropertyValue};
