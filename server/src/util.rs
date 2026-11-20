//! Small helpers: timestamps, project discovery, path normalisation.

use std::env;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// UTC timestamp in ISO 8601, second precision. No external crate needed.
pub fn now_iso() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    iso_from_unix(secs)
}

pub fn iso_from_unix(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let (h, m, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    // Civil-from-days (Howard Hinnant).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if mo <= 2 { 1 } else { 0 };
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{m:02}:{s:02}Z")
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(bytes);
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

/// Current commit and whether the working tree is clean. Ticket files and
/// proposals are left out: the server itself changes them all the time.
pub fn git_state(root: &Path) -> (String, String) {
    use std::process::Command;
    let run = |args: &[&str]| {
        Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
    };
    let commit = match run(&["rev-parse", "HEAD"]) {
        Some(c) if !c.is_empty() => c,
        _ => return ("none".into(), "unknown".into()),
    };
    let clean = match run(&[
        "status",
        "--porcelain",
        "--",
        ".",
        ":(exclude).incipit/tasks",
        ":(exclude).incipit/proposals",
    ]) {
        Some(out) if out.is_empty() => "yes",
        Some(_) => "no",
        None => "unknown",
    };
    (commit, clean.into())
}

/// Project root: explicit env, else the nearest ancestor holding `.incipit/`, else cwd.
pub fn project_root() -> PathBuf {
    for var in ["INCIPIT_PROJECT", "CLAUDE_PROJECT_DIR"] {
        if let Ok(v) = env::var(var) {
            if !v.is_empty() {
                return PathBuf::from(v);
            }
        }
    }
    let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let mut cur: Option<&Path> = Some(cwd.as_path());
    while let Some(dir) = cur {
        if dir.join(".incipit").is_dir() {
            return dir.to_path_buf();
        }
        cur = dir.parent();
    }
    cwd
}

/// Resolve `.` and `..` lexically, without touching the filesystem.
pub fn lexical_normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    out.push("..");
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Canonicalise the deepest existing ancestor (resolves symlinks) and re-append the
/// not-yet-existing remainder. Works for files that are about to be created.
pub fn resolve_path(path: &Path) -> PathBuf {
    let norm = lexical_normalize(path);
    let mut existing = norm.clone();
    let mut tail: Vec<std::ffi::OsString> = Vec::new();
    loop {
        if let Ok(canon) = existing.canonicalize() {
            let mut out = canon;
            for part in tail.iter().rev() {
                out.push(part);
            }
            return out;
        }
        match existing.file_name() {
            Some(name) => {
                tail.push(name.to_os_string());
                existing.pop();
            }
            None => return norm,
        }
    }
}

/// Path of `target` relative to `root`, with `/` separators, or None if outside.
pub fn relative_to(root: &Path, target: &Path) -> Option<String> {
    let root = resolve_path(root);
    let target = resolve_path(target);
    let rel = target.strip_prefix(&root).ok()?;
    let parts: Vec<String> = rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    Some(parts.join("/"))
}

/// True if `rel` equals `prefix` or lies below it. Case-insensitive, because the
/// default macOS filesystem is, and a guard must not be bypassed by `.Incipit/`.
pub fn under(rel: &str, prefix: &str) -> bool {
    let rel = rel.trim_matches('/').to_lowercase();
    let prefix = prefix.trim_start_matches("./").trim_matches('/').to_lowercase();
    if prefix.is_empty() || prefix == "." {
        return true;
    }
    rel == prefix || rel.starts_with(&format!("{prefix}/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_epoch_and_known_date() {
        assert_eq!(iso_from_unix(0), "1970-01-01T00:00:00Z");
        assert_eq!(iso_from_unix(1_700_000_000), "2023-11-14T22:13:20Z");
        assert_eq!(iso_from_unix(951_782_400), "2000-02-29T00:00:00Z");
    }

    #[test]
    fn sha256_known_vectors() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn normalize_removes_dot_segments() {
        assert_eq!(
            lexical_normalize(Path::new("/a/b/../c/./d")),
            PathBuf::from("/a/c/d")
        );
    }

    #[test]
    fn under_is_prefix_aware_and_case_insensitive() {
        assert!(under(".incipit/tasks/T-0001.json", ".incipit/tasks"));
        assert!(under(".Incipit/Tasks/x", ".incipit/tasks"));
        assert!(!under(".incipit/tasks-old/x", ".incipit/tasks"));
        assert!(under("src/main.rs", "."));
        assert!(under("tests/a.rs", "tests/"));
    }
}
