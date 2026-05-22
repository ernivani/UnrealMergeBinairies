//! Pure diff functions over `Property` lists. Order-independent; keyed by `path`.

use crate::schema::{Property, PropertyValue};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PropertyChange {
    Added {
        path: String,
        ty: String,
        value: PropertyValue,
    },
    Removed {
        path: String,
        ty: String,
        value: PropertyValue,
    },
    Changed {
        path: String,
        ty: String,
        old: PropertyValue,
        new: PropertyValue,
    },
}

/// 2-way diff: what changed going from `base` to `other`. Symmetric (swap args to flip).
pub fn diff_properties(base: &[Property], other: &[Property]) -> Vec<PropertyChange> {
    let base_map: HashMap<&str, &Property> = base.iter().map(|p| (p.path.as_str(), p)).collect();
    let other_map: HashMap<&str, &Property> = other.iter().map(|p| (p.path.as_str(), p)).collect();

    let mut changes = Vec::new();

    // Pass 1: things present in `base`; check if other has them.
    for (path, base_prop) in &base_map {
        match other_map.get(path) {
            None => changes.push(PropertyChange::Removed {
                path: path.to_string(),
                ty: base_prop.ty.clone(),
                value: base_prop.value.clone(),
            }),
            Some(other_prop) if other_prop.value != base_prop.value => {
                changes.push(PropertyChange::Changed {
                    path: path.to_string(),
                    ty: base_prop.ty.clone(),
                    old: base_prop.value.clone(),
                    new: other_prop.value.clone(),
                })
            }
            Some(_) => {}
        }
    }

    // Pass 2: things added in `other` that didn't exist in `base`.
    for (path, other_prop) in &other_map {
        if !base_map.contains_key(path) {
            changes.push(PropertyChange::Added {
                path: path.to_string(),
                ty: other_prop.ty.clone(),
                value: other_prop.value.clone(),
            });
        }
    }

    // Sort by path so output is deterministic.
    changes.sort_by(|a, b| change_path(a).cmp(change_path(b)));
    changes
}

fn change_path(c: &PropertyChange) -> &str {
    match c {
        PropertyChange::Added { path, .. }
        | PropertyChange::Removed { path, .. }
        | PropertyChange::Changed { path, .. } => path,
    }
}
