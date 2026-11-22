//! `task_commit`: the server commits a role's own work. Workers that run in
//! parallel each write their own files; a plain `git add -A` by one of them
//! would sweep up the files of the others. The server instead picks, from the
//! changed files, exactly those that belong to the calling role, plus the
//! ticket file and the evidence of the named ticket, and commits only those.
//!
//! Ownership of a path: among the roles whose `write_allow` covers it and
//! whose `write_deny` does not, the role with the longest matching prefix
//! owns it (`docs/research/` beats `docs/`, which beats `.`). Ties are owned
//! by every tied role. Files below `.incipit/` belong to no role: the ticket
//! file and the evidence go with their ticket, and the server's own shared
//! state (decision log, spec proposals) goes with every commit.

use crate::config::{self, RoleRules};
use crate::tools::one_line;
use crate::util::under;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

/// Never committed by a role: the team's configuration and git's own data.
const NOT_FOR_ROLES: [&str; 4] = [".claude", ".mcp.json", ".git", ".incipit"];

/// Server-written shared state, committed along with any commit.
const SHARED_STATE: [&str; 2] = [".incipit/decisions.json", ".incipit/proposals"];

const COMMITTING_ROLES: [&str; 6] = ["architect", "implementer", "tester", "auditor", "security", "researcher"];

