//! Owner commands on tickets, and the upgrade of an existing project.

use crate::store::{self, Ticket};
use crate::tools::{event, one_line, ticket_hash};
use crate::util::sha256_hex;
use std::fs;
use std::path::Path;

/// `incipit ticket withdraw <id> <reason> | role <id> <role> | verify`
pub fn ticket(root: &Path, args: &[String]) -> Result<String, String> {
    let backend = store::open(root)?;
    match args.first().map(String::as_str).unwrap_or("") {
        "withdraw" => {
            let id = args.get(1).ok_or("usage: incipit ticket withdraw <id> <reason>")?;
            let reason = one_line(&args[2..].join(" "));
            if reason.is_empty() {
                return Err("a reason is required: incipit ticket withdraw <id> <reason>".into());
            }
            let t = backend.update(id, &mut |t| {
                if t.status == "closed" || t.status == "withdrawn" {
                    return Err(format!("{} is already {}", t.id, t.status));
                }
                t.history.push(event("owner", "withdrawn", &format!("was {}: {reason}", t.status), "", ""));
                t.status = "withdrawn".into();
                Ok(())
            })?;
            Ok(format!("{} withdrawn", t.id))
        }
        "role" => {
            let id = args.get(1).ok_or("usage: incipit ticket role <id> implementer|tester")?;
            let role = args.get(2).map(String::as_str).unwrap_or("");
            if role != "implementer" && role != "tester" {
                return Err("usage: incipit ticket role <id> implementer|tester".into());
            }
            let t = backend.update(id, &mut |t| {
                if t.status != "open" && t.status != "blocked" {
                    return Err(format!("{} is {}; the role can only be set while open or blocked", t.id, t.status));
                }
                if t.role == role {
                    return Err(format!("{} already has role {role}", t.id));
                }
                t.history.push(event("owner", "role", &format!("{} -> {role}", t.role), "", ""));
                t.role = role.to_string();
                if role == "tester" {
                    t.audit_required = true;
                }
                Ok(())
            })?;
            Ok(format!("{} now has role {} (audit required: {})", t.id, t.role, t.audit_required))
        }
        "verify" => verify(root, &backend.list()?),
        "list" => {
            let filter = args.get(1).map(String::as_str);
            let tickets: Vec<Ticket> = backend
                .list()?
                .into_iter()
                .filter(|t| filter.map_or(true, |f| t.status == f))
                .collect();
            Ok(crate::tools::list_lines(&tickets))
        }
        "show" => {
            let id = args.get(1).ok_or("usage: incipit ticket show <id>")?;
            Ok(crate::tools::render(&backend.load(id)?, true))
        }
        _ => Err("usage: incipit ticket list [status] | show <id> | withdraw <id> <reason> | role <id> implementer|tester | verify".into()),
    }
}

fn file_hash(root: &Path, rel: &str) -> Option<String> {
    fs::read(root.join(rel)).ok().map(|b| sha256_hex(&b))
}

/// Check every closed ticket against its receipt.
fn verify(root: &Path, tickets: &[Ticket]) -> Result<String, String> {
    let mut out = String::new();
    let (mut ok, mut bad, mut bare) = (0, 0, 0);
    for t in tickets.iter().filter(|t| t.status == "closed") {
        let r = match &t.receipt {
            Some(r) => r,
            None => {
                bare += 1;
                out.push_str(&format!("{} NO RECEIPT (closed before receipts existed)\n", t.id));
                continue;
            }
        };
        let mut problems = Vec::new();
        if ticket_hash(t) != r.ticket_sha256 {
            problems.push("ticket text differs from what was accepted".to_string());
        }
        let mut files = vec![("verdict", &r.verdict_evidence, &r.verdict_sha256)];
        if !r.audit_evidence.is_empty() {
            files.push(("audit", &r.audit_evidence, &r.audit_sha256));
        }
        for (what, rel, sha) in files {
            if sha.is_empty() {
                continue;
            }
            match file_hash(root, rel) {
                None => problems.push(format!("{what} evidence {rel} is missing")),
                Some(h) if &h != sha => problems.push(format!("{what} evidence {rel} was changed")),
                Some(_) => {}
            }
        }
        if problems.is_empty() {
            ok += 1;
            let short = &r.commit[..r.commit.len().min(12)];
            out.push_str(&format!("{} OK (commit {short}, tree clean: {})\n", t.id, r.tree_clean));
        } else {
            bad += 1;
            out.push_str(&format!("{} MISMATCH: {}\n", t.id, problems.join("; ")));
        }
    }
    out.push_str(&format!("verified: {ok} ok, {bad} mismatch, {bare} without receipt\n"));
    if bad > 0 {
        return Err(out);
    }
    Ok(out)
}

