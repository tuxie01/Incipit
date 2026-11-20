//! Owner commands: setup phases and switching the lead on or off.

use serde_json::{json, Map, Value};
use std::fs;
use std::path::Path;

pub const PHASES: [&str; 5] = ["scaffold", "grill", "toolchain", "team", "activate"];

fn read_json(path: &Path) -> Result<Value, String> {
    match fs::read_to_string(path) {
        Ok(raw) => serde_json::from_str(&raw).map_err(|e| format!("invalid {}: {e}", path.display())),
        Err(_) => Ok(json!({})),
    }
}

fn write_json(path: &Path, value: &Value) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let body = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    fs::write(path, body + "\n").map_err(|e| format!("cannot write {}: {e}", path.display()))
}

fn phases_of(state: &Value) -> Map<String, Value> {
    state.get("phases").and_then(Value::as_object).cloned().unwrap_or_default()
}

fn is_done(phases: &Map<String, Value>, name: &str) -> bool {
    phases.get(name).and_then(Value::as_str) == Some("done")
}

/// Files the scaffold phase must have produced. The state file alone is not
/// trusted: a folder can be emptied by hand while `.incipit/state.json`
/// (hidden in file managers) still says "done".
const SCAFFOLD_FILES: [&str; 11] = [
    "CLAUDE.md",
    "SPEC.md",
    ".mcp.json",
    ".claude/settings.json",
    ".claude/agents/lead.md",
    ".claude/agents/architect.md",
    ".claude/agents/implementer.md",
    ".claude/agents/tester.md",
    ".claude/agents/auditor.md",
    ".incipit/config.json",
    ".incipit/protocol.md",
];

fn missing_scaffold(root: &Path) -> Vec<&'static str> {
    SCAFFOLD_FILES
        .iter()
        .copied()
        .filter(|rel| !root.join(rel).is_file())
        .collect()
}

/// A phase counts only if it is marked done and, for the scaffold, its files exist.
fn effective(phases: &Map<String, Value>, missing: &[&str], name: &str) -> bool {
    is_done(phases, name) && (name != "scaffold" || missing.is_empty())
}

pub fn phase(root: &Path, args: &[String]) -> Result<String, String> {
    let path = root.join(".incipit").join("state.json");
    let mut state = read_json(&path)?;
    let mut phases = phases_of(&state);
    let action = args.first().map(String::as_str).unwrap_or("show");
    match action {
        "show" => {}
        "done" | "reset" => {
            let name = args.get(1).ok_or("usage: incipit phase done|reset <name>")?;
            let pos = PHASES
                .iter()
                .position(|p| p == name)
                .ok_or_else(|| format!("unknown phase '{name}' ({})", PHASES.join(", ")))?;
            if action == "done" {
                let missing = missing_scaffold(root);
                if !missing.is_empty() {
                    return Err(format!(
                        "the scaffold is incomplete (missing: {}); run scaffold.sh again, it keeps existing files",
                        missing.join(", ")
                    ));
                }
                if let Some(open) = PHASES[..pos].iter().find(|p| !is_done(&phases, p)) {
                    return Err(format!("phase '{open}' must be done before '{name}'"));
                }
                phases.insert(name.clone(), json!("done"));
            } else {
                // Resetting a phase also resets everything that builds on it.
                for later in &PHASES[pos..] {
                    phases.remove(*later);
                }
            }
            state["phases"] = Value::Object(phases.clone());
            write_json(&path, &state)?;
        }
        other => return Err(format!("unknown phase action '{other}' (show, done, reset)")),
    }
    let missing = missing_scaffold(root);
    let mut out = String::new();
    for name in PHASES {
        if name == "scaffold" && is_done(&phases, name) && !missing.is_empty() {
            out.push_str(&format!("scaffold: incomplete (missing: {})\n", missing.join(", ")));
        } else {
            out.push_str(&format!("{name}: {}\n", if is_done(&phases, name) { "done" } else { "pending" }));
        }
    }
    match PHASES.iter().find(|p| !effective(&phases, &missing, p)) {
        Some(next) => out.push_str(&format!("next: {next}\n")),
        None => out.push_str("next: -\n"),
    }
    Ok(out)
}

/// Make the lead agent the main session (or undo that) in `.claude/settings.json`.
pub fn set_lead(root: &Path, on: bool) -> Result<String, String> {
    let path = root.join(".claude").join("settings.json");
    let mut settings = read_json(&path)?;
    let obj = settings.as_object_mut().ok_or(".claude/settings.json is not an object")?;
    if on {
        if !root.join(".claude").join("agents").join("lead.md").is_file() {
            return Err(".claude/agents/lead.md is missing; run the scaffold phase first".into());
        }
        obj.insert("agent".into(), json!("lead"));
    } else {
        obj.remove("agent");
    }
    write_json(&path, &settings)?;
    Ok(if on {
        "lead activated: new sessions in this project start as the lead. Restart Claude Code.".into()
    } else {
        "lead deactivated: new sessions start as a plain owner session. Restart Claude Code.".into()
    })
}
