//! Accessibility support that is not in a view.
//!
//! - [`start`] redraws a window once when an assistive client connects to it.
//!   GPUI forces one frame on activation, but in that frame the window still
//!   counts as inactive, so it sends no tree. A window that never draws again
//!   (the Hub on a still page) then shows VoiceOver and Cua an empty window.
//! - With `SAYSO_A11Y_AUDIT=1`, the audit reads the tree of each window every
//!   two seconds and logs each problem once, with the source location of the
//!   element in a debug build. Unit tests cannot see the tree: test windows
//!   never activate it. See `docs/accessibility.md`.

use gpui_kit::App;
use serde_json::Value;
use std::collections::{HashMap, HashSet};

pub fn start(cx: &mut App) {
    let audit = std::env::var_os("SAYSO_A11Y_AUDIT").is_some();
    if audit {
        log::info!("a11y audit: on");
    }
    cx.spawn(async move |cx| {
        let mut active: HashSet<gpui_kit::WindowId> = HashSet::new();
        let mut seen = HashSet::new();
        let mut tick = 0u32;
        loop {
            cx.background_executor().timer(std::time::Duration::from_millis(500)).await;
            tick += 1;
            let trees = cx.update(|cx| {
                let mut trees = Vec::new();
                for w in cx.windows() {
                    let _ = w.update(cx, |_, window, _| {
                        let id = w.window_id();
                        if window.is_a11y_active() {
                            // Newly active: draw once more, so this frame sends the tree.
                            if active.insert(id) {
                                log::debug!("a11y: redraw {id:?} for the first tree");
                                window.refresh();
                            }
                        } else {
                            active.remove(&id);
                        }
                        if audit && tick.is_multiple_of(4) && let Some(tree) = window.debug_a11y_tree_json() {
                            trees.push(tree);
                        }
                    });
                }
                trees
            });
            for tree in trees {
                let title = serde_json::from_str::<Value>(&tree)
                    .ok()
                    .and_then(|t| t.pointer("/frame/window_title").and_then(Value::as_str).map(str::to_string))
                    .unwrap_or_default();
                if seen.insert(format!("window {title}")) {
                    log::info!("a11y audit: reading the window \"{title}\"");
                }
                // `SAYSO_A11Y_DUMP=<folder>` writes the last tree of each window there.
                if let Some(dir) = std::env::var_os("SAYSO_A11Y_DUMP").filter(|d| !d.is_empty()) {
                    let name = if title.is_empty() { "untitled".to_string() } else { title.replace('/', "-") };
                    let _ = std::fs::write(std::path::Path::new(&dir).join(format!("{name}.json")), &tree);
                }
                for problem in check(&tree) {
                    if seen.insert(problem.clone()) {
                        log::warn!("a11y audit: {problem}");
                    }
                }
            }
        }
    })
    .detach();
}

/// Roles that a screen reader announces with a name, and that are useless
/// without one.
const NEEDS_NAME: &[&str] = &[
    "Button",
    "CheckBox",
    "Switch",
    "RadioButton",
    "Tab",
    "Slider",
    "SpinButton",
    "ComboBox",
    "Link",
    "MenuItem",
    "ProgressIndicator",
    "TextInput",
    "MultilineTextInput",
    "PasswordInput",
    "SearchInput",
    "RadioGroup",
    "TabList",
    "Dialog",
    "Heading",
];

