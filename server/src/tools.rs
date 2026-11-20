//! The ticket tools and their rules. Each mutating tool belongs to fixed roles
//! (see `allowed_roles`); the guard enforces that mapping per agent, and the
//! handlers enforce the ticket rules regardless of who calls.
//!
//! States: open -> in_progress -> implemented -> accepted -> closed
//!         in_progress <-> blocked, implemented -> in_progress (withdraw work),
//!         a FAIL verdict or audit sends the ticket back to open,
//!         the owner can set any unfinished ticket to withdrawn.
//!
//! Tickets are append-only. What a ticket asks for is fixed when it is
//! created; afterwards it can only gain clarifications and criteria. Asking
//! for less takes a new ticket and the owner withdrawing the old one.

use crate::store::{Amendment, Backend, Event, Receipt, Ticket};
use crate::util::{now_iso, sha256_hex};
use serde_json::{json, Value};
use std::path::Path;

pub const ROLES: [&str; 7] = ["lead", "architect", "implementer", "tester", "auditor", "security", "researcher"];


/// Which roles may call a tool. Read tools are open to every role.
pub fn allowed_roles(tool: &str) -> Option<&'static [&'static str]> {
    Some(match tool {
        "task_list" | "task_get" | "decision_list" => &ROLES,
        "task_create" | "task_amend" | "spec_propose" => &["architect"],
        "task_status" => &["implementer", "tester"],
        "task_comment" => &["architect", "implementer", "tester", "auditor", "security", "researcher"],
        "task_verdict" | "task_close" => &["tester"],
        "task_audit" => &["auditor"],
        "task_security" => &["security"],
        "decision_log" => &["lead"],
        _ => return None,
    })
}

/// Tools that carry the caller's role as an argument; the guard checks that
/// the argument matches the calling agent.
/// Tools that change nothing.
pub fn is_read(tool: &str) -> bool {
    matches!(tool, "task_list" | "task_get" | "decision_list")
}

pub fn carries_role(tool: &str) -> bool {
    tool == "task_comment" || tool == "task_status"
}

