//! PreToolUse hook. Reads the hook event on stdin and denies tool calls that
//! break the role rules. The agent is identified by `agent_type`, which Claude
//! Code sets inside a subagent and when the session runs with `--agent` or the
//! `agent` setting. Without it the caller is the owner in a plain session.
//!
//! Rules:
//! - Ticket files are written by the ticket server only, for everyone.
//! - Each ticket tool may be called only by the roles in `tools::allowed_roles`.
//! - A role writes only below its `write_allow` paths from `.incipit/config.json`.
//! - Roles never touch the team configuration (`.claude/`, `.mcp.json`, `.incipit/` control files).
//!
//! Edit/Write checks are exact. The Bash check is a text match on the command
//! and therefore best effort: it stops direct references, not determined evasion.

use crate::{config, tools, util};
use serde::Deserialize;
use serde_json::{json, Value};
use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(Deserialize, Default)]
struct HookInput {
    #[serde(default)]
    tool_name: String,
    #[serde(default)]
    tool_input: Value,
    #[serde(default)]
    agent_type: Option<String>,
    #[serde(default)]
    cwd: Option<String>,
}

/// Written by the ticket server only.
const SERVER_ONLY: [&str; 3] = [".incipit/tasks", ".incipit/proposals", ".incipit/decisions.json"];
/// Off limits for every role; the owner maintains these.
const TEAM_CONFIG: [&str; 6] = [
    ".claude",
    ".mcp.json",
    ".incipit/config.json",
    ".incipit/protocol.md",
    ".incipit/state.json",
    ".git",
];

/// The same in the form a shell command names them. `.git/` rather than
/// `.git`, so that `.gitignore` and `.github` stay usable.
const TEAM_CONFIG_IN_SHELL: [&str; 6] = [
    ".claude",
    ".mcp.json",
    ".incipit/config.json",
    ".incipit/protocol.md",
    ".incipit/state.json",
    ".git/",
];

/// Roles that judge by reading. Their shell runs read-only git and nothing else.
const READ_ONLY_ROLES: [&str; 3] = ["auditor", "security", "researcher"];

/// git subcommands that only read the repository. `status` is left out on
/// purpose: it may refresh and write the index.
const READ_ONLY_GIT: [&str; 13] = [
    "log", "show", "diff", "ls-files", "ls-tree", "rev-parse", "rev-list",
    "blame", "grep", "cat-file", "describe", "merge-base", "shortlog",
];

/// Options of those subcommands that write a file or run another program.
const WRITING_GIT_OPTIONS: [&str; 6] = ["--output", "--ext-diff", "--open-files-in-pager", "--paginate", "--exec", "-O"];

/// Split a command line into words the way a shell would for simple
/// commands: whitespace separates, single and double quotes group.
fn shell_words(command: &str) -> Result<Vec<String>, String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut in_word = false;
    let mut quote: Option<char> = None;
    for c in command.chars() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => current.push(c),
            None if c == '\'' || c == '"' => {
                quote = Some(c);
                in_word = true;
            }
            None if c.is_whitespace() => {
                if in_word {
                    words.push(std::mem::take(&mut current));
                    in_word = false;
                }
            }
            None => {
                current.push(c);
                in_word = true;
            }
        }
    }
    if quote.is_some() {
        return Err("unbalanced quote".into());
    }
    if in_word {
        words.push(current);
    }
    Ok(words)
}

/// Accept exactly one read-only git command, nothing else. Unlike the text
/// match for the other roles this is exact: a command without shell
/// operators is one program with its arguments, and the program is git.
pub fn read_only_git(command: &str) -> Result<(), String> {
    const OPERATORS: [char; 11] = ['\n', '\r', ';', '&', '|', '`', '$', '>', '<', '(', ')'];
    if let Some(c) = command.chars().find(|c| OPERATORS.contains(c) || *c == '\\') {
        return Err(format!("no shell operators allowed (found {c:?}); run one git command per call"));
    }
    let words = shell_words(command)?;
    match words.first().map(String::as_str) {
        Some("git") => {}
        _ => return Err("only read-only git commands are allowed".into()),
    }
    let sub = words.get(1).map(String::as_str).unwrap_or("");
    if !READ_ONLY_GIT.contains(&sub) {
        return Err(format!(
            "'git {sub}' is not a read-only command here (allowed: git {})",
            READ_ONLY_GIT.join(", git ")
        ));
    }
    for word in &words[2..] {
        if let Some(opt) = WRITING_GIT_OPTIONS.iter().find(|o| word == *o || word.starts_with(&format!("{o}=")) || (o.len() == 2 && word.starts_with(**o))) {
            return Err(format!("the option {opt} writes a file or runs a program"));
        }
    }
    Ok(())
}

