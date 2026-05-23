use std::collections::HashMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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

fn extract_guid(node_text: &str) -> Option<String> {
    for line in node_text.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("NodeGuid=") {
            return Some(rest.trim().to_string());
        }
    }
    None
}

fn parse_node_blobs(text: &str) -> HashMap<String, String> {
    let mut result = HashMap::new();
    for part in text.split("Begin Object").skip(1) {
        let end_idx = part.find("End Object").unwrap_or(part.len());
        let node_text = &part[..end_idx];
        if let Some(guid) = extract_guid(node_text) {
            result.insert(guid, node_text.to_string());
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
                if ours_blob == theirs_blob {
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

    const NODE_A_CHANGED: &str = "\
Begin Object Class=/Script/BlueprintGraph.K2Node_Event Name=\"K2Node_Event_0\"
   NodeGuid=AAAAAAAA000000000000000000000001
   NodePosX=200
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
}
