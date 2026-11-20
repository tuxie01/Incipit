//! Spec change proposals. No agent can write SPEC.md; the architect files a
//! proposal of exact replacements and the owner applies or rejects it with
//! the CLI. A proposal is applied completely or not at all.

use crate::util::now_iso;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Clone)]
pub struct Edit {
    pub old: String,
    pub new: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Proposal {
    pub id: String,
    pub status: String,
    pub created: String,
    pub reason: String,
    pub edits: Vec<Edit>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub decided: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
}

fn dir(root: &Path) -> PathBuf {
    root.join(".incipit").join("proposals")
}

fn valid_id(id: &str) -> bool {
    match id.strip_prefix("P-") {
        Some(n) => n.len() >= 4 && n.len() <= 9 && n.bytes().all(|b| b.is_ascii_digit()),
        None => false,
    }
}

fn path(root: &Path, id: &str) -> Result<PathBuf, String> {
    if !valid_id(id) {
        return Err(format!("invalid proposal id '{id}' (expected P-0001)"));
    }
    Ok(dir(root).join(format!("{id}.json")))
}

fn load(root: &Path, id: &str) -> Result<Proposal, String> {
    let raw = fs::read_to_string(path(root, id)?).map_err(|_| format!("proposal {id} not found"))?;
    serde_json::from_str(&raw).map_err(|e| format!("proposal {id} is unreadable: {e}"))
}

fn save(root: &Path, p: &Proposal) -> Result<(), String> {
    let target = path(root, &p.id)?;
    let tmp = dir(root).join(format!(".{}.tmp", p.id));
    let body = serde_json::to_string_pretty(p).map_err(|e| e.to_string())?;
    fs::write(&tmp, body + "\n").map_err(|e| format!("cannot write proposal: {e}"))?;
    fs::rename(&tmp, &target).map_err(|e| format!("cannot write proposal: {e}"))
}

fn all(root: &Path) -> Vec<Proposal> {
    let mut out = Vec::new();
    if let Ok(entries) = fs::read_dir(dir(root)) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if let Some(id) = name.strip_suffix(".json") {
                if let Ok(p) = load(root, id) {
                    out.push(p);
                }
            }
        }
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

/// Proposals the owner has not decided yet: id and reason.
pub fn pending(root: &Path) -> Vec<(String, String)> {
    all(root)
        .into_iter()
        .filter(|p| p.status == "pending")
        .map(|p| (p.id, p.reason))
        .collect()
}

/// Apply all edits to `text` in order. Each `old` must occur exactly once at
/// the moment it is applied.
fn apply_edits(text: &str, edits: &[Edit]) -> Result<String, String> {
    let mut out = text.to_string();
    for (i, edit) in edits.iter().enumerate() {
        let hits = out.matches(edit.old.as_str()).count();
        if hits != 1 {
            return Err(format!(
                "edit {}: the passage to replace occurs {hits} times in SPEC.md, it must occur exactly once",
                i + 1
            ));
        }
        out = out.replacen(edit.old.as_str(), edit.new.as_str(), 1);
    }
    Ok(out)
}

/// MCP tool, architect only.
pub fn propose(root: &Path, args: &Value) -> Result<String, String> {
    let reason = args
        .get("reason")
        .and_then(Value::as_str)
        .map(crate::tools::one_line)
        .filter(|r| !r.is_empty())
        .ok_or("missing argument 'reason'")?;
    let raw = args.get("edits").and_then(Value::as_array).ok_or("missing argument 'edits'")?;
    let mut edits = Vec::new();
    for (i, e) in raw.iter().enumerate() {
        let old = e.get("old").and_then(Value::as_str).unwrap_or("");
        let new = e.get("new").and_then(Value::as_str).unwrap_or("");
        if old.is_empty() {
            return Err(format!("edit {}: 'old' is empty", i + 1));
        }
        if old == new {
            return Err(format!("edit {}: 'old' and 'new' are identical", i + 1));
        }
        edits.push(Edit { old: old.to_string(), new: new.to_string() });
    }
    if edits.is_empty() {
        return Err("'edits' needs at least one entry".into());
    }
    let spec = fs::read_to_string(root.join("SPEC.md")).map_err(|_| "SPEC.md not found".to_string())?;
    // Refuse now what could not be applied later.
    apply_edits(&spec, &edits)?;

    fs::create_dir_all(dir(root)).map_err(|e| format!("cannot create proposals dir: {e}"))?;
    let next = all(root)
        .iter()
        .filter_map(|p| p.id.strip_prefix("P-").and_then(|n| n.parse::<u32>().ok()))
        .max()
        .unwrap_or(0)
        + 1;
    let proposal = Proposal {
        id: format!("P-{next:04}"),
        status: "pending".into(),
        created: now_iso(),
        reason,
        edits,
        decided: String::new(),
        note: String::new(),
    };
    save(root, &proposal)?;
    Ok(format!(
        "proposal {} filed with {} edit(s). The owner applies it with: incipit spec apply {}",
        proposal.id,
        proposal.edits.len(),
        proposal.id
    ))
}

fn indent(text: &str, mark: &str) -> String {
    text.lines().map(|l| format!("  {mark} {l}\n")).collect()
}

/// Owner CLI: `incipit spec list | show <id> | apply <id> | reject <id> [reason]`.
pub fn cli(root: &Path, args: &[String]) -> Result<String, String> {
    match args.first().map(String::as_str).unwrap_or("list") {
        "list" => {
            let items = all(root);
            if items.is_empty() {
                return Ok("no proposals".into());
            }
            let mut out = String::new();
            for p in items {
                out.push_str(&format!("{} {} {} edit(s) | {}\n", p.id, p.status, p.edits.len(), p.reason));
            }
            Ok(out)
        }
        "show" => {
            let p = load(root, args.get(1).ok_or("usage: incipit spec show <id>")?)?;
            let mut out = format!("{} ({})\nfiled: {}\nreason: {}\n", p.id, p.status, p.created, p.reason);
            for (i, e) in p.edits.iter().enumerate() {
                out.push_str(&format!("\nedit {}:\n{}{}", i + 1, indent(&e.old, "-"), indent(&e.new, "+")));
            }
            Ok(out)
        }
        "apply" => {
            let mut p = load(root, args.get(1).ok_or("usage: incipit spec apply <id>")?)?;
            if p.status != "pending" {
                return Err(format!("{} is already {}", p.id, p.status));
            }
            let spec_path = root.join("SPEC.md");
            let spec = fs::read_to_string(&spec_path).map_err(|_| "SPEC.md not found".to_string())?;
            let updated = apply_edits(&spec, &p.edits)
                .map_err(|e| format!("{e}. SPEC.md has changed since the proposal was filed; nothing was applied"))?;
            fs::write(&spec_path, updated).map_err(|e| format!("cannot write SPEC.md: {e}"))?;
            p.status = "applied".into();
            p.decided = now_iso();
            save(root, &p)?;
            Ok(format!("{} applied to SPEC.md ({} edit(s)). Review with git diff, then commit.", p.id, p.edits.len()))
        }
        "reject" => {
            let mut p = load(root, args.get(1).ok_or("usage: incipit spec reject <id> [reason]")?)?;
            if p.status != "pending" {
                return Err(format!("{} is already {}", p.id, p.status));
            }
            p.status = "rejected".into();
            p.decided = now_iso();
            p.note = crate::tools::one_line(&args[2..].join(" "));
            save(root, &p)?;
            Ok(format!("{} rejected", p.id))
        }
        other => Err(format!("unknown spec action '{other}' (list, show, apply, reject)")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(old: &str, new: &str) -> Edit {
        Edit { old: old.into(), new: new.into() }
    }

    #[test]
    fn edits_need_a_unique_passage() {
        assert_eq!(apply_edits("a b c", &[e("b", "x")]).unwrap(), "a x c");
        assert!(apply_edits("a b b", &[e("b", "x")]).is_err());
        assert!(apply_edits("a b c", &[e("z", "x")]).is_err());
    }

    #[test]
    fn edits_apply_in_order_or_not_at_all() {
        assert_eq!(apply_edits("one two", &[e("one", "1"), e("two", "2")]).unwrap(), "1 2");
        assert!(apply_edits("one two", &[e("one", "1"), e("three", "3")]).is_err());
    }
}