pub fn definitions() -> Value {
    let id = json!({"type": "string", "description": "Ticket id, e.g. T-0007"});
    let evidence = json!({"type": "string", "description": "Path of the evidence file below .incipit/evidence/, e.g. .incipit/evidence/T-0007-test.md. Must exist and be non-empty."});
    json!([
        {
            "name": "task_list",
            "description": "List tickets as one compact line each. Optional status filter.",
            "inputSchema": {"type": "object", "properties": {
                "status": {"type": "string", "enum": ["open", "in_progress", "blocked", "implemented", "accepted", "closed", "withdrawn"]}
            }}
        },
        {
            "name": "task_get",
            "description": "Show one ticket as a key-value block with its criteria, amendments, recent history and, once closed, its receipt.",
            "inputSchema": {"type": "object", "required": ["id"], "properties": {
                "id": id,
                "full_history": {"type": "boolean", "description": "Show the whole history instead of the last 6 events."}
            }}
        },
        {
            "name": "task_create",
            "description": "ARCHITECT ONLY. Create a ticket derived from the spec. A ticket cannot be rewritten later, only amended, so cut it carefully.",
            "inputSchema": {"type": "object", "required": ["title", "scope", "spec_ref", "acceptance"], "properties": {
                "title": {"type": "string"},
                "scope": {"type": "string", "description": "One or two sentences: what is in, what is out."},
                "spec_ref": {"type": "string", "description": "Spec section this ticket implements, e.g. SPEC.md#4.2"},
                "acceptance": {"type": "array", "items": {"type": "string"}, "minItems": 1, "description": "Checkable acceptance criteria."},
                "role": {"type": "string", "enum": ["implementer", "tester"], "description": "Who builds it. Use tester for test tools, harness work and checks; such tickets always require an audit. Default implementer."},
                "priority": {"type": "string", "enum": ["P1", "P2", "P3"]},
                "audit_required": {"type": "boolean", "description": "If true, the ticket cannot be closed without an auditor PASS."},
                "security_required": {"type": "boolean", "description": "If true, the ticket cannot be closed without a PASS of the security role. Only in projects that have that role."},
                "supersedes": {"type": "string", "description": "Id of the ticket this one replaces. The owner withdraws the old one."}
            }}
        },
        {
            "name": "task_amend",
            "description": "ARCHITECT ONLY. Add to a ticket that is open or blocked: a clarification, a further acceptance criterion, a new priority, or turning the audit requirement on. Nothing already in the ticket can be changed or removed; to ask for less, create a new ticket with `supersedes`.",
            "inputSchema": {"type": "object", "required": ["id"], "properties": {
                "id": id,
                "clarification": {"type": "string", "description": "Explains the ticket without changing what must be delivered."},
                "add_criterion": {"type": "string", "description": "A further checkable acceptance criterion."},
                "priority": {"type": "string", "enum": ["P1", "P2", "P3"]},
                "require_audit": {"type": "boolean", "description": "Only true is accepted: turns the audit requirement on."},
                "require_security": {"type": "boolean", "description": "Only true is accepted: turns the security review requirement on."}
            }}
        },
        {
            "name": "task_status",
            "description": "IMPLEMENTER or TESTER, for tickets of your own role only. Move a ticket between open, in_progress, blocked and implemented.",
            "inputSchema": {"type": "object", "required": ["id", "role", "status"], "properties": {
                "id": id,
                "role": {"type": "string", "enum": ["implementer", "tester"], "description": "Your own role. Must match the ticket's role."},
                "status": {"type": "string", "enum": ["in_progress", "blocked", "implemented"]},
                "note": {"type": "string", "description": "One line. Required for blocked."}
            }}
        },
        {
            "name": "task_comment",
            "description": "Add a one-line note to a ticket. `role` must be your own role.",
            "inputSchema": {"type": "object", "required": ["id", "role", "text"], "properties": {
                "id": id,
                "role": {"type": "string", "enum": ["architect", "implementer", "tester", "auditor", "security", "researcher"]},
                "text": {"type": "string"}
            }}
        },
        {
            "name": "task_verdict",
            "description": "TESTER ONLY. Record the acceptance test result for an implemented ticket. PASS moves it to accepted, FAIL back to open. The evidence file is hashed; changing it afterwards blocks closing.",
            "inputSchema": {"type": "object", "required": ["id", "verdict", "evidence"], "properties": {
                "id": id,
                "verdict": {"type": "string", "enum": ["PASS", "FAIL"]},
                "evidence": evidence,
                "note": {"type": "string"}
            }}
        },
        {
            "name": "task_audit",
            "description": "AUDITOR ONLY. Record the audit result against the spec for an implemented or accepted ticket. FAIL sends it back to open. The evidence file is hashed.",
            "inputSchema": {"type": "object", "required": ["id", "verdict", "evidence"], "properties": {
                "id": id,
                "verdict": {"type": "string", "enum": ["PASS", "FAIL"]},
                "evidence": evidence,
                "note": {"type": "string"}
            }}
        },
        {
            "name": "task_security",
            "description": "SECURITY ONLY. Record the security review result for an implemented or accepted ticket. FAIL sends it back to open. The evidence file is hashed.",
            "inputSchema": {"type": "object", "required": ["id", "verdict", "evidence"], "properties": {
                "id": id,
                "verdict": {"type": "string", "enum": ["PASS", "FAIL"]},
                "evidence": evidence,
                "note": {"type": "string"}
            }}
        },
        {
            "name": "decision_log",
            "description": "LEAD ONLY. Append to the decision log, which outlives the session. Log every decision the owner makes (kind decision, in the owner's words, with what it applies to), every question you put to the owner (kind question), the owner's answer to a logged question (kind answer, ref = the question's id), and anything the next session must know (kind note). Entries are never changed.",
            "inputSchema": {"type": "object", "required": ["kind", "text"], "properties": {
                "kind": {"type": "string", "enum": ["decision", "question", "answer", "note"]},
                "text": {"type": "string"},
                "ref": {"type": "string", "description": "For an answer: id of the question it settles. Otherwise optional ticket or proposal id."}
            }}
        },
        {
            "name": "decision_list",
            "description": "Show the open questions and the latest entries of the decision log.",
            "inputSchema": {"type": "object", "properties": {
                "open_only": {"type": "boolean"},
                "last": {"type": "integer", "description": "How many of the latest entries to show. Default 15."}
            }}
        },
        {
            "name": "task_close",
            "description": "TESTER ONLY. Close an accepted ticket. Refused unless a PASS verdict (and, where required, a PASS audit and a PASS security review) was recorded after the last implementation and the evidence files are unchanged since. Writes a receipt into the ticket: commit, evidence hashes, ticket hash.",
            "inputSchema": {"type": "object", "required": ["id"], "properties": {"id": id}}
        },
        {
            "name": "spec_propose",
            "description": "ARCHITECT ONLY. Propose a change to SPEC.md for the owner to apply. Each edit replaces an exact passage: `old` must occur exactly once in SPEC.md. To insert, use the neighbouring line as `old` and repeat it in `new` with the addition. The owner applies or rejects the proposal; nothing changes until then.",
            "inputSchema": {"type": "object", "required": ["reason", "edits"], "properties": {
                "reason": {"type": "string", "description": "Why, and which decision of the owner it records."},
                "edits": {"type": "array", "minItems": 1, "items": {"type": "object", "required": ["old", "new"], "properties": {
                    "old": {"type": "string"},
                    "new": {"type": "string"}
                }}}
            }}
        }
    ])
}

