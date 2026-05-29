//! Graph-level diff for UE Blueprint assets.
//!
//! Parses UE serialization text (Begin Object / End Object blocks) into
//! per-node blobs keyed by NodeGuid, then computes Added/Removed/Changed/Unchanged
//! status for each node across two versions of the same graph.

use std::collections::HashMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NodeStatus {
    Added,
    Removed,
    Changed,
    Unchanged,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphDiff {
    pub name: String,
    pub only_in_ours: bool,
    pub only_in_theirs: bool,
    pub node_statuses: HashMap<String, NodeStatus>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ThreeWayNodeStatus {
    Unchanged,
    ModifiedInOurs,
    ModifiedInTheirs,
    ModifiedInBoth,
    AddedInOurs,
    AddedInTheirs,
    AddedInBoth,
    AddedInBothConflict,
    RemovedInOurs,
    RemovedInTheirs,
    RemovedInBoth,
    ModifyDeleteConflict,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreeWayGraphDiff {
    pub name: String,
    pub only_in_ours: bool,
    pub only_in_theirs: bool,
    pub only_in_ancestor: bool,
    pub node_statuses: HashMap<String, ThreeWayNodeStatus>,
}

/// Compute per-GUID three-way status. The ancestor map represents the
/// merge base (git's %O); `ours` and `theirs` are %A and %B.
///
/// When a graph is missing from a side, its node set is empty for that side.
pub fn diff_graphs_three_way_inner(
    ancestor_graphs: &HashMap<String, String>,
    ours_graphs: &HashMap<String, String>,
    theirs_graphs: &HashMap<String, String>,
) -> Vec<ThreeWayGraphDiff> {
    let mut all_names: std::collections::BTreeSet<String> = Default::default();
    all_names.extend(ancestor_graphs.keys().cloned());
    all_names.extend(ours_graphs.keys().cloned());
    all_names.extend(theirs_graphs.keys().cloned());

    let mut result = Vec::new();
    for name in all_names {
        let anc_text = ancestor_graphs.get(&name);
        let our_text = ours_graphs.get(&name);
        let thr_text = theirs_graphs.get(&name);

        let only_in_ancestor = anc_text.is_some() && our_text.is_none() && thr_text.is_none();
        let only_in_ours = our_text.is_some() && anc_text.is_none() && thr_text.is_none();
        let only_in_theirs = thr_text.is_some() && anc_text.is_none() && our_text.is_none();

        let anc_nodes = anc_text.map(|t| parse_node_blobs(t)).unwrap_or_default();
        let our_nodes = our_text.map(|t| parse_node_blobs(t)).unwrap_or_default();
        let thr_nodes = thr_text.map(|t| parse_node_blobs(t)).unwrap_or_default();

        let mut all_guids: std::collections::BTreeSet<&String> = Default::default();
        all_guids.extend(anc_nodes.keys());
        all_guids.extend(our_nodes.keys());
        all_guids.extend(thr_nodes.keys());

        let mut node_statuses = HashMap::new();
        for guid in all_guids {
            let a = anc_nodes.get(guid);
            let o = our_nodes.get(guid);
            let t = thr_nodes.get(guid);

            let status = match (a, o, t) {
                // present nowhere - unreachable but cheap to handle
                (None, None, None) => continue,
                // only in ancestor
                (Some(_), None, None) => ThreeWayNodeStatus::RemovedInBoth,
                // added by one side
                (None, Some(_), None) => ThreeWayNodeStatus::AddedInOurs,
                (None, None, Some(_)) => ThreeWayNodeStatus::AddedInTheirs,
                // added by both
                (None, Some(o_b), Some(t_b)) => {
                    if blob_eq(o_b, t_b) {
                        ThreeWayNodeStatus::AddedInBoth
                    } else {
                        ThreeWayNodeStatus::AddedInBothConflict
                    }
                }
                // modify/delete
                (Some(_), Some(_), None) => {
                    let o_b = o.unwrap();
                    let a_b = a.unwrap();
                    if blob_eq(o_b, a_b) {
                        // ours unchanged, theirs deleted → just removed in theirs
                        ThreeWayNodeStatus::RemovedInTheirs
                    } else {
                        ThreeWayNodeStatus::ModifyDeleteConflict
                    }
                }
                (Some(_), None, Some(_)) => {
                    let t_b = t.unwrap();
                    let a_b = a.unwrap();
                    if blob_eq(t_b, a_b) {
                        ThreeWayNodeStatus::RemovedInOurs
                    } else {
                        ThreeWayNodeStatus::ModifyDeleteConflict
                    }
                }
                // present everywhere
                (Some(a_b), Some(o_b), Some(t_b)) => {
                    let o_eq_a = blob_eq(o_b, a_b);
                    let t_eq_a = blob_eq(t_b, a_b);
                    let o_eq_t = blob_eq(o_b, t_b);
                    if o_eq_a && t_eq_a {
                        ThreeWayNodeStatus::Unchanged
                    } else if o_eq_a {
                        ThreeWayNodeStatus::ModifiedInTheirs
                    } else if t_eq_a {
                        ThreeWayNodeStatus::ModifiedInOurs
                    } else if o_eq_t {
                        // Both sides made the same modification - pick either side, no conflict.
                        ThreeWayNodeStatus::ModifiedInOurs
                    } else {
                        ThreeWayNodeStatus::ModifiedInBoth
                    }
                }
            };
            node_statuses.insert(guid.clone(), status);
        }

        result.push(ThreeWayGraphDiff {
            name,
            only_in_ours,
            only_in_theirs,
            only_in_ancestor,
            node_statuses,
        });
    }
    result
}

// Canonicalise a node blob for *semantic* comparison: drop purely cosmetic
// fields so a node that merely moved on the canvas (UE rewrites NodePosX/NodePosY
// whenever either side edits the graph) isn't reported as changed/conflicting.
// Trims each line and drops blank lines so whitespace/line-ending differences
// don't matter either.
// Volatile, non-semantic bits of UE node serialization that differ between two
// exports of the *same* logic and would otherwise create false conflicts:
//   - ExportPath="..."        : embeds the per-file package path
//   - PinToolTip / PinFriendlyName : display-only, regenerated on reconstruction
//   - 32-hex GUIDs            : PinId / link / member / persistent IDs UE renews
//   - K2Node_<Class>_<index>  : the object-name index UE renumbers
// Patterns mirror the TS `normalizeBlob` in app/src/mergeGraphs.ts exactly.
use std::sync::OnceLock;
fn norm_patterns() -> &'static [(regex::Regex, &'static str)] {
    static RE: OnceLock<Vec<(regex::Regex, &'static str)>> = OnceLock::new();
    RE.get_or_init(|| {
        vec![
            (regex::Regex::new(r#"\s*ExportPath="[^"]*""#).unwrap(), ""),
            (regex::Regex::new(r#",?PinToolTip="(?:[^"\\]|\\.)*""#).unwrap(), ""),
            (regex::Regex::new(r#",?PinFriendlyName=NSLOCTEXT\([^)]*\)"#).unwrap(), ""),
            (regex::Regex::new(r#",?PinFriendlyName="(?:[^"\\]|\\.)*""#).unwrap(), ""),
            // Rust regex has no look-around; bound the GUID with non-hex or ends
            // via a capturing wrapper applied below instead.
        ]
    })
}

fn canon_guids(s: &str) -> String {
    // Replace maximal runs of exactly 32 hex chars with <GUID>.
    let mut out = String::with_capacity(s.len());
    let mut run = String::new();
    let flush = |run: &mut String, out: &mut String| {
        if !run.is_empty() {
            if run.len() == 32 {
                out.push_str("<GUID>");
            } else {
                out.push_str(run);
            }
            run.clear();
        }
    };
    for ch in s.chars() {
        if ch.is_ascii_hexdigit() {
            run.push(ch);
        } else {
            flush(&mut run, &mut out);
            out.push(ch);
        }
    }
    flush(&mut run, &mut out);
    out
}

fn strip_name_index() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"(K2Node_[A-Za-z]+)_\d+").unwrap())
}

fn normalize_blob(blob: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    for line in blob.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with("NodePosX=") || t.starts_with("NodePosY=") {
            continue;
        }
        let mut s = t.to_string();
        for (re, rep) in norm_patterns() {
            s = re.replace_all(&s, *rep).into_owned();
        }
        s = canon_guids(&s);
        s = strip_name_index().replace_all(&s, "$1").into_owned();
        out.push(s.trim().to_string());
    }
    out.join("\n")
}

// Semantic equality of two node blobs (ignores cosmetic position/whitespace).
fn blob_eq(a: &str, b: &str) -> bool {
    normalize_blob(a) == normalize_blob(b)
}

// Splits UE serialization text into per-node blobs keyed by NodeGuid.
// Uses depth-tracking to correctly handle nodes that contain nested Begin Object
// / End Object sub-objects (e.g., pins, default sub-objects).
// Only extracts NodeGuid from depth-1 (top-level node) properties.
// Duplicate GUIDs overwrite silently - malformed assets may lose nodes from diff.
fn parse_node_blobs(text: &str) -> HashMap<String, String> {
    let mut result = HashMap::new();
    let mut in_node = false;
    let mut depth: usize = 0;
    let mut node_lines: Vec<&str> = Vec::new();
    let mut node_guid: Option<String> = None;

    for line in text.lines() {
        let trimmed = line.trim();
        if !in_node {
            if trimmed.starts_with("Begin Object") {
                in_node = true;
                depth = 1;
                node_lines.clear();
                node_guid = None;
                node_lines.push(line);
            }
        } else {
            node_lines.push(line);
            if trimmed.starts_with("Begin Object") {
                depth += 1;
            } else if trimmed.starts_with("End Object") {
                depth -= 1;
                if depth == 0 {
                    if let Some(guid) = node_guid.take() {
                        result.insert(guid, node_lines.join("\n"));
                    }
                    in_node = false;
                    node_lines.clear();
                }
            } else if depth == 1 {
                // Only extract NodeGuid from top-level node properties.
                if let Some(rest) = trimmed.strip_prefix("NodeGuid=") {
                    node_guid = Some(rest.trim().to_string());
                }
            }
        }
    }
    result
}

pub fn diff_graphs_inner(
    ours_graphs: &HashMap<String, String>,
    theirs_graphs: &HashMap<String, String>,
) -> Vec<GraphDiff> {
    let mut all_names: std::collections::BTreeSet<String> = Default::default();
    all_names.extend(ours_graphs.keys().cloned());
    all_names.extend(theirs_graphs.keys().cloned());

    let mut result = Vec::new();
    for name in all_names {
        let ours_text = ours_graphs.get(&name);
        let theirs_text = theirs_graphs.get(&name);

        let only_in_ours = ours_text.is_some() && theirs_text.is_none();
        let only_in_theirs = ours_text.is_none() && theirs_text.is_some();

        let ours_nodes = ours_text.map(|t| parse_node_blobs(t)).unwrap_or_default();
        let theirs_nodes = theirs_text.map(|t| parse_node_blobs(t)).unwrap_or_default();

        let mut node_statuses = HashMap::new();

        for (guid, ours_blob) in &ours_nodes {
            if let Some(theirs_blob) = theirs_nodes.get(guid) {
                // Compare semantically - ignore cosmetic NodePos/whitespace so a
                // node that only moved isn't reported as Changed.
                if blob_eq(ours_blob, theirs_blob) {
                    node_statuses.insert(guid.clone(), NodeStatus::Unchanged);
                } else {
                    node_statuses.insert(guid.clone(), NodeStatus::Changed);
                }
            } else {
                node_statuses.insert(guid.clone(), NodeStatus::Removed);
            }
        }

        for guid in theirs_nodes.keys() {
            if !ours_nodes.contains_key(guid) {
                node_statuses.insert(guid.clone(), NodeStatus::Added);
            }
        }

        result.push(GraphDiff {
            name,
            only_in_ours,
            only_in_theirs,
            node_statuses,
        });
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_graphs(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    const NODE_A: &str = "\
Begin Object Class=/Script/BlueprintGraph.K2Node_Event Name=\"K2Node_Event_0\"
   NodeGuid=AAAAAAAA000000000000000000000001
   NodePosX=100
End Object
";

    // Semantically different from NODE_A (extra NodeComment), NOT just moved.
    const NODE_A_CHANGED: &str = "\
Begin Object Class=/Script/BlueprintGraph.K2Node_Event Name=\"K2Node_Event_0\"
   NodeGuid=AAAAAAAA000000000000000000000001
   NodePosX=100
   NodeComment=\"ours\"
End Object
";

    // Same node moved only (NodePosX differs) - must compare equal to NODE_A.
    const NODE_A_MOVED: &str = "\
Begin Object Class=/Script/BlueprintGraph.K2Node_Event Name=\"K2Node_Event_0\"
   NodeGuid=AAAAAAAA000000000000000000000001
   NodePosX=999
End Object
";

    const NODE_B: &str = "\
Begin Object Class=/Script/BlueprintGraph.K2Node_CallFunction Name=\"K2Node_CallFunction_0\"
   NodeGuid=BBBBBBBB000000000000000000000002
   NodePosX=300
End Object
";

    #[test]
    fn test_diff_unchanged() {
        let ours = make_graphs(&[("EventGraph", NODE_A)]);
        let theirs = make_graphs(&[("EventGraph", NODE_A)]);
        let diffs = diff_graphs_inner(&ours, &theirs);
        assert_eq!(diffs.len(), 1);
        let diff = &diffs[0];
        assert_eq!(diff.name, "EventGraph");
        assert!(!diff.only_in_ours);
        assert!(!diff.only_in_theirs);
        assert_eq!(
            diff.node_statuses.get("AAAAAAAA000000000000000000000001"),
            Some(&NodeStatus::Unchanged)
        );
    }

    #[test]
    fn position_only_change_is_unchanged() {
        // A node that only moved (different NodePosX) must NOT be reported as
        // changed - UE rewrites positions whenever either side edits the graph.
        let ours = make_graphs(&[("EventGraph", NODE_A)]);
        let theirs = make_graphs(&[("EventGraph", NODE_A_MOVED)]);
        let diffs = diff_graphs_inner(&ours, &theirs);
        assert_eq!(
            diffs[0].node_statuses.get("AAAAAAAA000000000000000000000001"),
            Some(&NodeStatus::Unchanged)
        );
    }

    #[test]
    fn three_way_position_only_change_is_unchanged() {
        let s = three_way_status(
            &[("EventGraph", NODE_A)],
            &[("EventGraph", NODE_A_MOVED)],
            &[("EventGraph", NODE_A)],
            "AAAAAAAA000000000000000000000001",
        );
        assert_eq!(s, Some(ThreeWayNodeStatus::Unchanged));
    }

    #[test]
    fn test_diff_changed() {
        let ours = make_graphs(&[("EventGraph", NODE_A)]);
        let theirs = make_graphs(&[("EventGraph", NODE_A_CHANGED)]);
        let diffs = diff_graphs_inner(&ours, &theirs);
        assert_eq!(
            diffs[0].node_statuses.get("AAAAAAAA000000000000000000000001"),
            Some(&NodeStatus::Changed)
        );
    }

    #[test]
    fn test_diff_removed() {
        let ours = make_graphs(&[("EventGraph", NODE_A)]);
        let theirs = make_graphs(&[("EventGraph", "")]);
        let diffs = diff_graphs_inner(&ours, &theirs);
        assert_eq!(
            diffs[0].node_statuses.get("AAAAAAAA000000000000000000000001"),
            Some(&NodeStatus::Removed)
        );
    }

    #[test]
    fn test_diff_added() {
        let ours = make_graphs(&[("EventGraph", "")]);
        let theirs = make_graphs(&[("EventGraph", NODE_B)]);
        let diffs = diff_graphs_inner(&ours, &theirs);
        assert_eq!(
            diffs[0].node_statuses.get("BBBBBBBB000000000000000000000002"),
            Some(&NodeStatus::Added)
        );
    }

    #[test]
    fn test_graph_only_in_ours() {
        let ours = make_graphs(&[("EventGraph", NODE_A), ("MyFunction", NODE_B)]);
        let theirs = make_graphs(&[("EventGraph", NODE_A)]);
        let diffs = diff_graphs_inner(&ours, &theirs);
        let my_fn = diffs.iter().find(|d| d.name == "MyFunction").unwrap();
        assert!(my_fn.only_in_ours);
        assert!(!my_fn.only_in_theirs);
    }

    #[test]
    fn test_graph_only_in_theirs() {
        let ours = make_graphs(&[("EventGraph", NODE_A)]);
        let theirs = make_graphs(&[("EventGraph", NODE_A), ("NewGraph", NODE_B)]);
        let diffs = diff_graphs_inner(&ours, &theirs);
        let new_graph = diffs.iter().find(|d| d.name == "NewGraph").unwrap();
        assert!(!new_graph.only_in_ours);
        assert!(new_graph.only_in_theirs);
    }

    const NODE_WITH_SUBOBJ: &str = "\
Begin Object Class=/Script/BlueprintGraph.K2Node_Event Name=\"K2Node_Event_0\"
   NodeGuid=DDDDDDDD000000000000000000000004
   Begin Object Name=\"SubPin_0\"
      PinName=\"execute\"
   End Object
   NodePosX=100
End Object
";

    const NODE_WITH_SUBOBJ_CHANGED: &str = "\
Begin Object Class=/Script/BlueprintGraph.K2Node_Event Name=\"K2Node_Event_0\"
   NodeGuid=DDDDDDDD000000000000000000000004
   Begin Object Name=\"SubPin_0\"
      PinName=\"execute\"
   End Object
   NodePosX=100
   NodeComment=\"changed\"
End Object
";

    #[test]
    fn test_diff_changed_after_subobj() {
        // Verifies that a property after a nested Begin Object block is included
        // in the blob comparison. If the parser truncates at the inner End Object,
        // both blobs would be equal and this test would incorrectly pass as Unchanged.
        let ours = make_graphs(&[("EventGraph", NODE_WITH_SUBOBJ)]);
        let theirs = make_graphs(&[("EventGraph", NODE_WITH_SUBOBJ_CHANGED)]);
        let diffs = diff_graphs_inner(&ours, &theirs);
        assert_eq!(
            diffs[0].node_statuses.get("DDDDDDDD000000000000000000000004"),
            Some(&NodeStatus::Changed)
        );
    }

    // A different semantic change than NODE_A_CHANGED (different comment).
    const NODE_A_V2: &str = "\
Begin Object Class=/Script/BlueprintGraph.K2Node_Event Name=\"K2Node_Event_0\"
   NodeGuid=AAAAAAAA000000000000000000000001
   NodePosX=100
   NodeComment=\"theirs\"
End Object
";

    fn three_way_status(
        anc: &[(&str, &str)],
        ours: &[(&str, &str)],
        theirs: &[(&str, &str)],
        guid: &str,
    ) -> Option<ThreeWayNodeStatus> {
        let diffs = diff_graphs_three_way_inner(
            &make_graphs(anc), &make_graphs(ours), &make_graphs(theirs),
        );
        diffs.iter().find(|d| d.name == "EventGraph")
            .and_then(|d| d.node_statuses.get(guid).cloned())
    }

    #[test]
    fn three_way_unchanged() {
        let s = three_way_status(
            &[("EventGraph", NODE_A)], &[("EventGraph", NODE_A)], &[("EventGraph", NODE_A)],
            "AAAAAAAA000000000000000000000001",
        );
        assert_eq!(s, Some(ThreeWayNodeStatus::Unchanged));
    }

    #[test]
    fn three_way_modified_in_ours() {
        let s = three_way_status(
            &[("EventGraph", NODE_A)],
            &[("EventGraph", NODE_A_CHANGED)],
            &[("EventGraph", NODE_A)],
            "AAAAAAAA000000000000000000000001",
        );
        assert_eq!(s, Some(ThreeWayNodeStatus::ModifiedInOurs));
    }

    #[test]
    fn three_way_modified_in_theirs() {
        let s = three_way_status(
            &[("EventGraph", NODE_A)],
            &[("EventGraph", NODE_A)],
            &[("EventGraph", NODE_A_CHANGED)],
            "AAAAAAAA000000000000000000000001",
        );
        assert_eq!(s, Some(ThreeWayNodeStatus::ModifiedInTheirs));
    }

    #[test]
    fn three_way_modified_in_both_same_change_is_not_conflict() {
        let s = three_way_status(
            &[("EventGraph", NODE_A)],
            &[("EventGraph", NODE_A_CHANGED)],
            &[("EventGraph", NODE_A_CHANGED)],
            "AAAAAAAA000000000000000000000001",
        );
        assert_eq!(s, Some(ThreeWayNodeStatus::ModifiedInOurs));
    }

    #[test]
    fn three_way_modified_in_both_conflict() {
        let s = three_way_status(
            &[("EventGraph", NODE_A)],
            &[("EventGraph", NODE_A_CHANGED)],
            &[("EventGraph", NODE_A_V2)],
            "AAAAAAAA000000000000000000000001",
        );
        assert_eq!(s, Some(ThreeWayNodeStatus::ModifiedInBoth));
    }

    #[test]
    fn three_way_removed_in_ours() {
        let s = three_way_status(
            &[("EventGraph", NODE_A)],
            &[("EventGraph", "")],
            &[("EventGraph", NODE_A)],
            "AAAAAAAA000000000000000000000001",
        );
        assert_eq!(s, Some(ThreeWayNodeStatus::RemovedInOurs));
    }

    #[test]
    fn three_way_removed_in_theirs() {
        let s = three_way_status(
            &[("EventGraph", NODE_A)],
            &[("EventGraph", NODE_A)],
            &[("EventGraph", "")],
            "AAAAAAAA000000000000000000000001",
        );
        assert_eq!(s, Some(ThreeWayNodeStatus::RemovedInTheirs));
    }

    #[test]
    fn three_way_removed_in_both() {
        let s = three_way_status(
            &[("EventGraph", NODE_A)],
            &[("EventGraph", "")],
            &[("EventGraph", "")],
            "AAAAAAAA000000000000000000000001",
        );
        assert_eq!(s, Some(ThreeWayNodeStatus::RemovedInBoth));
    }

    #[test]
    fn three_way_modify_delete_conflict_ours_kept() {
        let s = three_way_status(
            &[("EventGraph", NODE_A)],
            &[("EventGraph", NODE_A_CHANGED)],
            &[("EventGraph", "")],
            "AAAAAAAA000000000000000000000001",
        );
        assert_eq!(s, Some(ThreeWayNodeStatus::ModifyDeleteConflict));
    }

    #[test]
    fn three_way_modify_delete_conflict_theirs_kept() {
        let s = three_way_status(
            &[("EventGraph", NODE_A)],
            &[("EventGraph", "")],
            &[("EventGraph", NODE_A_CHANGED)],
            "AAAAAAAA000000000000000000000001",
        );
        assert_eq!(s, Some(ThreeWayNodeStatus::ModifyDeleteConflict));
    }

    #[test]
    fn three_way_added_in_ours() {
        let s = three_way_status(
            &[("EventGraph", "")],
            &[("EventGraph", NODE_B)],
            &[("EventGraph", "")],
            "BBBBBBBB000000000000000000000002",
        );
        assert_eq!(s, Some(ThreeWayNodeStatus::AddedInOurs));
    }

    #[test]
    fn three_way_added_in_theirs() {
        let s = three_way_status(
            &[("EventGraph", "")],
            &[("EventGraph", "")],
            &[("EventGraph", NODE_B)],
            "BBBBBBBB000000000000000000000002",
        );
        assert_eq!(s, Some(ThreeWayNodeStatus::AddedInTheirs));
    }

    #[test]
    fn three_way_added_in_both_identical() {
        let s = three_way_status(
            &[("EventGraph", "")],
            &[("EventGraph", NODE_B)],
            &[("EventGraph", NODE_B)],
            "BBBBBBBB000000000000000000000002",
        );
        assert_eq!(s, Some(ThreeWayNodeStatus::AddedInBoth));
    }

    #[test]
    fn three_way_added_in_both_conflict() {
        // Two SEMANTICALLY different node blobs that share the same GUID (rare
        // but the algorithm should flag them as a conflict). Differ by a real
        // field (NodeComment), not just position.
        let other_b = "Begin Object Class=/Script/BlueprintGraph.K2Node_CallFunction Name=\"K2Node_CallFunction_0\"
   NodeGuid=BBBBBBBB000000000000000000000002
   NodePosX=300
   NodeComment=\"theirs\"
End Object
";
        let s = three_way_status(
            &[("EventGraph", "")],
            &[("EventGraph", NODE_B)],
            &[("EventGraph", other_b)],
            "BBBBBBBB000000000000000000000002",
        );
        assert_eq!(s, Some(ThreeWayNodeStatus::AddedInBothConflict));
    }

    #[test]
    fn three_way_graph_only_in_ancestor_yields_removed_in_both() {
        let diffs = diff_graphs_three_way_inner(
            &make_graphs(&[("DeadGraph", NODE_A)]),
            &make_graphs(&[]),
            &make_graphs(&[]),
        );
        let dead = diffs.iter().find(|d| d.name == "DeadGraph").unwrap();
        assert!(dead.only_in_ancestor);
        assert_eq!(
            dead.node_statuses.get("AAAAAAAA000000000000000000000001"),
            Some(&ThreeWayNodeStatus::RemovedInBoth),
        );
    }
}
