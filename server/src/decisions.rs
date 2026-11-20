//! The decision log. The lead keeps no memory between sessions and writes no
//! files, so what the owner decided and what is still open would otherwise
//! live only in a chat that is gone after the next restart. The log is
//! append-only: an entry is never changed, a question is closed by an answer
//! that refers to it.

use crate::util::now_iso;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::thread::sleep;
use std::time::Duration;

#[derive(Serialize, Deserialize, Clone)]
pub struct Entry {
    pub id: String,
    pub at: String,
    /// `decision`, `question`, `answer` or `note`.
    pub kind: String,
    pub text: String,
    /// For an answer: the question it settles. Otherwise an optional ticket or proposal id.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub r#ref: String,
}

fn file(root: &Path) -> PathBuf {
    root.join(".incipit").join("decisions.json")
}

pub fn load(root: &Path) -> Result<Vec<Entry>, String> {
    match fs::read_to_string(file(root)) {
        Ok(raw) => serde_json::from_str(&raw).map_err(|e| format!("decision log is unreadable: {e}")),
        Err(_) => Ok(Vec::new()),
    }
}

/// Questions that no answer refers to yet.
pub fn open_questions(entries: &[Entry]) -> Vec<&Entry> {
    entries
        .iter()
        .filter(|e| e.kind == "question")
        .filter(|q| !entries.iter().any(|a| a.kind == "answer" && a.r#ref == q.id))
        .collect()
}

fn line(e: &Entry) -> String {
    let reference = if e.r#ref.is_empty() { String::new() } else { format!(" [{}]", e.r#ref) };
    format!("{} {} {}{}: {}\n", e.id, &e.at[..10.min(e.at.len())], e.kind, reference, e.text)
}

/// MCP tool, lead only.
pub fn log(root: &Path, args: &Value) -> Result<String, String> {
    let get = |k: &str| args.get(k).and_then(Value::as_str).map(str::trim).unwrap_or("");
    let kind = get("kind");
    if !["decision", "question", "answer", "note"].contains(&kind) {
        return Err("kind must be decision, question, answer or note".into());
    }
    let text = crate::tools::one_line(get("text"));
    if text.is_empty() {
        return Err("missing argument 'text'".into());
    }
    let reference = get("ref").to_string();

    // One writer at a time; the lock file is removed when the guard drops.
    let lock = root.join(".incipit").join(".decisions.lock");
    let mut held = false;
    for _ in 0..200 {
        if fs::OpenOptions::new().write(true).create_new(true).open(&lock).is_ok() {
            held = true;
            break;
        }
        sleep(Duration::from_millis(15));
    }
    if !held {
        // A lock that survives three seconds belongs to a process that died.
        let _ = fs::remove_file(&lock);
    }
    let result = (|| {
        let mut entries = load(root)?;
        if kind == "answer" {
            let open = open_questions(&entries);
            if !open.iter().any(|q| q.id == reference) {
                return Err(format!(
                    "an answer needs 'ref' with the id of an open question (open: {})",
                    if open.is_empty() { "none".to_string() } else { open.iter().map(|q| q.id.as_str()).collect::<Vec<_>>().join(", ") }
                ));
            }
        }
        let id = format!("D-{:04}", entries.len() + 1);
        entries.push(Entry { id: id.clone(), at: now_iso(), kind: kind.to_string(), text, r#ref: reference });
        let body = serde_json::to_string_pretty(&entries).map_err(|e| e.to_string())?;
        let tmp = root.join(".incipit").join(".decisions.tmp");
        fs::write(&tmp, body + "\n").map_err(|e| format!("cannot write decision log: {e}"))?;
        fs::rename(&tmp, file(root)).map_err(|e| format!("cannot write decision log: {e}"))?;
        Ok(format!("logged {id} ({kind})"))
    })();
    let _ = fs::remove_file(&lock);
    result
}

fn render(entries: &[Entry], open_only: bool, last: usize) -> String {
    let open = open_questions(entries);
    let mut out = format!("open questions[{}]:\n", open.len());
    for q in &open {
        out.push_str(&line(q));
    }
    if !open_only {
        let skip = entries.len().saturating_sub(last);
        out.push_str(&format!("log[{} of {}]:\n", entries.len() - skip, entries.len()));
        for e in entries.iter().skip(skip) {
            out.push_str(&line(e));
        }
    }
    out
}

/// MCP tool, every role.
pub fn list(root: &Path, args: &Value) -> Result<String, String> {
    let entries = load(root)?;
    let open_only = args.get("open_only").and_then(Value::as_bool).unwrap_or(false);
    let last = args.get("last").and_then(Value::as_u64).unwrap_or(15) as usize;
    Ok(render(&entries, open_only, last))
}

/// Owner CLI: `incipit decisions [open|all]`.
pub fn cli(root: &Path, args: &[String]) -> Result<String, String> {
    let entries = load(root)?;
    match args.first().map(String::as_str).unwrap_or("") {
        "open" => Ok(render(&entries, true, 0)),
        "all" => Ok(render(&entries, false, usize::MAX)),
        "" => Ok(render(&entries, false, 20)),
        other => Err(format!("unknown argument '{other}' (open, all)")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(id: &str, kind: &str, r: &str) -> Entry {
        Entry { id: id.into(), at: "2026-11-20T10:00:00Z".into(), kind: kind.into(), text: "t".into(), r#ref: r.into() }
    }

    #[test]
    fn a_question_is_open_until_an_answer_refers_to_it() {
        let log = vec![e("D-0001", "question", ""), e("D-0002", "question", ""), e("D-0003", "answer", "D-0001")];
        let open = open_questions(&log);
        assert_eq!(open.len(), 1);
        assert_eq!(open[0].id, "D-0002");
    }
}