fn git(root: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .map_err(|e| format!("cannot run git: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(format!("git {} failed: {}", args.first().unwrap_or(&""), err.trim()));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Changed paths (modified, added, deleted, untracked; both sides of a rename).
pub fn changed_paths(root: &Path) -> Result<Vec<String>, String> {
    let raw = git(root, &["status", "--porcelain=v1", "-z", "--untracked-files=all"])?;
    let mut out = Vec::new();
    let mut fields = raw.split('\0').filter(|f| !f.is_empty());
    while let Some(entry) = fields.next() {
        if entry.len() < 4 {
            continue;
        }
        let (xy, path) = entry.split_at(3);
        out.push(path.to_string());
        // A rename or copy is followed by its source path.
        if xy.starts_with('R') || xy.starts_with('C') {
            if let Some(source) = fields.next() {
                out.push(source.to_string());
            }
        }
    }
    out.sort();
    out.dedup();
    Ok(out)
}

fn prefix_len(prefix: &str) -> usize {
    let p = prefix.trim_start_matches("./").trim_matches('/');
    if p == "." {
        0
    } else {
        p.len()
    }
}

/// The roles that own `path` (see the module comment).
pub fn owners(path: &str, roles: &BTreeMap<String, RoleRules>) -> Vec<String> {
    if NOT_FOR_ROLES.iter().any(|p| under(path, p)) {
        return Vec::new();
    }
    let mut best: Option<usize> = None;
    let mut owners = Vec::new();
    for (name, rules) in roles {
        if name == "lead" || rules.write_deny.iter().any(|p| under(path, p)) {
            continue;
        }
        let len = match rules.write_allow.iter().filter(|p| under(path, p)).map(|p| prefix_len(p)).max() {
            Some(l) => l,
            None => continue,
        };
        match best {
            Some(b) if len < b => {}
            Some(b) if len == b => owners.push(name.clone()),
            _ => {
                best = Some(len);
                owners = vec![name.clone()];
            }
        }
    }
    owners
}

fn belongs_to_ticket(path: &str, id: &str) -> bool {
    if path.eq_ignore_ascii_case(&format!(".incipit/tasks/{id}.json")) {
        return true;
    }
    if !under(path, ".incipit/evidence") {
        return false;
    }
    let name = path.rsplit('/').next().unwrap_or("");
    name.starts_with(&format!("{id}-")) || name.starts_with(&format!("{id}."))
}

/// Which of `changed` a commit of `role` for ticket `id` takes, and which it leaves.
pub fn select(
    changed: &[String],
    role: &str,
    id: Option<&str>,
    roles: &BTreeMap<String, RoleRules>,
) -> (Vec<String>, Vec<String>) {
    let mut take = Vec::new();
    let mut leave = Vec::new();
    for path in changed {
        let mine = owners(path, roles).iter().any(|o| o == role)
            || id.map_or(false, |id| belongs_to_ticket(path, id))
            || SHARED_STATE.iter().any(|p| under(path, p));
        if mine {
            take.push(path.clone());
        } else {
            leave.push(path.clone());
        }
    }
    (take, leave)
}

fn listing(paths: &[String], limit: usize) -> String {
    let mut out = String::new();
    for p in paths.iter().take(limit) {
        out.push_str(&format!("  {p}\n"));
    }
    if paths.len() > limit {
        out.push_str(&format!("  ... and {} more\n", paths.len() - limit));
    }
    out
}

/// MCP tool.
pub fn commit(root: &Path, store: &dyn crate::store::Backend, args: &Value) -> Result<String, String> {
    let get = |k: &str| args.get(k).and_then(Value::as_str).map(str::trim).unwrap_or("");
    let role = get("role");
    if !COMMITTING_ROLES.contains(&role) {
        return Err(format!("invalid role '{role}'"));
    }
    let message = one_line(get("message"));
    if message.is_empty() {
        return Err("missing argument 'message': one line saying what the commit contains".into());
    }
    let id = match get("id") {
        "" => None,
        id => {
            store.load(id)?;
            Some(id)
        }
    };
    let top = git(root, &["rev-parse", "--show-toplevel"])
        .map_err(|_| "this project is not a git repository".to_string())?;
    if crate::util::resolve_path(Path::new(top.trim())) != crate::util::resolve_path(root) {
        return Err("the project root is not the root of its git repository; task_commit needs both to be the same".into());
    }
    let roles = config::load(root)?.roles;
    if !roles.contains_key(role) {
        return Err(format!("the role '{role}' has no rules in .incipit/config.json"));
    }
    let changed = changed_paths(root)?;
    let (take, leave) = select(&changed, role, id, &roles);
    let own_work = take.iter().any(|p| !SHARED_STATE.iter().any(|s| under(p, s)));
    if !own_work {
        let mut msg = format!("nothing of the {role} role to commit");
        if let Some(id) = id {
            msg.push_str(&format!(" and no change to {id} or its evidence"));
        }
        if !leave.is_empty() {
            msg.push_str(&format!("; {} changed file(s) belong to other roles", leave.len()));
        }
        return Err(msg);
    }

    let specs: Vec<String> = take.iter().map(|p| format!(":(literal){p}")).collect();
    let mut add = vec!["add", "-A", "--"];
    add.extend(specs.iter().map(String::as_str));
    git(root, &add)?;
    let subject = match id {
        Some(id) => format!("{id}: {message}"),
        None => message.clone(),
    };
    let body = format!("{subject}\n\nIncipit-Role: {role}");
    let mut commit = vec!["commit", "--only", "-q", "-m", body.as_str(), "--"];
    commit.extend(specs.iter().map(String::as_str));
    git(root, &commit).map_err(|e| {
        if e.contains("user.email") || e.contains("user.name") || e.contains("identity") {
            "git has no identity for commits; the owner sets it with git config user.name and user.email".to_string()
        } else {
            e
        }
    })?;
    let hash = git(root, &["rev-parse", "--short=12", "HEAD"])?;
    let mut out = format!("committed {} ({} file(s)):\n{}", hash.trim(), take.len(), listing(&take, 30));
    if !leave.is_empty() {
        out.push_str(&format!("left uncommitted, not yours ({}):\n{}", leave.len(), listing(&leave, 10)));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roles() -> BTreeMap<String, RoleRules> {
        let mut m = BTreeMap::new();
        let r = |allow: &[&str], deny: &[&str]| RoleRules {
            write_allow: allow.iter().map(|s| s.to_string()).collect(),
            write_deny: deny.iter().map(|s| s.to_string()).collect(),
        };
        m.insert("lead".into(), r(&[], &[]));
        m.insert("architect".into(), r(&["docs/"], &["docs/test-plan.md"]));
        m.insert("implementer".into(), r(&["."], &["tests/", ".incipit/", "docs/test-plan.md"]));
        m.insert("tester".into(), r(&["tests/", ".incipit/evidence/", "docs/test-plan.md"], &[]));
        m.insert("auditor".into(), r(&[".incipit/evidence/"], &[]));
        m.insert("researcher".into(), r(&["docs/research/"], &[]));
        m
    }

    #[test]
    fn the_longest_prefix_owns_a_path() {
        let r = roles();
        assert_eq!(owners("src/main.c", &r), vec!["implementer"]);
        assert_eq!(owners("docs/design/sys.md", &r), vec!["architect"]);
        assert_eq!(owners("docs/research/thing.md", &r), vec!["researcher"]);
        assert_eq!(owners("docs/test-plan.md", &r), vec!["tester"]);
        assert_eq!(owners("tests/run.sh", &r), vec!["tester"]);
        assert!(owners(".incipit/evidence/T-0001-test.md", &r).is_empty());
        assert!(owners(".claude/agents/lead.md", &r).is_empty());
        assert!(owners(".git/config", &r).is_empty());
    }

    #[test]
    fn a_commit_takes_own_files_and_its_ticket_only() {
        let r = roles();
        let changed: Vec<String> = [
            "src/main.c",
            "docs/research/thing.md",
            "docs/design/sys.md",
            ".incipit/tasks/T-0012.json",
            ".incipit/tasks/T-0008.json",
            ".incipit/evidence/T-0012-test.md",
            ".incipit/evidence/T-0008-audit.md",
            ".incipit/decisions.json",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let (take, leave) = select(&changed, "implementer", Some("T-0012"), &r);
        assert_eq!(take, vec!["src/main.c", ".incipit/tasks/T-0012.json", ".incipit/evidence/T-0012-test.md", ".incipit/decisions.json"]);
        assert_eq!(leave.len(), 4);
        let (take, _) = select(&changed, "researcher", None, &r);
        assert_eq!(take, vec!["docs/research/thing.md", ".incipit/decisions.json"]);
        assert!(!belongs_to_ticket(".incipit/evidence/T-00123-test.md", "T-0012"));
    }
}