pub enum Decision {
    Allow,
    Deny(String),
}

fn deny(msg: impl Into<String>) -> Decision {
    Decision::Deny(msg.into())
}

/// Plugin agents are namespaced (`plugin:role`); project agents are bare.
fn role_of(agent_type: &Option<String>) -> Option<String> {
    let raw = agent_type.as_deref()?.trim();
    if raw.is_empty() {
        return None;
    }
    Some(raw.rsplit(':').next().unwrap_or(raw).to_lowercase())
}

fn check_ticket_tool(tool: &str, input: &Value, role: &Option<String>) -> Decision {
    let allowed = match tools::allowed_roles(tool) {
        Some(a) => a,
        None => return Decision::Allow,
    };
    let is_read = tools::is_read(tool);
    match role {
        None if is_read => Decision::Allow,
        None => deny(format!(
            "{tool} is reserved for the {} agent. The owner session reads tickets only.",
            allowed.join("/")
        )),
        Some(r) if !allowed.contains(&r.as_str()) => deny(format!(
            "{tool} is not available to the {r} role (allowed: {}).",
            allowed.join(", ")
        )),
        Some(r) => {
            if tools::carries_role(tool) {
                let claimed = input.get("role").and_then(Value::as_str).unwrap_or("");
                if claimed != r {
                    return deny(format!("{tool}: role must be your own role '{r}', not '{claimed}'."));
                }
            }
            Decision::Allow
        }
    }
}

fn check_write(root: &Path, cwd: &Path, file: &str, role: &Option<String>) -> Decision {
    let target = if Path::new(file).is_absolute() {
        PathBuf::from(file)
    } else {
        cwd.join(file)
    };
    let rel = util::relative_to(root, &target);

    if let Some(rel) = &rel {
        if SERVER_ONLY.iter().any(|p| util::under(rel, p)) {
            return deny("Tickets and proposals are written by the ticket server only. Use the task_* and spec_propose tools.");
        }
    }
    let role = match role {
        Some(r) => r,
        None => return Decision::Allow, // owner session
    };
    let rel = match rel {
        Some(r) => r,
        None => return deny(format!("The {role} role writes only inside the project.")),
    };
    if TEAM_CONFIG.iter().any(|p| util::under(&rel, p)) {
        return deny(format!("'{rel}' is team configuration and owner-only."));
    }
    let cfg = match config::load(root) {
        Ok(c) => c,
        Err(e) => return deny(format!("Write refused, role rules are unavailable: {e}")),
    };
    let rules = match cfg.roles.get(role.as_str()) {
        Some(r) => r,
        None => return deny(format!("The role '{role}' has no write rules in .incipit/config.json.")),
    };
    if let Some(denied) = rules.write_deny.iter().find(|p| util::under(&rel, p)) {
        return deny(format!("The {role} role may not write '{rel}': '{denied}' is closed to this role."));
    }
    if rules.write_allow.iter().any(|p| util::under(&rel, p)) {
        return Decision::Allow;
    }
    deny(format!(
        "The {role} role may not write '{rel}'. Allowed: {}.",
        if rules.write_allow.is_empty() { "nothing".to_string() } else { rules.write_allow.join(", ") }
    ))
}

fn check_bash(command: &str, role: &Option<String>) -> Decision {
    if let Some(r) = role {
        if READ_ONLY_ROLES.contains(&r.as_str()) {
            return match read_only_git(command) {
                Ok(()) => Decision::Allow,
                Err(e) => deny(format!("The {r} role has a read-only git shell: {e}.")),
            };
        }
    }
    let cmd = command.to_lowercase();
    if let Some(path) = SERVER_ONLY.iter().find(|p| cmd.contains(**p)) {
        return deny(format!("Shell access to {path} is blocked. Use the task_* tools."));
    }
    let role = match role {
        Some(r) => r,
        None => return Decision::Allow,
    };
    if role == "lead" {
        return deny("The lead runs no shell commands. Hand the work to a worker.");
    }
    for path in TEAM_CONFIG_IN_SHELL {
        if cmd.contains(path) {
            return deny(format!("Shell access to '{path}' is owner-only."));
        }
    }
    // git configuration decides which programs git runs (hooks, pagers,
    // diff drivers) for every role that commits after this one.
    let words: Vec<&str> = cmd.split_whitespace().collect();
    if words.windows(2).any(|w| w[0].ends_with("git") && (w[1] == "config" || w[1] == "-c" || w[1].starts_with("--config-env"))) {
        return deny("Changing git configuration is owner-only.");
    }
    // The incipit CLI (activate, phase, ...) is the owner's tool.
    let calls_cli = cmd
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-'))
        .any(|token| token == "incipit");
    if calls_cli {
        return deny("The incipit CLI is owner-only.");
    }
    Decision::Allow
}