// --- status ----------------------------------------------------------------

/// Everything the owner needs at a glance, ending with the steps that are theirs.
pub fn status(root: &Path) -> Result<String, String> {
    let mut out = String::new();
    let mut steps: Vec<String> = Vec::new();

    let phases = crate::admin::phase(root, &[])?;
    let next = phases.lines().find_map(|l| l.strip_prefix("next: ")).unwrap_or("-");
    if next == "-" {
        out.push_str("setup: complete\n");
    } else {
        out.push_str(&format!("setup: next phase is {next}\n"));
        steps.push("/incipit:newproject in Claude Code, to continue the setup".into());
    }
    let settings = fs::read_to_string(root.join(".claude").join("settings.json")).unwrap_or_default();
    let lead = serde_json::from_str::<serde_json::Value>(&settings)
        .ok()
        .and_then(|v| v.get("agent").and_then(|a| a.as_str()).map(|a| a == "lead"))
        .unwrap_or(false);
    out.push_str(&format!("lead: {}\n", if lead { "active" } else { "not active" }));

    let tickets = store::open(root)?.list()?;
    let count = |st: &str| tickets.iter().filter(|t| t.status == st).count();
    out.push_str(&format!(
        "tickets: {} open, {} in progress, {} blocked, {} implemented, {} accepted, {} closed, {} withdrawn\n",
        count("open"), count("in_progress"), count("blocked"), count("implemented"),
        count("accepted"), count("closed"), count("withdrawn")
    ));
    let active: Vec<Ticket> = tickets
        .iter()
        .filter(|t| t.status != "closed" && t.status != "withdrawn")
        .cloned()
        .collect();
    if !active.is_empty() {
        out.push_str(&crate::tools::list_lines(&active));
    }
    // A superseded ticket stays in the queue until the owner withdraws it.
    for t in &tickets {
        if t.supersedes.is_empty() || t.status == "withdrawn" {
            continue;
        }
        if let Some(old) = tickets.iter().find(|o| o.id == t.supersedes) {
            if old.status != "withdrawn" && old.status != "closed" {
                steps.push(format!("incipit ticket withdraw {} \"superseded by {}\"", old.id, t.id));
            }
        }
    }

    let pending = crate::spec::pending(root);
    out.push_str(&format!("spec proposals pending: {}\n", pending.len()));
    for (id, reason) in &pending {
        out.push_str(&format!("  {id}: {reason}\n"));
        steps.push(format!("incipit spec show {id}   then: incipit spec apply {id}  (or reject)"));
    }

    let log = crate::decisions::load(root)?;
    let open = crate::decisions::open_questions(&log);
    out.push_str(&format!("open questions: {}\n", open.len()));
    for q in &open {
        out.push_str(&format!("  {}: {}\n", q.id, q.text));
    }
    if !open.is_empty() {
        steps.push("answer the open questions to the lead".into());
    }

    let dirty = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["status", "--porcelain"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).lines().count());
    match dirty {
        Some(0) => out.push_str("git: clean\n"),
        Some(n) => {
            out.push_str(&format!("git: {n} uncommitted file(s)\n"));
            steps.push("git add -A && git commit -m \"...\"   (if no worker is about to commit)".into());
        }
        None => out.push_str("git: not a repository\n"),
    }

    out.push_str(&format!("\nyour steps: {}\n", if steps.is_empty() { "none" } else { "" }));
    for step in steps {
        out.push_str(&format!("  {step}\n"));
    }
    Ok(out)
}

// --- roles and upgrade -----------------------------------------------------

