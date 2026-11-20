//! Ticket storage. The `Backend` trait is the seam for further backends
//! (e.g. GitHub Issues); only the file backend exists today.

use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::thread::sleep;
use std::time::{Duration, SystemTime};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Event {
    pub at: String,
    pub role: String,
    pub action: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub detail: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub evidence: String,
    /// SHA-256 of the evidence file at the moment the event was recorded.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub sha256: String,
}

/// An addition to a ticket. Tickets are never rewritten; they only grow.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Amendment {
    pub n: u32,
    pub at: String,
    /// `clarification` or `criterion`.
    pub kind: String,
    pub text: String,
}

/// Written by the server when a ticket is closed: what the closure rests on.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Receipt {
    pub closed_at: String,
    /// Git commit at closing time, or `none`.
    pub commit: String,
    /// `yes`, `no` or `unknown`. Ticket files themselves are not counted.
    pub tree_clean: String,
    /// Hash over title, scope, spec reference, role, criteria and amendments.
    pub ticket_sha256: String,
    pub verdict_at: String,
    pub verdict_evidence: String,
    pub verdict_sha256: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub audit_at: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub audit_evidence: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub audit_sha256: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub security_at: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub security_evidence: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub security_sha256: String,
}

fn default_role() -> String {
    "implementer".to_string()
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Ticket {
    pub id: String,
    pub title: String,
    pub status: String,
    pub priority: String,
    /// The role that builds the ticket: `implementer` or `tester`.
    #[serde(default = "default_role")]
    pub role: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub supersedes: String,
    #[serde(default)]
    pub scope: String,
    #[serde(default)]
    pub spec_ref: String,
    #[serde(default)]
    pub acceptance: Vec<String>,
    #[serde(default)]
    pub audit_required: bool,
    /// Needs a PASS of the security role before it can close.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub security_required: bool,
    pub created: String,
    pub updated: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub amendments: Vec<Amendment>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub receipt: Option<Receipt>,
    #[serde(default)]
    pub history: Vec<Event>,
}

pub trait Backend {
    fn list(&self) -> Result<Vec<Ticket>, String>;
    fn load(&self, id: &str) -> Result<Ticket, String>;
    /// Run `f` on the ticket under an exclusive lock and persist the result.
    fn update(
        &self,
        id: &str,
        f: &mut dyn FnMut(&mut Ticket) -> Result<(), String>,
    ) -> Result<Ticket, String>;
    /// Allocate the next id under lock, build the ticket and persist it.
    fn create(&self, build: &mut dyn FnMut(String) -> Ticket) -> Result<Ticket, String>;
}

/// Ticket ids are `T-` plus at least four digits. Checked on every access so an
/// id can never be used to address a file outside the tasks directory.
pub fn valid_id(id: &str) -> bool {
    match id.strip_prefix("T-") {
        Some(num) => num.len() >= 4 && num.len() <= 9 && num.bytes().all(|b| b.is_ascii_digit()),
        None => false,
    }
}

pub struct FileBackend {
    dir: PathBuf,
}

impl FileBackend {
    pub fn new(project_root: &Path) -> Self {
        Self {
            dir: project_root.join(".incipit").join("tasks"),
        }
    }

    fn path(&self, id: &str) -> Result<PathBuf, String> {
        if !valid_id(id) {
            return Err(format!("invalid ticket id '{id}' (expected T-0001)"));
        }
        Ok(self.dir.join(format!("{id}.json")))
    }

    fn read(&self, id: &str) -> Result<Ticket, String> {
        let path = self.path(id)?;
        let raw = fs::read_to_string(&path).map_err(|_| format!("ticket {id} not found"))?;
        serde_json::from_str(&raw).map_err(|e| format!("ticket {id} is unreadable: {e}"))
    }

    /// Write to a temp file and rename, so a crash never leaves half a ticket.
    fn write(&self, ticket: &Ticket) -> Result<(), String> {
        let path = self.path(&ticket.id)?;
        let tmp = self.dir.join(format!(".{}.tmp", ticket.id));
        let body = serde_json::to_string_pretty(ticket).map_err(|e| e.to_string())?;
        let mut file = fs::File::create(&tmp).map_err(|e| format!("cannot write ticket: {e}"))?;
        file.write_all(body.as_bytes())
            .and_then(|_| file.write_all(b"\n"))
            .and_then(|_| file.sync_all())
            .map_err(|e| format!("cannot write ticket: {e}"))?;
        fs::rename(&tmp, &path).map_err(|e| format!("cannot write ticket: {e}"))
    }

    fn lock(&self) -> Result<LockGuard, String> {
        fs::create_dir_all(&self.dir).map_err(|e| format!("cannot create tasks dir: {e}"))?;
        let path = self.dir.join(".lock");
        for _ in 0..200 {
            match fs::OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(_) => return Ok(LockGuard { path }),
                Err(_) => {
                    // A lock older than 10 s belongs to a process that died.
                    let stale = fs::metadata(&path)
                        .and_then(|m| m.modified())
                        .ok()
                        .and_then(|t| SystemTime::now().duration_since(t).ok())
                        .map(|age| age > Duration::from_secs(10))
                        .unwrap_or(false);
                    if stale {
                        let _ = fs::remove_file(&path);
                    } else {
                        sleep(Duration::from_millis(15));
                    }
                }
            }
        }
        Err("ticket store is locked by another process".into())
    }
}

struct LockGuard {
    path: PathBuf,
}

impl Drop for LockGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

impl Backend for FileBackend {
    fn list(&self) -> Result<Vec<Ticket>, String> {
        let mut out = Vec::new();
        let entries = match fs::read_dir(&self.dir) {
            Ok(e) => e,
            Err(_) => return Ok(out),
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if let Some(id) = name.strip_suffix(".json") {
                if valid_id(id) {
                    out.push(self.read(id)?);
                }
            }
        }
        out.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(out)
    }

    fn load(&self, id: &str) -> Result<Ticket, String> {
        self.read(id)
    }

    fn update(
        &self,
        id: &str,
        f: &mut dyn FnMut(&mut Ticket) -> Result<(), String>,
    ) -> Result<Ticket, String> {
        let _guard = self.lock()?;
        let mut ticket = self.read(id)?;
        f(&mut ticket)?;
        ticket.updated = crate::util::now_iso();
        self.write(&ticket)?;
        Ok(ticket)
    }

    fn create(&self, build: &mut dyn FnMut(String) -> Ticket) -> Result<Ticket, String> {
        let _guard = self.lock()?;
        let max = self
            .list()?
            .iter()
            .filter_map(|t| t.id.strip_prefix("T-").and_then(|n| n.parse::<u32>().ok()))
            .max()
            .unwrap_or(0);
        let ticket = build(format!("T-{:04}", max + 1));
        self.write(&ticket)?;
        Ok(ticket)
    }
}

/// Pick the backend named in `.incipit/config.json`.
pub fn open(project_root: &Path) -> Result<Box<dyn Backend>, String> {
    let name = crate::config::load(project_root)
        .map(|c| c.backend)
        .unwrap_or_else(|_| "files".to_string());
    match name.as_str() {
        "files" | "" => Ok(Box::new(FileBackend::new(project_root))),
        other => Err(format!(
            "ticket backend '{other}' is not implemented; set \"backend\": \"files\" in .incipit/config.json"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_validation_blocks_traversal() {
        assert!(valid_id("T-0001"));
        assert!(valid_id("T-12345"));
        assert!(!valid_id("T-1"));
        assert!(!valid_id("../T-0001"));
        assert!(!valid_id("T-0001/../../x"));
        assert!(!valid_id("T-00a1"));
    }
}