/// The problems in one tree from `Window::debug_a11y_tree_json`.
pub fn check(tree: &str) -> Vec<String> {
    let Ok(tree) = serde_json::from_str::<Value>(tree) else {
        return vec!["the tree is not JSON".into()];
    };
    let Some(nodes) = tree.get("nodes").and_then(Value::as_object) else {
        return Vec::new();
    };
    let mut parent: HashMap<&str, &str> = HashMap::new();
    for (id, node) in nodes {
        for child in node.get("children").and_then(Value::as_array).into_iter().flatten() {
            if let Some(child) = child.as_str() {
                parent.insert(child, id);
            }
        }
    }
    let aria = |id: &str| nodes.get(id).and_then(|n| n.get("aria"));
    let text = |id: &str, key: &str| aria(id).and_then(|a| a.get(key)).and_then(Value::as_str).filter(|s| !s.trim().is_empty()).is_some();
    let role = |id: &str| aria(id).and_then(|a| a.get("role")).and_then(Value::as_str).unwrap_or("").to_string();
    // A name can also come from text inside the node.
    fn has_text_inside(id: &str, nodes: &serde_json::Map<String, Value>) -> bool {
        let Some(node) = nodes.get(id) else { return false };
        node.get("children").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).any(|child| {
            let named = nodes
                .get(child)
                .and_then(|n| n.get("aria"))
                .is_some_and(|a| ["label", "value"].iter().any(|k| a.get(*k).and_then(Value::as_str).is_some_and(|s| !s.trim().is_empty())));
            named || has_text_inside(child, nodes)
        })
    }
    let has_ancestor = |id: &str, wanted: &str| {
        let mut at = id;
        while let Some(p) = parent.get(at) {
            if role(p) == wanted {
                return true;
            }
            at = p;
        }
        false
    };
    let place = |id: &str| {
        let node = &nodes[id];
        let at = node.get("source_location").and_then(Value::as_str).unwrap_or("unknown place");
        match node.get("element_id").and_then(Value::as_str) {
            Some(el) => format!("{at} ({el})"),
            None => at.to_string(),
        }
    };

    let mut problems = Vec::new();
    for id in nodes.keys() {
        let r = role(id);
        if NEEDS_NAME.contains(&r.as_str()) && !text(id, "label") && !text(id, "value") && !text(id, "placeholder") && !has_text_inside(id, nodes) {
            problems.push(format!("{r} without a name at {}", place(id)));
        }
        if r == "RadioButton" && !has_ancestor(id, "RadioGroup") {
            problems.push(format!("RadioButton outside a RadioGroup at {}", place(id)));
        }
        if r == "Tab" && !has_ancestor(id, "TabList") {
            problems.push(format!("Tab outside a TabList at {}", place(id)));
        }
    }
    problems.sort();
    problems
}

#[cfg(test)]
mod tests {
    use super::check;

    fn tree(nodes: &str) -> String {
        format!(r#"{{"root":"a","nodes":{{"a":{{"children":["b","c","d","e"],"aria":{{"role":"Window","label":"Sayso"}}}},{nodes}}}}}"#)
    }

    #[test]
    fn a_named_tree_has_no_problems() {
        let t = tree(
            r#""b":{"aria":{"role":"Button","label":"Save"}},
               "c":{"children":["f"],"aria":{"role":"RadioGroup","label":"Theme"}},
               "f":{"aria":{"role":"RadioButton","label":"Dark"}},
               "d":{"children":["g"],"aria":{"role":"Button"}},
               "g":{"aria":{"role":"Label","value":"Get started"}},
               "e":{"aria":{"role":"Heading","label":"Home"}}"#,
        );
        assert_eq!(check(&t), Vec::<String>::new());
    }

    #[test]
    fn finds_unnamed_controls_and_orphans() {
        let t = tree(
            r#""b":{"aria":{"role":"Button"},"source_location":"src/x.rs:1:1","element_id":"save"},
               "c":{"aria":{"role":"RadioButton","label":"Dark"},"source_location":"src/x.rs:2:1"},
               "d":{"aria":{"role":"Tab","label":"Home"},"source_location":"src/x.rs:3:1"},
               "e":{"aria":{"role":"Switch","label":"  "},"source_location":"src/x.rs:4:1"}"#,
        );
        assert_eq!(
            check(&t),
            vec![
                "Button without a name at src/x.rs:1:1 (save)",
                "RadioButton outside a RadioGroup at src/x.rs:2:1",
                "Switch without a name at src/x.rs:4:1",
                "Tab outside a TabList at src/x.rs:3:1",
            ]
        );
    }

    #[test]
    fn bad_json_is_one_problem() {
        assert_eq!(check("{"), vec!["the tree is not JSON".to_string()]);
    }
}