const TEMPLATES: [(&str, &str); 5] = [
    ("lead", include_str!("../../templates/base/dot-claude/agents/lead.md")),
    ("architect", include_str!("../../templates/base/dot-claude/agents/architect.md")),
    ("implementer", include_str!("../../templates/base/dot-claude/agents/implementer.md")),
    ("tester", include_str!("../../templates/base/dot-claude/agents/tester.md")),
    ("auditor", include_str!("../../templates/base/dot-claude/agents/auditor.md")),
];
const OPTIONAL_TEMPLATES: [(&str, &str); 2] = [
    ("security", include_str!("../../templates/optional-agents/security.md")),
    ("researcher", include_str!("../../templates/optional-agents/researcher.md")),
];
const PROTOCOL: &str = include_str!("../../templates/base/dot-incipit/protocol.md");

fn mark(version: &str) -> String {
    format!("<!-- incipit-update {version} -->")
}

const HEADER_NOTE: &str = "This section overrides anything above that contradicts it.";

/// Rule changes per version, appended to briefs written for an older version.
/// Project-specific text in the briefs is left untouched.
fn update_note(version: &str, role: &str) -> Option<&'static str> {
    Some(match (version, role) {
        ("0.1.2", "lead") => "- Every ticket has a `role`. `implementer` tickets: implementer with
  `order: implement`, then tester with `order: verify`. `tester` tickets
  (test tools, harness, checks): tester with `order: build`; they always need
  the auditor before the tester can close with `order: close`.
- A ticket is never rewritten. The architect adds to it (`task_amend`) or
  creates a replacement with `supersedes`. Taking a ticket out of the queue is
  the owner's step: `incipit ticket withdraw <id> <reason>` in a terminal.
- Spec changes: the architect files a proposal (`spec_propose`), the owner
  applies it with `incipit spec apply <id>`. Tell the owner the exact command
  when one of these is due.
",
        ("0.1.2", "architect") => "- `task_update` is gone. A ticket cannot be rewritten. While it is `open` or
  `blocked` you add to it with `task_amend`: a clarification, a further
  criterion, a new priority, or the audit requirement. Nothing can be removed
  or reworded. To ask for less or for something else, create a new ticket
  with `supersedes: <old id>` and report the old id so the owner can withdraw
  it.
- `task_create` takes `role`: `implementer` for product code, `tester` for
  test tools, harness work and checks. Tester tickets always require an audit.
- Every ticket must be closable on its own. A criterion that can only be
  checked once a later ticket exists belongs to that later ticket.
- You cannot write `SPEC.md`. File spec changes with `spec_propose`: each edit
  replaces one exact passage that occurs exactly once. One proposal per owner
  decision, named in `reason`. Report the proposal id; the owner applies it.
",
        ("0.1.2", "implementer") => "- `task_status` needs `role: \"implementer\"` and works only on tickets of
  role `implementer`. A `tester` ticket handed to you is a mistake: report
  `BLOCKED`.
- Commit your work, with the ticket id at the start of the message, before
  you set the ticket to `implemented`.
",
        ("0.1.2", "tester") => "- New order `build`, for tickets of role `tester` (test tools, harness,
  checks). You build them: `task_status` to `in_progress` and later to
  `implemented`, always with `role: \"tester\"`. Show that the tool or check
  fails when the thing it checks is wrong. Then write the evidence and record
  your verdict. These tickets always need the auditor before you can close.
- Commit tests and evidence, with the ticket id at the start of the message,
  before `task_verdict`. The evidence file is hashed when the verdict is
  recorded: do not edit it afterwards, or closing is refused.
- Before `task_close`, commit anything still uncommitted, including the
  auditor's evidence file. Closing writes a receipt into the ticket (commit,
  evidence hashes, ticket hash). If the result says the tree was not clean,
  commit and mention it in `note`.
",
        ("0.1.2", "auditor") => "- Tickets of role `tester` are built and verified by the tester. For those
  you are the only independent check: a tool or check that passes whatever
  the product does is a FAIL.
- Your evidence file is hashed when you record the audit. Do not edit it
  afterwards.
