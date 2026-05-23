//! Wire types for the JSON emitted by ue-host/Plugins/MergeBinariesExport.
//!
//! These deserialise the response shape from Plan 1 §6. Field naming follows
//! the JSON (camelCase) via serde rename, while Rust field names stay snake.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A full export response from the commandlet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetSnapshot {
    /// Echo of the request id (only present when the request had one).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,

    pub ok: bool,

    /// Echo of the input path (absolute, OS-shaped). Goldens strip this.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,

    pub package: Package,

    pub asset: Asset,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Package {
    pub name: String,

    #[serde(rename = "engineVersion")]
    pub engine_version: String,

    #[serde(rename = "fileVersionUE5")]
    pub file_version_ue5: u32,

    #[serde(rename = "savedHash")]
    pub saved_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Asset {
    pub class: String,

    #[serde(rename = "parentClass", default)]
    pub parent_class: String,

    #[serde(default)]
    pub name: String,

    #[serde(default)]
    pub properties: Vec<Property>,

    #[serde(default)]
    pub graphs: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Property {
    pub path: String,

    #[serde(rename = "type")]
    pub ty: String,

    pub value: PropertyValue,
}

/// Property values are dynamic — they can be a primitive (bool/number/string)
/// or a typed-summary object for structs/arrays/maps/sets. We accept any JSON.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum PropertyValue {
    Null,
    Bool(bool),
    Number(serde_json::Number),
    String(String),
    Summary(serde_json::Map<String, serde_json::Value>),
}

/// Wire-format response when the commandlet reports an error (`ok:false`).
/// We don't deserialise into AssetSnapshot in that case — call sites should
/// branch on `ok` before treating a response as a snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,
    pub ok: bool, // always false for this variant
    pub error: String,
}