pub fn decide(root: &Path, raw: &str) -> Decision {
    let input: HookInput = match serde_json::from_str(raw) {
        Ok(i) => i,
        Err(e) => return deny(format!("incipit guard: unreadable hook input ({e})")),
    };
    let role = role_of(&input.agent_type);
    if let Some(r) = &role {
        if !tools::ROLES.contains(&r.as_str()) {
            // Built-in or foreign agents get read access only.
            let writes = matches!(input.tool_name.as_str(), "Edit" | "Write" | "MultiEdit" | "NotebookEdit" | "Bash");
            let tool = input.tool_name.rsplit("__").next().unwrap_or("");
            let ticket_write = input.tool_name.starts_with("mcp__")
                && input.tool_name.contains("incipit")
                && !tools::is_read(tool);
            if writes || ticket_write {
                return deny(format!("The agent type '{r}' is not an Incipit role and is read-only here."));
            }
            return Decision::Allow;
        }
    }
    let cwd = input
        .cwd
        .as_deref()
        .map(PathBuf::from)
        .unwrap_or_else(|| root.to_path_buf());

    let name = input.tool_name.as_str();
    if name.starts_with("mcp__") && name.contains("incipit") {
        let tool = name.rsplit("__").next().unwrap_or("");
        return check_ticket_tool(tool, &input.tool_input, &role);
    }
    match name {
        "Edit" | "Write" | "MultiEdit" | "NotebookEdit" => {
            let file = input
                .tool_input
                .get("file_path")
                .or_else(|| input.tool_input.get("notebook_path"))
                .and_then(Value::as_str)
                .unwrap_or("");
            if file.is_empty() {
                return deny("incipit guard: write without a file path");
            }
            check_write(root, &cwd, file, &role)
        }
        "Bash" => {
            let command = input.tool_input.get("command").and_then(Value::as_str).unwrap_or("");
            check_bash(command, &role)
        }
        _ => Decision::Allow,
    }
}

pub fn run(root: &Path) {
    let mut raw = String::new();
    if std::io::stdin().read_to_string(&mut raw).is_err() {
        eprintln!("incipit guard: cannot read hook input");
        std::process::exit(2);
    }
    if let Decision::Deny(reason) = decide(root, &raw) {
        let out = json!({
            "hookSpecificOutput": {
                "hookEventName": "PreToolUse",
                "permissionDecision": "deny",
                "permissionDecisionReason": reason
            }
        });
        println!("{out}");
    }
}

#[cfg(test)]
mod tests {
    use super::read_only_git;

    #[test]
    fn read_only_git_accepts_plain_reads() {
        for ok in [
            "git log --oneline -- src/",
            "git show 4bd3c2c:src/main.c",
            "git diff 858a047 cc0edbe -- tests/",
            "git log --format='%h %s' --grep=T-0014",
            "git ls-files references/",
            "git blame -L 10,20 src/zstr.c",
        ] {
            assert!(read_only_git(ok).is_ok(), "{ok}");
        }
    }

    #[test]
    fn read_only_git_refuses_everything_else() {
        for bad in [
            "git status",
            "git commit -am x",
            "git checkout -- src/main.c",
            "git config core.pager cat",
            "git -c core.pager=sh log",
            "git -C /tmp log",
            "git log > out.txt",
            "git log; rm -rf src",
            "git log && touch x",
            "git log | sh",
            "git log $(touch x)",
            "git log `touch x`",
            "git diff --output=/tmp/x",
            "git diff --ext-diff",
            "git grep -Ovim foo",
            "GIT_PAGER=sh git log",
            "ls src",
            "git log 'unbalanced",
        ] {
            assert!(read_only_git(bad).is_err(), "{bad}");
        }
    }
}