",
        ("0.1.3", "lead") => "- **Decision log.** You remember nothing between sessions; the log does.
  At the start of every session call `decision_list` and pick up from the
  open questions. Log with `decision_log`: every decision the owner makes
  (kind `decision`, in the owner's words, with the ticket or topic it applies
  to), every question you put to the owner (kind `question`), the owner's
  answer to a logged question (kind `answer`, `ref` = the question's id), and
  what the next session must know (kind `note`). When you list several
  changes or options for the owner, log the list, not a pointer to the chat.
- **Commits.** Implementer and tester commit with `git add -A`, which takes
  along ticket state and the documents of roles without a shell. If files are
  left uncommitted and no worker is about to commit, give the owner the
  command: `git add -A && git commit -m \"...\"`.
- **Optional roles**, if this project has them (see `optional_roles` in
  `.incipit/config.json`). `security`: a ticket with `security_required`
  needs its PASS before closing; send it after the tester's verdict (and the
  auditor, if required) with `order: security`, then the tester with
  `order: close`. `researcher`: send it with `order: research` and one
  precise question when a ticket rests on what a reference or standard says
  and nobody has checked; it reports where the answer is written down.
- The owner sees everything due with `incipit status` in a terminal.
",
        ("0.1.3", "architect") => "- If the project has a security role (`optional_roles` in
  `.incipit/config.json`), set `security_required` on tickets that fall
  under `security_policy` in the same file. On an existing open ticket use
  `task_amend` with `require_security`.
",
        ("0.1.3", "implementer") => "- Commit with `git add -A` (then `git commit`, ticket id first in the
  message). That takes along the ticket state and documents written by roles
  that have no shell. Never name files under `.incipit/` in a command.
",
        ("0.1.3", "tester") => "- Commit with `git add -A` (then `git commit`, ticket id first in the
  message). That takes along the ticket state and documents written by roles
  that have no shell. Never name files under `.incipit/` in a command.
- A ticket with `security_required` also needs a PASS of the security role
  before `task_close` works. If closing is refused for that reason, leave the
  ticket `accepted` and report that the security review is pending.
",
        ("0.1.4", "lead") => "- The auditor (and the security reviewer, if the project has one) now has
  a read-only git shell. It checks \"unchanged since\", \"which commits\" and
  \"what did this ticket change\" itself. Do not hand the git checks of an
  audit to the tester: the tester built or verified the ticket and would be
  checking its own work.
",
        ("0.1.4", "auditor") | ("0.1.4", "security") => "- You now have a shell that runs read-only git and nothing else: `git log`,
  `show`, `diff`, `ls-files`, `ls-tree`, `rev-parse`, `rev-list`, `blame`,
  `grep`, `cat-file`, `describe`, `merge-base`, `shortlog`. One command per
  call, without pipes, redirections, variables or `&&`; the guard refuses
  anything else. Use it for every claim about change: which commits belong to
  the ticket (`git log --grep=<id>`), what they changed
  (`git show --stat <commit>`), whether something is unchanged since a commit
  (`git diff <commit> -- <path>`). Such a claim is `read` only if you ran the
  command; name it in the evidence. You still build and run nothing.
",
        ("0.1.4", "implementer") | ("0.1.4", "tester") => "- Changing git configuration (`git config`, `git -c`) and writing below
  `.git/` are owner-only now. Git configuration decides which programs git
  runs for every role that commits after you.
",
        ("0.1.5", "lead") => "- Every worker commits its own work with `task_commit`. The server takes
  only the files of the calling role and the state of the named ticket, so
  workers can run side by side without their files ending up in each other's
  commits. The rule \"git add -A\" of 0.1.3 is gone. If `incipit status`
  shows uncommitted files that no worker is about to commit, tell the owner
  which ones.
",
        ("0.1.5", "implementer") => "- `git add -A` is gone. Commit with `task_commit` (your role, the ticket
  id, one line). The server commits your files and the ticket's state and
  nothing of other roles. `git add`, `git commit` and every other command
  that changes the index, a branch or the history are blocked for you.
",
        ("0.1.5", "tester") => "- `git add -A` is gone. Commit with `task_commit` (your role, the ticket
  id, one line): before the verdict, before closing (it takes the evidence of
  auditor and security reviewer along), and once more after closing so the
  receipt is committed. `git add`, `git commit` and every other command that
  changes the index, a branch or the history are blocked for you.
",
        ("0.1.5", "architect") | ("0.1.5", "researcher") => "- Commit what you wrote with `task_commit`: your role, the id of the
  ticket it belongs to if there is one, and one line. The server takes only
  your files.
",
        ("0.1.5", "auditor") | ("0.1.5", "security") => "- After recording your result, commit the evidence with `task_commit`:
  your role and the ticket id.
