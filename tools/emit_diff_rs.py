from pathlib import Path

RUST = r"""
//! Compare two loaded [`AppConfig`](crate::config::model::AppConfig) values.

use std::collections::{HashMap, HashSet};

use serde::Serialize;
use serde_json::Value;

use crate::config::model::{AppConfig, RoutingRule};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ConfigDiffChange {
    pub path: String,
    pub kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub left: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub right: Option<Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConfigDiffReport {
    pub diagnostic_version: &'static str,
    pub left_path: String,
    pub right_path: String,
    pub identical: bool,
    pub changes: Vec<ConfigDiffChange>,
}

pub fn diff_app_configs(
    left: &AppConfig,
    right: &AppConfig,
    left_path: &str,
    right_path: &str,
) -> ConfigDiffReport {
    let mut changes = Vec::new();

    diff_json_leaf(
        "server",
        &serde_json::to_value(&left.server).expect("server serde"),
        &serde_json::to_value(&right.server).expect("server serde"),
        &mut changes,
    );

    if left.log_level != right.log_level {
        changes.push(ConfigDiffChange {
            path: "log_level".to_string(),
            kind: "modified",
            left: Some(Value::String(left.log_level.clone())),
            right: Some(Value::String(right.log_level.clone())),
        });
    }

    diff_json_leaf(
        "registries",
        &serde_json::to_value(&left.registries).expect("registries serde"),
        &serde_json::to_value(&right.registries).expect("registries serde"),
        &mut changes,
    );

    diff_routes_by_id(&left.routes, &right.routes, &mut changes);

    let identical = changes.is_empty();
    ConfigDiffReport {
        diagnostic_version: "1.0",
        left_path: left_path.to_string(),
        right_path: right_path.to_string(),
        identical,
        changes,
    }
}

fn diff_routes_by_id(
    left: &[RoutingRule],
    right: &[RoutingRule],
    changes: &mut Vec<ConfigDiffChange>,
) {
    let lm: HashMap<String, &RoutingRule> = left.iter().map(|r| (r.id.clone(), r)).collect();
    let rm: HashMap<String, &RoutingRule> = right.iter().map(|r| (r.id.clone(), r)).collect();

    let ids_l: HashSet<_> = lm.keys().cloned().collect();
    let ids_r: HashSet<_> = rm.keys().cloned().collect();

    for id in ids_l.difference(&ids_r) {
        changes.push(ConfigDiffChange {
            path: format!("routes[{id}]"),
            kind: "removed",
            left: Some(serde_json::to_value(lm[id]).expect("rule serde")),
            right: None,
        });
    }
    for id in ids_r.difference(&ids_l) {
        changes.push(ConfigDiffChange {
            path: format!("routes[{id}]"),
            kind: "added",
            left: None,
            right: Some(serde_json::to_value(rm[id]).expect("rule serde")),
        });
    }
    for id in ids_l.intersection(&ids_r) {
        let vl = serde_json::to_value(lm[id]).expect("rule serde");
        let vr = serde_json::to_value(rm[id]).expect("rule serde");
        if vl != vr {
            changes.push(ConfigDiffChange {
                path: format!("routes[{id}]"),
                kind: "modified",
                left: Some(vl),
                right: Some(vr),
            });
        }
    }
}

fn diff_json_leaf(path: &str, a: &Value, b: &Value, out: &mut Vec<ConfigDiffChange>) {
    if a == b {
        return;
    }
    match (a, b) {
        (Value::Object(ma), Value::Object(mb)) => {
            let keys: HashSet<String> = ma.keys().chain(mb.keys()).cloned().collect();
            for k in keys {
                let p = if path.is_empty() {
                    k.clone()
                } else {
                    format!("{path}.{k}")
                };
                match (ma.get(&k), mb.get(&k)) {
                    (Some(va), Some(vb)) => diff_json_leaf(&p, va, vb, out),
                    (None, Some(vb)) => out.push(ConfigDiffChange {
                        path: p,
                        kind: "added",
                        left: None,
                        right: Some(vb.clone()),
                    }),
                    (Some(va), None) => out.push(ConfigDiffChange {
                        path: p,
                        kind: "removed",
                        left: Some(va.clone()),
                        right: None,
                    }),
                    (None, None) => {}
                }
            }
        }
        (Value::Array(aa), Value::Array(ab)) if aa.len() != ab.len() || array_any_ne(aa, ab) => {
            out.push(ConfigDiffChange {
                path: path.to_string(),
                kind: "modified",
                left: Some(a.clone()),
                right: Some(b.clone()),
            });
        }
        _ => {
            out.push(ConfigDiffChange {
                path: path.to_string(),
                kind: "modified",
                left: Some(a.clone()),
                right: Some(b.clone()),
            });
        }
    }
}

fn array_any_ne(aa: &[Value], ab: &[Value]) -> bool {
    aa.len() == ab.len()
        && aa
            .iter()
            .zip(ab.iter())
            .any(|(x, y)| x != y)
}

impl ConfigDiffReport {
    pub fn format_text(&self) -> String {
        if self.identical {
            return format!(
                "config-diff: no differences between {} and {}",
                self.left_path, self.right_path
            );
        }
        let mut s = format!(
            "config-diff: {} difference(s) between {} and {}\n",
            self.changes.len(),
            self.left_path,
            self.right_path
        );
        for c in &self.changes {
            match c.kind {
                "added" => s.push_str(&format!("+ {}  (only in right)\n", c.path)),
                "removed" => s.push_str(&format!("- {}  (only in left)\n", c.path)),
                "modified" => s.push_str(&format!("~ {}\n", c.path)),
                _ => s.push_str(&format!("? {} {:?}\n", c.path, c.kind)),
            }
        }
        s
    }

    pub fn format_markdown(&self) -> String {
        if self.identical {
            return format!(
                "### Config diff\n\nNo differences between `{}` and `{}`.",
                self.left_path, self.right_path
            );
        }
        let mut s = format!(
            "### Config diff\n\nComparing `{}` (left/base) -> `{}` (right/change); {} change(s).\n\n",
            self.left_path,
            self.right_path,
            self.changes.len()
        );
        for c in &self.changes {
            match c.kind {
                "added" => s.push_str(&format!("- **{}** -- added on right only\n", c.path)),
                "removed" => s.push_str(&format!(
                    "- **{}** -- removed on right (present only on left)\n",
                    c.path
                )),
                "modified" => s.push_str(&format!("- **{}** -- modified\n", c.path)),
                _ => s.push_str(&format!("- **{}** -- {:?}\n", c.path, c.kind)),
            }
        }
        s
    }
}
"""

def main() -> None:
    root = Path(__file__).resolve().parents[1]
    out = root / "src" / "config" / "diff.rs"
    out.write_text(RUST.strip() + "\n", encoding="utf-8", newline="\n")
    print("wrote", out)


if __name__ == "__main__":
    main()