fn s<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args.get(key).and_then(Value::as_str).map(str::trim).filter(|v| !v.is_empty())
}

fn req<'a>(args: &'a Value, key: &str) -> Result<&'a str, String> {
    s(args, key).ok_or_else(|| format!("missing argument '{key}'"))
}

/// Keep free text on one line so tickets and lists stay compact and parseable.
pub fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn string_list(args: &Value, key: &str) -> Option<Result<Vec<String>, String>> {
    let value = args.get(key)?;
    let arr = match value.as_array() {
        Some(a) => a,
        None => return Some(Err(format!("'{key}' must be a list of strings"))),
    };
    let items: Vec<String> = arr
        .iter()
        .filter_map(Value::as_str)
        .map(one_line)
        .filter(|v| !v.is_empty())
        .collect();
    if items.is_empty() {
        return Some(Err(format!("'{key}' needs at least one entry")));
    }
    Some(Ok(items))
}

fn priority(args: &Value) -> Result<Option<String>, String> {
    match s(args, "priority") {
        None => Ok(None),
        Some(p) if ["P1", "P2", "P3"].contains(&p) => Ok(Some(p.to_string())),
        Some(p) => Err(format!("invalid priority '{p}' (P1, P2 or P3)")),
    }
}

pub fn event(role: &str, action: &str, detail: &str, evidence: &str, sha256: &str) -> Event {
    Event {
        at: now_iso(),
        role: role.to_string(),
        action: action.to_string(),
        detail: one_line(detail),
        evidence: evidence.to_string(),
        sha256: sha256.to_string(),
    }
}

/// Evidence must be a real, non-empty file below `.incipit/evidence/`.
/// Returns its project-relative path and the hash of its content.
fn check_evidence(root: &Path, evidence: &str) -> Result<(String, String), String> {
    let rel = crate::util::relative_to(root, &root.join(evidence))
        .ok_or_else(|| "evidence path must be inside the project".to_string())?;
    if !crate::util::under(&rel, ".incipit/evidence") || rel.eq_ignore_ascii_case(".incipit/evidence") {
        return Err("evidence must be a file below .incipit/evidence/".into());
    }
    let path = root.join(&rel);
    let meta = std::fs::metadata(&path)
        .map_err(|_| format!("evidence file '{rel}' does not exist; write it first"))?;
    if !meta.is_file() || meta.len() == 0 {
        return Err(format!("evidence file '{rel}' is empty or not a file"));
    }
    let bytes = std::fs::read(&path).map_err(|e| format!("cannot read evidence '{rel}': {e}"))?;
    Ok((rel, sha256_hex(&bytes)))
}

fn last_index(ticket: &Ticket, action: &str) -> Option<usize> {
    ticket.history.iter().rposition(|e| e.action == action)
}

/// Hash over everything that defines what the ticket asks for.
pub fn ticket_hash(t: &Ticket) -> String {
    let mut text = format!("{}\n{}\n{}\n{}\n", t.title, t.scope, t.spec_ref, t.role);
    for a in &t.acceptance {
        text.push_str(&format!("AC|{a}\n"));
    }
    for a in &t.amendments {
        text.push_str(&format!("A{}|{}|{}\n", a.n, a.kind, a.text));
    }
    sha256_hex(text.as_bytes())
}