",
        _ => return None,
    })
}

const VERSIONS: [&str; 4] = ["0.1.2", "0.1.3", "0.1.4", "0.1.5"];

fn tools_line(text: &str) -> Option<&str> {
    // Frontmatter only: stop at the closing delimiter.
    let mut lines = text.lines();
    if lines.next()? != "---" {
        return None;
    }
    for line in lines {
        if line == "---" {
            return None;
        }
        if line.starts_with("tools:") {
            return Some(line);
        }
    }
    None
}

fn optional_roles(root: &Path) -> Vec<String> {
    crate::config::load(root).map(|c| c.optional_roles).unwrap_or_default()
}

/// The tools line a brief should have. The lead's list of agents it may
/// start grows with the project's optional roles.
fn wanted_tools(role: &str, template: &str, optional: &[String]) -> Result<String, String> {
    let line = tools_line(template).ok_or("template without tools line")?.to_string();
    if role != "lead" || optional.is_empty() {
        return Ok(line);
    }
    let base = "Agent(architect, implementer, tester, auditor)";
    if !line.contains(base) {
        return Err("lead template has an unexpected Agent list".into());
    }
    Ok(line.replace(base, &format!("Agent(architect, implementer, tester, auditor, {})", optional.join(", "))))
}

/// Replace the tools line and append missing rule sections. Returns what was done.
fn refresh_brief(path: &Path, role: &str, template: &str, optional: &[String]) -> Result<Vec<&'static str>, String> {
    let mut text = fs::read_to_string(path).map_err(|_| format!("{} is missing", path.display()))?;
    let mut done = Vec::new();
    let want = wanted_tools(role, template, optional)?;
    match tools_line(&text) {
        Some(have) if have == want => {}
        Some(have) => {
            let have = have.to_string();
            text = text.replacen(&have, &want, 1);
            done.push("tools list");
        }
        None => return Err(format!("{} has no tools line in its frontmatter", path.display())),
    }
    for version in VERSIONS {
        let m = mark(version);
        if text.contains(&m) {
            continue;
        }
        if !text.ends_with('\n') {
            text.push('\n');
        }
        match update_note(version, role) {
            Some(note) => {
                text.push_str(&format!("\n{m}\n## Rules changed in Incipit {version}\n\n{HEADER_NOTE}\n\n{note}"));
                done.push("rules section");
            }
            // Nothing changed for this role; the mark alone records that.
            None => text.push_str(&format!("\n{m}\n")),
        }
    }
    done.dedup();
    if !done.is_empty() || fs::read_to_string(path).ok().as_deref() != Some(text.as_str()) {
        fs::write(path, &text).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    }
    Ok(done)
}

/// Bring a project that was scaffolded by an older version up to date.
/// Safe to run again: every step checks whether it is still needed.
pub fn upgrade(root: &Path) -> Result<String, String> {
    let agents = root.join(".claude").join("agents");
    if !agents.is_dir() {
        return Err("no .claude/agents here; run this in an Incipit project".into());
    }
    let optional = optional_roles(root);
    let mut out = String::new();
    let mut briefs: Vec<(&str, &str)> = TEMPLATES.to_vec();
    for (role, template) in OPTIONAL_TEMPLATES {
        if optional.iter().any(|r| r == role) {
            briefs.push((role, template));
        }
    }
    for (role, template) in briefs {
        let path = agents.join(format!("{role}.md"));
        let done = refresh_brief(&path, role, template, &optional)?;
        if done.is_empty() {
            out.push_str(&format!("kept    .claude/agents/{role}.md\n"));
        } else {
            out.push_str(&format!("updated .claude/agents/{role}.md ({})\n", done.join(", ")));
        }
    }
    let protocol = root.join(".incipit").join("protocol.md");
    if fs::read_to_string(&protocol).ok().as_deref() == Some(PROTOCOL) {
        out.push_str("kept    .incipit/protocol.md\n");
    } else {
        fs::write(&protocol, PROTOCOL).map_err(|e| format!("cannot write protocol.md: {e}"))?;
        out.push_str("updated .incipit/protocol.md\n");
    }
    out.push_str("\nNext: commit, then restart Claude Code. `incipit status` shows what is due.\n");
    Ok(out)
}

/// `incipit role add security|researcher`: switch an optional role on.
pub fn role(root: &Path, args: &[String]) -> Result<String, String> {
    let usage = "usage: incipit role add security|researcher";
    if args.first().map(String::as_str) != Some("add") {
        return Err(usage.into());
    }
    let name = args.get(1).map(String::as_str).unwrap_or("");
    let template = OPTIONAL_TEMPLATES
        .iter()
        .find(|(role, _)| *role == name)
        .map(|(_, t)| *t)
        .ok_or(usage)?;
    let agents = root.join(".claude").join("agents");
    if !agents.join("lead.md").is_file() {
        return Err("no .claude/agents/lead.md here; run the scaffold phase first".into());
    }

    // 1. Config: record the role and give it write rules.
    let config_path = root.join(".incipit").join("config.json");
    let raw = fs::read_to_string(&config_path).map_err(|e| format!("cannot read .incipit/config.json: {e}"))?;
    let mut config: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| format!("invalid .incipit/config.json: {e}"))?;
    let obj = config.as_object_mut().ok_or(".incipit/config.json is not an object")?;
    let list = obj.entry("optional_roles").or_insert_with(|| serde_json::json!([]));
    let arr = list.as_array_mut().ok_or("optional_roles is not a list")?;
    let mut out = String::new();
    if arr.iter().any(|v| v.as_str() == Some(name)) {
        out.push_str(&format!("kept    optional_roles (already has {name})\n"));
    } else {
        arr.push(serde_json::json!(name));
        out.push_str(&format!("updated .incipit/config.json (optional_roles + {name})\n"));
    }
    let project_name = obj
        .get("project")
        .and_then(|p| p.get("name"))
        .and_then(|n| n.as_str())
        .unwrap_or("")
        .to_string();
    let roles = obj.entry("roles").or_insert_with(|| serde_json::json!({}));
    let roles = roles.as_object_mut().ok_or("roles is not an object")?;
    if !roles.contains_key(name) {
        let allow = if name == "security" { ".incipit/evidence/" } else { "docs/research/" };
        roles.insert(name.to_string(), serde_json::json!({"write_allow": [allow], "write_deny": []}));
        out.push_str(&format!("updated .incipit/config.json (roles.{name} may write {allow})\n"));
    }
    if name == "security" && !obj.contains_key("security_policy") {
        obj.insert("security_policy".into(), serde_json::json!(""));
    }
    let body = serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?;
    fs::write(&config_path, body + "\n").map_err(|e| format!("cannot write .incipit/config.json: {e}"))?;

    // 2. The brief.
    let brief = agents.join(format!("{name}.md"));
    if brief.is_file() {
        out.push_str(&format!("kept    .claude/agents/{name}.md\n"));
    } else {
        let mut text = template.to_string();
        // After the team phase nobody fills placeholders any more, so do it here.
        let state = fs::read_to_string(root.join(".incipit").join("state.json")).unwrap_or_default();
        let team_done = serde_json::from_str::<serde_json::Value>(&state)
            .ok()
            .and_then(|v| v.get("phases").and_then(|p| p.get("team")).and_then(|t| t.as_str()).map(|t| t == "done"))
            .unwrap_or(false);
        if team_done {
            let shown = if project_name.is_empty() { "project" } else { project_name.as_str() };
            text = text.replace("{{PROJECT_NAME}}", shown).replace("{{PROJECT_NOTES}}", "None.");
        }
        fs::write(&brief, text).map_err(|e| format!("cannot write {}: {e}", brief.display()))?;
        out.push_str(&format!("created .claude/agents/{name}.md\n"));
    }

    // 3. The lead may start the new agent.
    let optional = optional_roles(root);
    let done = refresh_brief(&agents.join("lead.md"), "lead", TEMPLATES[0].1, &optional)?;
    out.push_str(&if done.is_empty() {
        "kept    .claude/agents/lead.md\n".to_string()
    } else {
        format!("updated .claude/agents/lead.md ({})\n", done.join(", "))
    });

    out.push_str("\nNext:\n");
    if name == "security" {
        out.push_str("  set \"security_policy\" in .incipit/config.json: which tickets need a security review, in one sentence\n");
    }
    out.push_str("  commit, then restart Claude Code\n");
    Ok(out)
}