/// The hash recorded with an evidence-bearing event must still match the file.
fn evidence_unchanged(root: &Path, e: &Event, what: &str) -> Result<(), String> {
    if e.sha256.is_empty() {
        // Recorded before hashes existed; nothing to compare against.
        return Ok(());
    }
    let bytes = std::fs::read(root.join(&e.evidence))
        .map_err(|_| format!("{what} evidence '{}' is missing", e.evidence))?;
    if sha256_hex(&bytes) != e.sha256 {
        return Err(format!(
            "{what} evidence '{}' was changed after it was recorded; record the {what} again",
            e.evidence
        ));
    }
    Ok(())
}

pub fn render(ticket: &Ticket, full: bool) -> String {
    let mut out = format!(
        "TICKET {}\nstatus: {}\nrole: {}\npriority: {}\ntitle: {}\nscope: {}\nspec: {}\naudit_required: {}\n",
        ticket.id, ticket.status, ticket.role, ticket.priority, ticket.title, ticket.scope, ticket.spec_ref, ticket.audit_required
    );
    if ticket.security_required {
        out.push_str("security_required: true\n");
    }
    if !ticket.supersedes.is_empty() {
        out.push_str(&format!("supersedes: {}\n", ticket.supersedes));
    }
    out.push_str("acceptance:\n");
    let mut n = 0;
    for item in &ticket.acceptance {
        n += 1;
        out.push_str(&format!("  {n}. {item}\n"));
    }
    for a in ticket.amendments.iter().filter(|a| a.kind == "criterion") {
        n += 1;
        out.push_str(&format!("  {n}. {} (added A{}, {})\n", a.text, a.n, a.at));
    }
    let notes: Vec<&Amendment> = ticket.amendments.iter().filter(|a| a.kind != "criterion").collect();
    if !notes.is_empty() {
        out.push_str("clarifications:\n");
        for a in notes {
            out.push_str(&format!("  A{} {}: {}\n", a.n, a.at, a.text));
        }
    }
    if let Some(r) = &ticket.receipt {
        out.push_str(&format!(
            "receipt:\n  closed_at: {}\n  commit: {}\n  tree_clean: {}\n  ticket_sha256: {}\n  verdict: {} {} sha256={}\n",
            r.closed_at, r.commit, r.tree_clean, r.ticket_sha256, r.verdict_at, r.verdict_evidence, r.verdict_sha256
        ));
        if !r.audit_evidence.is_empty() {
            out.push_str(&format!("  audit: {} {} sha256={}\n", r.audit_at, r.audit_evidence, r.audit_sha256));
        }
        if !r.security_evidence.is_empty() {
            out.push_str(&format!("  security: {} {} sha256={}\n", r.security_at, r.security_evidence, r.security_sha256));
        }
    }
    let skip = if full { 0 } else { ticket.history.len().saturating_sub(6) };
    out.push_str(&format!("history[{} of {}]:\n", ticket.history.len() - skip, ticket.history.len()));
    for e in ticket.history.iter().skip(skip) {
        out.push_str(&format!("  {} {} {}", e.at, e.role, e.action));
        if !e.evidence.is_empty() {
            out.push_str(&format!(" evidence={}", e.evidence));
        }
        if !e.detail.is_empty() {
            out.push_str(&format!(" | {}", e.detail));
        }
        out.push('\n');
    }
    out
}

/// One compact line per ticket. `checks` names the reviews a ticket needs
/// besides the tester's verdict.
pub fn list_lines(tickets: &[Ticket]) -> String {
    let mut out = format!("tasks[{}]{{id,status,prio,role,checks,title}}:\n", tickets.len());
    for t in tickets {
        let checks = match (t.audit_required, t.security_required) {
            (true, true) => "audit+security",
            (true, false) => "audit",
            (false, true) => "security",
            (false, false) => "-",
        };
        out.push_str(&format!("{},{},{},{},{},{}\n", t.id, t.status, t.priority, t.role, checks, t.title));
    }
    out
}

/// A security review can only be demanded where someone can give it.
fn has_security_role(root: &Path) -> bool {
    crate::config::load(root)
        .map(|c| c.optional_roles.iter().any(|r| r == "security"))
        .unwrap_or(false)
}

const NO_SECURITY_ROLE: &str =
    "this project has no security role, so the ticket could never close. The owner adds one with: incipit role add security";

pub fn call(root: &Path, store: &dyn Backend, tool: &str, args: &Value) -> Result<String, String> {
    match tool {
        "task_list" => {
            let filter = s(args, "status");
            let tickets: Vec<Ticket> = store
                .list()?
                .into_iter()
                .filter(|t| filter.map_or(true, |f| t.status == f))
                .collect();
            Ok(list_lines(&tickets))
        }
        "task_get" => {
            let ticket = store.load(req(args, "id")?)?;
            let full = args.get("full_history").and_then(Value::as_bool).unwrap_or(false);
            Ok(render(&ticket, full))
        }
        "task_create" => {
            let title = one_line(req(args, "title")?);
            let scope = one_line(req(args, "scope")?);
            let spec_ref = one_line(req(args, "spec_ref")?);
            let acceptance = string_list(args, "acceptance")
                .ok_or_else(|| "missing argument 'acceptance'".to_string())??;
            let prio = priority(args)?.unwrap_or_else(|| "P2".to_string());
            let role = s(args, "role").unwrap_or("implementer").to_string();
            if role != "implementer" && role != "tester" {
                return Err(format!("invalid role '{role}' (implementer or tester)"));
            }
            // Whoever builds a ticket cannot be its only judge: the tester
            // closes its own tickets, so an auditor must have passed them.
            let audit = role == "tester" || args.get("audit_required").and_then(Value::as_bool).unwrap_or(false);
            let security = args.get("security_required").and_then(Value::as_bool).unwrap_or(false);
            if security && !has_security_role(root) {
                return Err(NO_SECURITY_ROLE.into());
            }
            let supersedes = s(args, "supersedes").unwrap_or("").to_string();
            if !supersedes.is_empty() {
                store.load(&supersedes)?;
            }
            let ticket = store.create(&mut |id| {
                let now = now_iso();
                let detail = if supersedes.is_empty() { String::new() } else { format!("supersedes {supersedes}") };
                Ticket {
                    id,
                    title: title.clone(),
                    status: "open".into(),
                    priority: prio.clone(),
                    role: role.clone(),
                    supersedes: supersedes.clone(),
                    scope: scope.clone(),
                    spec_ref: spec_ref.clone(),
                    acceptance: acceptance.clone(),
                    audit_required: audit,
                    security_required: security,
                    created: now.clone(),
                    updated: now,
                    amendments: Vec::new(),
                    receipt: None,
                    history: vec![event("architect", "created", &detail, "", "")],
                }
            })?;
            let mut out = format!("created {} ({}, role {})", ticket.id, ticket.title, ticket.role);
            if !supersedes.is_empty() {
                out.push_str(&format!("; {supersedes} stays as it is until the owner withdraws it"));
            }
            Ok(out)
        }
        "task_amend" => {
            let prio = priority(args)?;
            if args.get("require_audit").and_then(Value::as_bool) == Some(false) {
                return Err("the audit requirement can be turned on, not off".into());
            }
            let want_security = match args.get("require_security").and_then(Value::as_bool) {
                Some(false) => return Err("the security review requirement can be turned on, not off".into()),
                Some(true) if !has_security_role(root) => return Err(NO_SECURITY_ROLE.into()),
                other => other == Some(true),
            };
            let ticket = store.update(req(args, "id")?, &mut |t| {
                if t.status != "open" && t.status != "blocked" {
                    return Err(format!(
                        "{} is {}; only open or blocked tickets can be amended. Create a follow-up ticket instead",
                        t.id, t.status
                    ));
                }
                let mut changed = Vec::new();
                for (key, kind) in [("clarification", "clarification"), ("add_criterion", "criterion")] {
                    if let Some(text) = s(args, key) {
                        let n = t.amendments.len() as u32 + 1;
                        t.amendments.push(Amendment {
                            n,
                            at: now_iso(),
                            kind: kind.to_string(),
                            text: one_line(text),
                        });
                        changed.push(format!("A{n} {kind}"));
                    }
                }
                if let Some(v) = &prio {
                    if *v != t.priority {
                        changed.push(format!("priority {} -> {v}", t.priority));
                        t.priority = v.clone();
                    }
                }
                if args.get("require_audit").and_then(Value::as_bool) == Some(true) && !t.audit_required {
                    t.audit_required = true;
                    changed.push("audit required".to_string());
                }
                if want_security && !t.security_required {
                    t.security_required = true;
                    changed.push("security review required".to_string());
                }
                if changed.is_empty() {
                    return Err("nothing to add".into());
                }
                t.history.push(event("architect", "amended", &changed.join(", "), "", ""));
                Ok(())
            })?;
            Ok(format!("amended {} ({} amendments)", ticket.id, ticket.amendments.len()))
        }
        "task_status" => {
            let role = req(args, "role")?;
            if role != "implementer" && role != "tester" {
                return Err(format!("invalid role '{role}'"));
            }
            let target = req(args, "status")?;
            let note = s(args, "note").unwrap_or("");
            let ticket = store.update(req(args, "id")?, &mut |t| {
                if t.role != role {
                    return Err(format!(
                        "{} is a {} ticket; the {role} does not move it through its states",
                        t.id, t.role
                    ));
                }
                let ok = matches!(
                    (t.status.as_str(), target),
                    ("open", "in_progress")
                        | ("in_progress", "implemented")
                        | ("in_progress", "blocked")
                        | ("blocked", "in_progress")
                        | ("implemented", "in_progress")
                );
                if !ok {
                    return Err(format!("{}: cannot go from {} to {}", t.id, t.status, target));
                }
                if target == "blocked" && note.is_empty() {
                    return Err("a blocked ticket needs a note saying what blocks it".into());
                }
                t.status = target.to_string();
                t.history.push(event(role, &format!("status:{target}"), note, "", ""));
                Ok(())
            })?;
            Ok(format!("{} is now {}", ticket.id, ticket.status))
        }
        "task_comment" => {
            let role = req(args, "role")?;
            if !["architect", "implementer", "tester", "auditor", "security", "researcher"].contains(&role) {
                return Err(format!("invalid role '{role}'"));
            }
            let text = req(args, "text")?;
            let ticket = store.update(req(args, "id")?, &mut |t| {
                if t.status == "closed" || t.status == "withdrawn" {
                    return Err(format!("{} is {}", t.id, t.status));
                }
                t.history.push(event(role, "comment", text, "", ""));
                Ok(())
            })?;
            Ok(format!("comment added to {}", ticket.id))
        }
        "task_verdict" => {
            let verdict = req(args, "verdict")?;
            if verdict != "PASS" && verdict != "FAIL" {
                return Err("verdict must be PASS or FAIL".into());
            }
            let (evidence, sha) = check_evidence(root, req(args, "evidence")?)?;
            let note = s(args, "note").unwrap_or("");
            let ticket = store.update(req(args, "id")?, &mut |t| {
                if t.status != "implemented" {
                    return Err(format!("{} is {}; a verdict needs status implemented", t.id, t.status));
                }
                t.status = if verdict == "PASS" { "accepted" } else { "open" }.into();
                t.history.push(event("tester", &format!("verdict:{verdict}"), note, &evidence, &sha));
                Ok(())
            })?;
            Ok(format!("{} verdict {} recorded, status {}", ticket.id, verdict, ticket.status))
        }
        "task_audit" => {
            let verdict = req(args, "verdict")?;
            if verdict != "PASS" && verdict != "FAIL" {
                return Err("verdict must be PASS or FAIL".into());
            }
            let (evidence, sha) = check_evidence(root, req(args, "evidence")?)?;
            let note = s(args, "note").unwrap_or("");
            let ticket = store.update(req(args, "id")?, &mut |t| {
                if t.status != "implemented" && t.status != "accepted" {
                    return Err(format!("{} is {}; an audit needs status implemented or accepted", t.id, t.status));
                }
                if verdict == "FAIL" {
                    t.status = "open".into();
                }
                t.history.push(event("auditor", &format!("audit:{verdict}"), note, &evidence, &sha));
                Ok(())
            })?;
            Ok(format!("{} audit {} recorded, status {}", ticket.id, verdict, ticket.status))
        }
        "task_security" => {
            let verdict = req(args, "verdict")?;
            if verdict != "PASS" && verdict != "FAIL" {
                return Err("verdict must be PASS or FAIL".into());
            }
            let (evidence, sha) = check_evidence(root, req(args, "evidence")?)?;
            let note = s(args, "note").unwrap_or("");
            let ticket = store.update(req(args, "id")?, &mut |t| {
                if t.status != "implemented" && t.status != "accepted" {
                    return Err(format!("{} is {}; a security review needs status implemented or accepted", t.id, t.status));
                }
                if verdict == "FAIL" {
                    t.status = "open".into();
                }
                t.history.push(event("security", &format!("security:{verdict}"), note, &evidence, &sha));
                Ok(())
            })?;
            Ok(format!("{} security review {} recorded, status {}", ticket.id, verdict, ticket.status))
        }
        "decision_log" => crate::decisions::log(root, args),
        "decision_list" => crate::decisions::list(root, args),
        "task_close" => {
            let (commit, tree_clean) = crate::util::git_state(root);
            let ticket = store.update(req(args, "id")?, &mut |t| {
                if t.status != "accepted" {
                    return Err(format!(
                        "{} is {}; only an accepted ticket can be closed (record a PASS verdict first)",
                        t.id, t.status
                    ));
                }
                let implemented = last_index(t, "status:implemented")
                    .ok_or_else(|| format!("{} was never implemented", t.id))?;
                let verdict = match last_index(t, "verdict:PASS") {
                    Some(i) if i > implemented => t.history[i].clone(),
                    _ => return Err(format!("{}: no PASS verdict since the last implementation", t.id)),
                };
                evidence_unchanged(root, &verdict, "verdict")?;
                let audit = if t.audit_required {
                    match last_index(t, "audit:PASS") {
                        Some(i) if i > implemented => Some(t.history[i].clone()),
                        _ => return Err(format!("{}: audit is required and no PASS audit was recorded since the last implementation", t.id)),
                    }
                } else {
                    None
                };
                if let Some(a) = &audit {
                    evidence_unchanged(root, a, "audit")?;
                }
                let security = if t.security_required {
                    match last_index(t, "security:PASS") {
                        Some(i) if i > implemented => Some(t.history[i].clone()),
                        _ => return Err(format!("{}: a security review is required and no PASS was recorded since the last implementation", t.id)),
                    }
                } else {
                    None
                };
                if let Some(sec) = &security {
                    evidence_unchanged(root, sec, "security review")?;
                }
                let closed_at = now_iso();
                t.receipt = Some(Receipt {
                    closed_at,
                    commit: commit.clone(),
                    tree_clean: tree_clean.clone(),
                    ticket_sha256: ticket_hash(t),
                    verdict_at: verdict.at.clone(),
                    verdict_evidence: verdict.evidence.clone(),
                    verdict_sha256: verdict.sha256.clone(),
                    audit_at: audit.as_ref().map(|a| a.at.clone()).unwrap_or_default(),
                    audit_evidence: audit.as_ref().map(|a| a.evidence.clone()).unwrap_or_default(),
                    audit_sha256: audit.as_ref().map(|a| a.sha256.clone()).unwrap_or_default(),
                    security_at: security.as_ref().map(|a| a.at.clone()).unwrap_or_default(),
                    security_evidence: security.as_ref().map(|a| a.evidence.clone()).unwrap_or_default(),
                    security_sha256: security.as_ref().map(|a| a.sha256.clone()).unwrap_or_default(),
                });
                t.status = "closed".into();
                t.history.push(event("tester", "closed", "", "", ""));
                Ok(())
            })?;
            let short = &commit[..commit.len().min(12)];
            let mut out = format!("{} closed, receipt written (commit {short}, tree clean: {tree_clean})", ticket.id);
            if tree_clean == "no" {
                out.push_str(". Uncommitted changes exist: commit them so the receipt's commit covers the accepted state");
            }
            Ok(out)
        }
        "spec_propose" => crate::spec::propose(root, args),
        other => Err(format!("unknown tool '{other}'")),
    }
}
