#!/usr/bin/env bash
# End-to-end check of the incipit binary: ticket lifecycle over MCP and the
# guard's decisions. Usage: smoke.sh <path-to-incipit-binary>
set -uo pipefail

BIN="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
export INCIPIT_PROJECT="$WORK"
unset CLAUDE_PROJECT_DIR

mkdir -p "$WORK/.incipit/evidence" "$WORK/.claude/agents" "$WORK/src" "$WORK/tests" "$WORK/docs"
cat > "$WORK/.incipit/config.json" <<'JSON'
{
  "backend": "files",
  "roles": {
    "lead": {"write_allow": [], "write_deny": []},
    "architect": {"write_allow": ["docs/"], "write_deny": []},
    "implementer": {"write_allow": ["."], "write_deny": ["tests/", ".incipit/"]},
    "tester": {"write_allow": ["tests/", ".incipit/evidence/"], "write_deny": []},
    "auditor": {"write_allow": [".incipit/evidence/"], "write_deny": []}
  }
}
JSON

PASS=0
FAIL=0
ID=0

ok()  { PASS=$((PASS + 1)); }
bad() { FAIL=$((FAIL + 1)); echo "FAIL: $1"; echo "      got: $2"; }

# call <tool> <json-arguments>: one tools/call round trip, prints the reply.
call() {
  ID=$((ID + 1))
  printf '{"jsonrpc":"2.0","id":%d,"method":"tools/call","params":{"name":"%s","arguments":%s}}\n' "$ID" "$1" "$2" | "$BIN" mcp
}

expect() { # <description> <needle> <haystack>
  case "$3" in *"$2"*) ok ;; *) bad "$1" "$3" ;; esac
}

refuse() { # <description> <haystack>
  case "$2" in *'"isError":true'*) ok ;; *) bad "$1 (should be refused)" "$2" ;; esac
}

# guard <description> <allow|deny> <hook-json>
guard() {
  local out
  out="$(printf '%s' "$3" | "$BIN" guard)"
  if [ "$2" = deny ]; then
    case "$out" in *'"permissionDecision":"deny"'*) ok ;; *) bad "guard: $1 (should deny)" "$out" ;; esac
  else
    if [ -z "$out" ]; then ok; else bad "guard: $1 (should allow)" "$out"; fi
  fi
}

# --- protocol -----------------------------------------------------------
out="$(printf '%s\n%s\n%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{}}}' \
  '{"jsonrpc":"2.0","method":"notifications/initialized"}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/list"}' | "$BIN" mcp)"
expect "initialize answers" '"serverInfo"' "$out"
expect "tools/list has task_close" '"task_close"' "$out"
[ "$(printf '%s\n' "$out" | wc -l | tr -d ' ')" = 2 ] && ok || bad "notification must not be answered" "$out"
out="$(printf '%s\n' '{"jsonrpc":"2.0","id":3,"method":"nope"}' | "$BIN" mcp)"
expect "unknown method is an error" '-32601' "$out"
out="$(printf '%s\n' 'not json' | "$BIN" mcp)"
expect "garbage is a parse error" '-32700' "$out"

# --- ticket lifecycle ---------------------------------------------------
out="$(call task_create '{"title":"Parse config","scope":"Read config.toml into a struct","spec_ref":"SPEC.md#3","acceptance":["AC-1 valid file parses","AC-2 invalid file is rejected"]}')"
expect "create" 'created T-0001' "$out"
out="$(call task_create '{"title":"Audited thing","scope":"x","spec_ref":"SPEC.md#4","acceptance":["AC-3"],"audit_required":true,"priority":"P1"}')"
expect "create second" 'created T-0002' "$out"
out="$(call task_create '{"title":"No criteria","scope":"x","spec_ref":"SPEC.md#4","acceptance":[]}')"
refuse "create without acceptance" "$out"

out="$(call task_list '{}')"
expect "list shows both" 'tasks[2]' "$out"
expect "list line format" 'T-0002,open,P1,implementer,audit,Audited thing' "$out"

refuse "close an open ticket" "$(call task_close '{"id":"T-0001"}')"
refuse "verdict on an open ticket" "$(call task_verdict '{"id":"T-0001","verdict":"PASS","evidence":".incipit/evidence/T-0001-test.md"}')"
refuse "skip in_progress" "$(call task_status '{"id":"T-0001","role":"implementer","status":"implemented"}')"
refuse "path traversal id" "$(call task_get '{"id":"../../etc/passwd"}')"

expect "start" 'in_progress' "$(call task_status '{"id":"T-0001","role":"implementer","status":"in_progress"}')"
refuse "amend while in progress" "$(call task_amend '{"id":"T-0001","clarification":"x"}')"
refuse "tester moves an implementer ticket" "$(call task_status '{"id":"T-0001","role":"tester","status":"implemented"}')"
refuse "block without note" "$(call task_status '{"id":"T-0001","role":"implementer","status":"blocked"}')"
expect "implemented" 'implemented' "$(call task_status '{"id":"T-0001","role":"implementer","status":"implemented"}')"

refuse "verdict without evidence file" "$(call task_verdict '{"id":"T-0001","verdict":"PASS","evidence":".incipit/evidence/T-0001-test.md"}')"
: > "$WORK/.incipit/evidence/T-0001-test.md"
refuse "verdict with empty evidence" "$(call task_verdict '{"id":"T-0001","verdict":"PASS","evidence":".incipit/evidence/T-0001-test.md"}')"
echo "AC-1 PASS, AC-2 FAIL" > "$WORK/.incipit/evidence/T-0001-test.md"
echo "x" > "$WORK/src/fake.md"
refuse "evidence outside evidence dir" "$(call task_verdict '{"id":"T-0001","verdict":"PASS","evidence":"src/fake.md"}')"
refuse "evidence escaping the project" "$(call task_verdict '{"id":"T-0001","verdict":"PASS","evidence":".incipit/evidence/../../../etc/hosts"}')"

expect "FAIL verdict reopens" 'status open' "$(call task_verdict '{"id":"T-0001","verdict":"FAIL","evidence":".incipit/evidence/T-0001-test.md","note":"AC-2 fails"}')"
refuse "close after FAIL" "$(call task_close '{"id":"T-0001"}')"

call task_status '{"id":"T-0001","role":"implementer","status":"in_progress"}' >/dev/null
call task_status '{"id":"T-0001","role":"implementer","status":"implemented"}' >/dev/null
refuse "close before the new verdict" "$(call task_close '{"id":"T-0001"}')"
expect "PASS verdict accepts" 'status accepted' "$(call task_verdict '{"id":"T-0001","verdict":"PASS","evidence":".incipit/evidence/T-0001-test.md"}')"
echo "edited after the verdict" >> "$WORK/.incipit/evidence/T-0001-test.md"
out="$(call task_close '{"id":"T-0001"}')"
refuse "close with evidence changed after the verdict" "$out"
expect "refusal names the changed evidence" 'was changed after it was recorded' "$out"
echo "AC-1 PASS, AC-2 FAIL" > "$WORK/.incipit/evidence/T-0001-test.md"
out="$(call task_close '{"id":"T-0001"}')"
expect "close accepted ticket" 'T-0001 closed, receipt written' "$out"
expect "no git: receipt says so" 'commit none' "$out"
out="$(call task_get '{"id":"T-0001"}')"
expect "receipt is shown" 'receipt:' "$out"
expect "receipt carries the evidence hash" 'T-0001-test.md sha256=' "$out"
expect "receipt carries the ticket hash" 'ticket_sha256: ' "$out"
refuse "close twice" "$(call task_close '{"id":"T-0001"}')"
refuse "comment on closed" "$(call task_comment '{"id":"T-0001","role":"tester","text":"late"}')"

# audit_required
call task_status '{"id":"T-0002","role":"implementer","status":"in_progress"}' >/dev/null
call task_status '{"id":"T-0002","role":"implementer","status":"implemented"}' >/dev/null
echo "all PASS" > "$WORK/.incipit/evidence/T-0002-test.md"
echo "audit ok" > "$WORK/.incipit/evidence/T-0002-audit.md"
call task_verdict '{"id":"T-0002","verdict":"PASS","evidence":".incipit/evidence/T-0002-test.md"}' >/dev/null
out="$(call task_close '{"id":"T-0002"}')"
refuse "close without required audit" "$out"
expect "refusal names the audit" 'audit is required' "$out"
expect "audit PASS" 'audit PASS recorded' "$(call task_audit '{"id":"T-0002","verdict":"PASS","evidence":".incipit/evidence/T-0002-audit.md"}')"
expect "close after audit" 'T-0002 closed' "$(call task_close '{"id":"T-0002"}')"
out="$(call task_get '{"id":"T-0002","full_history":true}')"
expect "history records the audit" 'auditor audit:PASS' "$out"
expect "list filter" 'tasks[2]' "$(call task_list '{"status":"closed"}')"

# --- tickets only grow --------------------------------------------------
call task_create '{"title":"Growing","scope":"s","spec_ref":"SPEC.md#5","acceptance":["AC-5 first"]}' >/dev/null   # T-0003
expect "task_update no longer exists" "unknown tool 'task_update'" "$(call task_update '{"id":"T-0003","title":"x"}')"
refuse "amend with nothing" "$(call task_amend '{"id":"T-0003"}')"
refuse "audit cannot be turned off" "$(call task_amend '{"id":"T-0003","require_audit":false}')"
expect "amend adds" 'amended T-0003 (2 amendments)' "$(call task_amend '{"id":"T-0003","clarification":"Means the TT build","add_criterion":"AC-6 second","require_audit":true,"priority":"P1"}')"
out="$(call task_get '{"id":"T-0003"}')"
expect "original criterion stays" '1. AC-5 first' "$out"
expect "added criterion is marked" '2. AC-6 second (added A2' "$out"
expect "clarification is listed" 'A1 ' "$out"
expect "audit is now required" 'audit_required: true' "$out"
expect "priority changed" 'priority: P1' "$out"
refuse "supersede an unknown ticket" "$(call task_create '{"title":"x","scope":"s","spec_ref":"SPEC.md#5","acceptance":["a"],"supersedes":"T-0999"}')"
expect "supersede" 'T-0003 stays as it is until the owner withdraws it' "$(call task_create '{"title":"Growing, recut","scope":"s","spec_ref":"SPEC.md#5","acceptance":["AC-5 first"],"supersedes":"T-0003"}')"   # T-0004
expect "owner withdraws" 'T-0003 withdrawn' "$("$BIN" ticket withdraw T-0003 superseded by T-0004)"
out="$("$BIN" ticket withdraw T-0004 2>&1)"
expect "withdraw needs a reason" 'a reason is required' "$out"
refuse "withdrawn ticket cannot start" "$(call task_status '{"id":"T-0003","role":"implementer","status":"in_progress"}')"
refuse "withdrawn ticket cannot be amended" "$(call task_amend '{"id":"T-0003","clarification":"x"}')"
out="$("$BIN" ticket withdraw T-0001 late 2>&1)"
expect "closed ticket cannot be withdrawn" 'already closed' "$out"
expect "list shows role and withdrawn" 'T-0003,withdrawn,P1,implementer,audit,Growing' "$(call task_list '{}')"

# --- tester-built tickets -----------------------------------------------
out="$(call task_create '{"title":"Harness tool","scope":"s","spec_ref":"SPEC.md#7","acceptance":["tool fails on a wrong input"],"role":"tester"}')"   # T-0005
expect "tester ticket" 'role tester' "$out"
expect "tester ticket forces the audit" 'audit_required: true' "$(call task_get '{"id":"T-0005"}')"
refuse "implementer moves a tester ticket" "$(call task_status '{"id":"T-0005","role":"implementer","status":"in_progress"}')"
expect "tester starts its ticket" 'in_progress' "$(call task_status '{"id":"T-0005","role":"tester","status":"in_progress"}')"
call task_status '{"id":"T-0005","role":"tester","status":"implemented"}' >/dev/null
echo "tool run" > "$WORK/.incipit/evidence/T-0005-test.md"
echo "audit" > "$WORK/.incipit/evidence/T-0005-audit.md"
call task_verdict '{"id":"T-0005","verdict":"PASS","evidence":".incipit/evidence/T-0005-test.md"}' >/dev/null
refuse "tester cannot close its own ticket unaudited" "$(call task_close '{"id":"T-0005"}')"
call task_audit '{"id":"T-0005","verdict":"PASS","evidence":".incipit/evidence/T-0005-audit.md"}' >/dev/null
echo "audit, edited" > "$WORK/.incipit/evidence/T-0005-audit.md"
refuse "close with audit evidence changed" "$(call task_close '{"id":"T-0005"}')"
echo "audit" > "$WORK/.incipit/evidence/T-0005-audit.md"
expect "tester ticket closes after the audit" 'T-0005 closed' "$(call task_close '{"id":"T-0005"}')"
expect "owner sets a role" 'now has role tester (audit required: true)' "$("$BIN" ticket role T-0004 tester)"

# --- receipts -------------------------------------------------------------
out="$("$BIN" ticket verify)"
expect "verify passes on untouched evidence" 'verified: 3 ok, 0 mismatch, 0 without receipt' "$out"
echo "rewritten later" > "$WORK/.incipit/evidence/T-0002-test.md"
out="$("$BIN" ticket verify 2>&1)"; rc=$?
expect "verify names the changed evidence" 'T-0002 MISMATCH: verdict evidence .incipit/evidence/T-0002-test.md was changed' "$out"
[ "$rc" != 0 ] && ok || bad "verify exits non-zero on mismatch" "$rc"
echo "all PASS" > "$WORK/.incipit/evidence/T-0002-test.md"

# receipt with a git repository
if command -v git >/dev/null 2>&1; then
  git -C "$WORK" init -q && git -C "$WORK" -c user.name=t -c user.email=t@t add -A >/dev/null 2>&1
  git -C "$WORK" -c user.name=t -c user.email=t@t commit -qm base
  call task_create '{"title":"With git","scope":"s","spec_ref":"SPEC.md#5","acceptance":["a"]}' >/dev/null   # T-0006
  call task_status '{"id":"T-0006","role":"implementer","status":"in_progress"}' >/dev/null
  call task_status '{"id":"T-0006","role":"implementer","status":"implemented"}' >/dev/null
  echo "ok" > "$WORK/.incipit/evidence/T-0006-test.md"
  call task_verdict '{"id":"T-0006","verdict":"PASS","evidence":".incipit/evidence/T-0006-test.md"}' >/dev/null
  out="$(call task_close '{"id":"T-0006"}')"
  expect "uncommitted evidence makes the tree dirty" 'tree clean: no' "$out"
  expect "dirty tree is pointed out" 'Uncommitted changes exist' "$out"
  expect "receipt has the commit" "commit $(git -C "$WORK" rev-parse HEAD | cut -c1-12)" "$out"
  call task_create '{"title":"With git, clean","scope":"s","spec_ref":"SPEC.md#5","acceptance":["a"]}' >/dev/null   # T-0007
  call task_status '{"id":"T-0007","role":"implementer","status":"in_progress"}' >/dev/null
  call task_status '{"id":"T-0007","role":"implementer","status":"implemented"}' >/dev/null
  echo "ok" > "$WORK/.incipit/evidence/T-0007-test.md"
  git -C "$WORK" -c user.name=t -c user.email=t@t add -A >/dev/null 2>&1
  git -C "$WORK" -c user.name=t -c user.email=t@t commit -qm "T-0007: evidence"
  call task_verdict '{"id":"T-0007","verdict":"PASS","evidence":".incipit/evidence/T-0007-test.md"}' >/dev/null
  expect "ticket files alone do not make the tree dirty" 'tree clean: yes' "$(call task_close '{"id":"T-0007"}')"
  rm -rf "$WORK/.git"
fi

# --- spec proposals -------------------------------------------------------
printf '# Spec\n\n## 10. Existing work\n\n| Start-up | main.c | adopt |\n| Resource | zdesk.rsc | adopt |\n' > "$WORK/SPEC.md"
refuse "proposal for a passage that is not there" "$(call spec_propose '{"reason":"r","edits":[{"old":"nope","new":"x"}]}')"
refuse "proposal for an ambiguous passage" "$(call spec_propose '{"reason":"r","edits":[{"old":"adopt","new":"rewrite"}]}')"
refuse "proposal without a reason" "$(call spec_propose '{"edits":[{"old":"main.c | adopt","new":"main.c | rewrite"}]}')"
expect "proposal is filed" 'proposal P-0001 filed with 2 edit(s)' "$(call spec_propose '{"reason":"Owner 2026-11-20: start-up and resource are rewritten","edits":[{"old":"main.c | adopt","new":"main.c | rewrite"},{"old":"zdesk.rsc | adopt","new":"zdesk.rsc | rewrite"}]}')"
case "$(cat "$WORK/SPEC.md")" in *rewrite*) bad "a proposal must not change SPEC.md" "changed" ;; *) ok ;; esac
expect "owner lists proposals" 'P-0001 pending 2 edit(s)' "$("$BIN" spec list)"
out="$("$BIN" spec show P-0001)"
expect "show prints the old text" '  - main.c | adopt' "$out"
expect "show prints the new text" '  + main.c | rewrite' "$out"
expect "owner applies" 'P-0001 applied to SPEC.md' "$("$BIN" spec apply P-0001)"
expect "SPEC.md is changed" '| Start-up | main.c | rewrite |' "$(cat "$WORK/SPEC.md")"
out="$("$BIN" spec apply P-0001 2>&1)"
expect "a proposal applies once" 'already applied' "$out"
call spec_propose '{"reason":"second","edits":[{"old":"# Spec","new":"# Specification"}]}' >/dev/null
printf '# Other\n' > "$WORK/SPEC.md"
out="$("$BIN" spec apply P-0002 2>&1)"
expect "stale proposal is refused" 'nothing was applied' "$out"
expect "owner rejects" 'P-0002 rejected' "$("$BIN" spec reject P-0002 outdated)"

# unknown backend is refused, not silently replaced
sed -i.bak 's/"backend": "files"/"backend": "github"/' "$WORK/.incipit/config.json"
refuse "unimplemented backend" "$(call task_list '{}')"
mv "$WORK/.incipit/config.json.bak" "$WORK/.incipit/config.json"

# --- guard --------------------------------------------------------------
hook() { # <agent_type or ""> <tool_name> <tool_input-json>
  if [ -n "$1" ]; then
    printf '{"hook_event_name":"PreToolUse","cwd":"%s","agent_type":"%s","tool_name":"%s","tool_input":%s}' "$WORK" "$1" "$2" "$3"
  else
    printf '{"hook_event_name":"PreToolUse","cwd":"%s","tool_name":"%s","tool_input":%s}' "$WORK" "$2" "$3"
  fi
}

# ticket tools per role
guard "tester may close"            allow "$(hook tester mcp__incipit__task_close '{"id":"T-0001"}')"
for role in lead architect implementer auditor; do
  guard "$role may not close"       deny  "$(hook $role mcp__incipit__task_close '{"id":"T-0001"}')"
done
guard "owner session may not close" deny  "$(hook '' mcp__incipit__task_close '{"id":"T-0001"}')"
guard "unknown agent may not close" deny  "$(hook general-purpose mcp__incipit__task_close '{"id":"T-0001"}')"
guard "lead may read"               allow "$(hook lead mcp__incipit__task_get '{"id":"T-0001"}')"
guard "lead may not create"         deny  "$(hook lead mcp__incipit__task_create '{}')"
guard "lead may not comment"        deny  "$(hook lead mcp__incipit__task_comment '{"id":"T-0001","role":"lead","text":"x"}')"
guard "architect may create"        allow "$(hook architect mcp__incipit__task_create '{}')"
guard "implementer may not create"  deny  "$(hook implementer mcp__incipit__task_create '{}')"
guard "implementer may not verdict" deny  "$(hook implementer mcp__incipit__task_verdict '{}')"
guard "auditor may audit"           allow "$(hook auditor mcp__incipit__task_audit '{}')"
guard "tester may not audit"        deny  "$(hook tester mcp__incipit__task_audit '{}')"
guard "architect may amend"         allow "$(hook architect mcp__incipit__task_amend '{}')"
guard "implementer may not amend"   deny  "$(hook implementer mcp__incipit__task_amend '{}')"
guard "architect may propose"       allow "$(hook architect mcp__incipit__spec_propose '{}')"
guard "lead may not propose"        deny  "$(hook lead mcp__incipit__spec_propose '{}')"
guard "unknown agent may not propose" deny "$(hook general-purpose mcp__incipit__spec_propose '{}')"
guard "implementer sets own status" allow "$(hook implementer mcp__incipit__task_status '{"id":"T-0001","role":"implementer","status":"in_progress"}')"
guard "tester sets own status"      allow "$(hook tester mcp__incipit__task_status '{"id":"T-0001","role":"tester","status":"in_progress"}')"
guard "tester posing as implementer" deny "$(hook tester mcp__incipit__task_status '{"id":"T-0001","role":"implementer","status":"implemented"}')"
guard "architect may not set status" deny "$(hook architect mcp__incipit__task_status '{"id":"T-0001","role":"implementer","status":"in_progress"}')"
guard "no direct write to proposals" deny "$(hook architect Write '{"file_path":".incipit/proposals/P-0009.json"}')"
guard "no shell access to proposals" deny "$(hook implementer Bash '{"command":"cat .incipit/proposals/P-0001.json"}')"
guard "role may not withdraw via CLI" deny "$(hook tester Bash '{"command":"incipit ticket withdraw T-0001 x"}')"
guard "comment under own role"      allow "$(hook implementer mcp__incipit__task_comment '{"id":"T-0001","role":"implementer","text":"x"}')"
guard "comment under foreign role"  deny  "$(hook implementer mcp__incipit__task_comment '{"id":"T-0001","role":"tester","text":"x"}')"

# direct writes to ticket files: nobody
for role in '' lead architect implementer tester auditor; do
  guard "'$role' may not edit a ticket file" deny "$(hook "$role" Edit '{"file_path":".incipit/tasks/T-0001.json"}')"
done
guard "absolute ticket path"        deny  "$(hook tester Write "{\"file_path\":\"$WORK/.incipit/tasks/T-0009.json\"}")"
guard "dot-dot ticket path"         deny  "$(hook tester Write '{"file_path":"tests/../.incipit/tasks/T-0009.json"}')"
guard "case-variant ticket path"    deny  "$(hook tester Write '{"file_path":".Incipit/Tasks/T-0009.json"}')"
ln -s "$WORK/.incipit/tasks" "$WORK/tests/link"
guard "symlink into tickets"        deny  "$(hook tester Write '{"file_path":"tests/link/T-0009.json"}')"

# role write paths
guard "implementer writes source"   allow "$(hook implementer Write '{"file_path":"src/main.rs"}')"
guard "implementer not tests"       deny  "$(hook implementer Edit '{"file_path":"tests/a.rs"}')"
guard "implementer not evidence"    deny  "$(hook implementer Write '{"file_path":".incipit/evidence/T-0001-test.md"}')"
guard "implementer not settings"    deny  "$(hook implementer Edit '{"file_path":".claude/settings.json"}')"
guard "implementer not agents"      deny  "$(hook implementer Write '{"file_path":".claude/agents/tester.md"}')"
guard "implementer not mcp config"  deny  "$(hook implementer Write '{"file_path":".mcp.json"}')"
guard "implementer not role config" deny  "$(hook implementer Edit '{"file_path":".incipit/config.json"}')"
guard "implementer not outside"     deny  "$(hook implementer Write '{"file_path":"/tmp/x"}')"
guard "tester writes tests"         allow "$(hook tester Write '{"file_path":"tests/a.rs"}')"
guard "tester writes evidence"      allow "$(hook tester Write '{"file_path":".incipit/evidence/T-0001-test.md"}')"
guard "tester not source"           deny  "$(hook tester Edit '{"file_path":"src/main.rs"}')"
guard "architect writes docs"       allow "$(hook architect Write '{"file_path":"docs/adr-001.md"}')"
guard "architect not source"        deny  "$(hook architect Write '{"file_path":"src/main.rs"}')"
guard "auditor writes evidence"     allow "$(hook auditor Write '{"file_path":".incipit/evidence/T-0001-audit.md"}')"
guard "auditor not source"          deny  "$(hook auditor Write '{"file_path":"src/main.rs"}')"
guard "lead writes nothing"         deny  "$(hook lead Write '{"file_path":"docs/x.md"}')"
guard "unknown agent writes nothing" deny "$(hook Explore Write '{"file_path":"src/main.rs"}')"
guard "owner writes source"         allow "$(hook '' Write '{"file_path":"src/main.rs"}')"
guard "owner writes settings"       allow "$(hook '' Edit '{"file_path":".claude/settings.json"}')"

# shell
guard "implementer runs cargo"      allow "$(hook implementer Bash '{"command":"cargo test"}')"
guard "shell touching tickets"      deny  "$(hook implementer Bash '{"command":"sed -i s/open/closed/ .incipit/tasks/T-0001.json"}')"
guard "owner shell touching tickets" deny "$(hook '' Bash '{"command":"rm .incipit/tasks/T-0001.json"}')"
guard "shell touching settings"     deny  "$(hook tester Bash '{"command":"echo {} > .claude/settings.json"}')"
guard "role runs incipit CLI"       deny  "$(hook implementer Bash '{"command":"~/.incipit/bin/incipit deactivate"}')"
guard "lead runs no shell"          deny  "$(hook lead Bash '{"command":"ls"}')"
guard "owner runs incipit CLI"      allow "$(hook '' Bash '{"command":"~/.incipit/bin/incipit phase show"}')"
guard "other tools pass"            allow "$(hook lead Read '{"file_path":"SPEC.md"}')"

# missing role rules fail closed
mv "$WORK/.incipit/config.json" "$WORK/.incipit/config.json.off"
guard "no config: role write refused" deny "$(hook implementer Write '{"file_path":"src/main.rs"}')"
mv "$WORK/.incipit/config.json.off" "$WORK/.incipit/config.json"
out="$(printf 'garbage' | "$BIN" guard)"
expect "unreadable hook input is denied" '"deny"' "$out"

# --- owner CLI ----------------------------------------------------------
out="$("$BIN" phase show)"
expect "phases start pending" 'next: scaffold' "$out"
out="$("$BIN" activate 2>&1)"
expect "activate needs the lead brief" 'lead.md is missing' "$out"
out="$("$BIN" phase done scaffold 2>&1)"
expect "scaffold cannot be done without its files" 'scaffold is incomplete' "$out"
expect "missing files are named" 'CLAUDE.md' "$out"
echo '{"hooks":{}}' > "$WORK/.claude/settings.json"
echo '{}' > "$WORK/.mcp.json"
for f in CLAUDE.md SPEC.md .incipit/protocol.md; do echo x > "$WORK/$f"; done
for a in lead architect implementer tester auditor; do echo x > "$WORK/.claude/agents/$a.md"; done
out="$("$BIN" phase done grill 2>&1)"
expect "phases are ordered" "must be done before" "$out"
"$BIN" phase done scaffold >/dev/null
out="$("$BIN" phase done grill)"
expect "next phase advances" 'next: toolchain' "$out"
# The folder is emptied by hand while the hidden state file stays behind.
mv "$WORK/SPEC.md" "$WORK/SPEC.md.gone"
out="$("$BIN" phase show)"
expect "emptied scaffold is reported" 'scaffold: incomplete (missing: SPEC.md)' "$out"
expect "emptied scaffold is next again" 'next: scaffold' "$out"
out="$("$BIN" phase done toolchain 2>&1)"
expect "later phases wait for the scaffold" 'scaffold is incomplete' "$out"
mv "$WORK/SPEC.md.gone" "$WORK/SPEC.md"
expect "restored scaffold continues" 'next: toolchain' "$("$BIN" phase show)"
out="$("$BIN" phase reset scaffold)"
expect "reset clears later phases" 'next: scaffold' "$out"
out="$("$BIN" guard --help)"
expect "guard explains itself when asked" 'PreToolUse hook' "$out"
"$BIN" activate >/dev/null
expect "activate sets the agent" '"agent": "lead"' "$(cat "$WORK/.claude/settings.json")"
expect "activate keeps other settings" '"hooks"' "$(cat "$WORK/.claude/settings.json")"
"$BIN" deactivate >/dev/null
case "$(cat "$WORK/.claude/settings.json")" in *'"agent"'*) bad "deactivate removes the agent" "$(cat "$WORK/.claude/settings.json")" ;; *) ok ;; esac

# --- upgrade of an older project ----------------------------------------
for a in lead architect implementer tester auditor; do
  printf -- '---\nname: %s\ndescription: old\ntools: Read, mcp__incipit__task_update\n---\n\nProject text for %s.\n' "$a" "$a" > "$WORK/.claude/agents/$a.md"
done
out="$("$BIN" upgrade)"
expect "upgrade reports the briefs" 'updated .claude/agents/tester.md (tools list, rules section)' "$out"
expect "upgrade replaces the protocol" 'updated .incipit/protocol.md' "$out"
expect "tester gains task_status" 'mcp__incipit__task_status' "$(sed -n 4p "$WORK/.claude/agents/tester.md")"
case "$(cat "$WORK/.claude/agents/architect.md")" in *task_update,*|*"task_update"$'\n'*) bad "architect loses task_update" "still listed" ;; *) ok ;; esac
expect "architect gains spec_propose" 'mcp__incipit__spec_propose' "$(sed -n 4p "$WORK/.claude/agents/architect.md")"
expect "project text is kept" 'Project text for tester.' "$(cat "$WORK/.claude/agents/tester.md")"
expect "rules section is appended" 'Rules changed in Incipit 0.1.2' "$(cat "$WORK/.claude/agents/lead.md")"
out="$("$BIN" upgrade)"
expect "upgrade is idempotent" 'kept    .claude/agents/tester.md' "$out"
[ "$(grep -c 'Rules changed in Incipit 0.1.2' "$WORK/.claude/agents/lead.md")" = 1 ] && ok || bad "rules section appended once" "more than once"

expect "0.1.3 rules are appended too" 'Rules changed in Incipit 0.1.3' "$(cat "$WORK/.claude/agents/lead.md")"
expect "lead gains the decision log" 'mcp__incipit__decision_log' "$(sed -n 4p "$WORK/.claude/agents/lead.md")"
[ "$(grep -c 'incipit-update 0.1.3' "$WORK/.claude/agents/auditor.md")" = 1 ] && ok || bad "roles without new rules get the mark once" "$(grep -c 'incipit-update 0.1.3' "$WORK/.claude/agents/auditor.md")"

# --- optional roles -------------------------------------------------------
out="$(call task_create '{"title":"Needs security","scope":"s","spec_ref":"SPEC.md#5","acceptance":["a"],"security_required":true}')"
refuse "security review cannot be required without the role" "$out"
expect "refusal says how to add the role" 'incipit role add security' "$out"
out="$("$BIN" role add nonsense 2>&1)"
expect "unknown optional role" 'usage: incipit role add' "$out"
out="$("$BIN" role add security)"
expect "role add creates the brief" 'created .claude/agents/security.md' "$out"
expect "role add extends the lead" 'updated .claude/agents/lead.md (tools list)' "$out"
expect "lead may start the security agent" 'Agent(architect, implementer, tester, auditor, security)' "$(sed -n 4p "$WORK/.claude/agents/lead.md")"
expect "config lists the role" '"security"' "$(cat "$WORK/.incipit/config.json")"
out="$("$BIN" role add security)"
expect "role add is idempotent" 'kept    .claude/agents/security.md' "$out"
"$BIN" role add researcher >/dev/null
expect "both optional roles for the lead" 'auditor, security, researcher)' "$(sed -n 4p "$WORK/.claude/agents/lead.md")"
out="$("$BIN" upgrade)"
expect "upgrade keeps the lead's optional agents" 'kept    .claude/agents/lead.md' "$out"
expect "upgrade covers optional briefs" '.claude/agents/security.md' "$out"

out="$(call task_create '{"title":"Needs security","scope":"s","spec_ref":"SPEC.md#5","acceptance":["a"],"security_required":true}')"
expect "security ticket is created" 'created T-' "$out"
SID="$(printf '%s' "$out" | sed -n 's/.*created \(T-[0-9]*\).*/\1/p')"
expect "list shows the check" ",security,Needs security" "$(call task_list '{}')"
refuse "security requirement cannot be turned off" "$(call task_amend "{\"id\":\"$SID\",\"require_security\":false}")"
call task_status "{\"id\":\"$SID\",\"role\":\"implementer\",\"status\":\"in_progress\"}" >/dev/null
call task_status "{\"id\":\"$SID\",\"role\":\"implementer\",\"status\":\"implemented\"}" >/dev/null
echo "tested" > "$WORK/.incipit/evidence/$SID-test.md"
echo "reviewed" > "$WORK/.incipit/evidence/$SID-security.md"
call task_verdict "{\"id\":\"$SID\",\"verdict\":\"PASS\",\"evidence\":\".incipit/evidence/$SID-test.md\"}" >/dev/null
out="$(call task_close "{\"id\":\"$SID\"}")"
refuse "close without the security review" "$out"
expect "refusal names the security review" 'a security review is required' "$out"
expect "security FAIL reopens" 'status open' "$(call task_security "{\"id\":\"$SID\",\"verdict\":\"FAIL\",\"evidence\":\".incipit/evidence/$SID-security.md\"}")"
call task_status "{\"id\":\"$SID\",\"role\":\"implementer\",\"status\":\"in_progress\"}" >/dev/null
call task_status "{\"id\":\"$SID\",\"role\":\"implementer\",\"status\":\"implemented\"}" >/dev/null
call task_verdict "{\"id\":\"$SID\",\"verdict\":\"PASS\",\"evidence\":\".incipit/evidence/$SID-test.md\"}" >/dev/null
refuse "an old security PASS would not count; none yet after rework" "$(call task_close "{\"id\":\"$SID\"}")"
expect "security PASS" 'security review PASS recorded' "$(call task_security "{\"id\":\"$SID\",\"verdict\":\"PASS\",\"evidence\":\".incipit/evidence/$SID-security.md\"}")"
expect "close after the security review" "$SID closed" "$(call task_close "{\"id\":\"$SID\"}")"
expect "receipt carries the security evidence" "security: " "$(call task_get "{\"id\":\"$SID\"}")"

guard "security may record its review"   allow "$(hook security mcp__incipit__task_security '{}')"
guard "auditor may not record it"        deny  "$(hook auditor mcp__incipit__task_security '{}')"
guard "tester may not record it"         deny  "$(hook tester mcp__incipit__task_security '{}')"
guard "security may not close"           deny  "$(hook security mcp__incipit__task_close '{}')"
guard "security writes evidence"         allow "$(hook security Write '{"file_path":".incipit/evidence/T-0001-security.md"}')"
guard "security not source"              deny  "$(hook security Write '{"file_path":"src/main.rs"}')"
guard "researcher writes research notes" allow "$(hook researcher Write '{"file_path":"docs/research/cookie-jar.md"}')"
guard "researcher not source"            deny  "$(hook researcher Write '{"file_path":"src/main.rs"}')"
guard "researcher not other docs"        deny  "$(hook researcher Write '{"file_path":"docs/design/x.md"}')"
guard "researcher may comment"           allow "$(hook researcher mcp__incipit__task_comment '{"id":"T-0001","role":"researcher","text":"x"}')"
guard "researcher may not create"        deny  "$(hook researcher mcp__incipit__task_create '{}')"

# --- decision log ---------------------------------------------------------
guard "lead may log a decision"          allow "$(hook lead mcp__incipit__decision_log '{}')"
guard "implementer may not log"          deny  "$(hook implementer mcp__incipit__decision_log '{}')"
guard "owner session may not log"        deny  "$(hook '' mcp__incipit__decision_log '{}')"
guard "unknown agent may not log"        deny  "$(hook general-purpose mcp__incipit__decision_log '{}')"
guard "everyone may read the log"        allow "$(hook tester mcp__incipit__decision_list '{}')"
guard "owner session may read the log"   allow "$(hook '' mcp__incipit__decision_list '{}')"
guard "no direct write to the log"       deny  "$(hook lead Write '{"file_path":".incipit/decisions.json"}')"
guard "no shell access to the log"       deny  "$(hook implementer Bash '{"command":"echo [] > .incipit/decisions.json"}')"

refuse "log needs a kind" "$(call decision_log '{"kind":"rumour","text":"x"}')"
refuse "log needs a text" "$(call decision_log '{"kind":"note","text":"  "}')"
expect "log a question" 'logged D-0001 (question)' "$(call decision_log '{"kind":"question","text":"Does Q1 also apply to T-0003?"}')"
expect "log a decision" 'logged D-0002 (decision)' "$(call decision_log '{"kind":"decision","text":"Start-up is rewritten, minimal","ref":"T-0005"}')"
out="$(call decision_list '{}')"
expect "open question is listed" 'open questions[1]' "$out"
expect "decision shows its reference" 'decision [T-0005]: Start-up is rewritten, minimal' "$out"
refuse "answer without a question" "$(call decision_log '{"kind":"answer","text":"yes"}')"
refuse "answer to something that is no open question" "$(call decision_log '{"kind":"answer","text":"yes","ref":"D-0002"}')"
expect "answer closes the question" 'logged D-0003 (answer)' "$(call decision_log '{"kind":"answer","text":"Yes, same rule","ref":"D-0001"}')"
expect "no open question left" 'open questions[0]' "$(call decision_list '{"open_only":true}')"
refuse "a question is answered once" "$(call decision_log '{"kind":"answer","text":"again","ref":"D-0001"}')"
expect "owner reads the log" 'D-0003' "$("$BIN" decisions all)"
call decision_log '{"kind":"question","text":"Which TeraDesk binary?"}' >/dev/null

# --- owner overview -------------------------------------------------------
out="$("$BIN" ticket list open)"
expect "owner lists tickets" 'tasks[' "$out"
out="$("$BIN" ticket show T-0001)"
expect "owner shows a ticket with its receipt" 'receipt:' "$out"
printf '# Other\n' > "$WORK/SPEC.md"
call spec_propose '{"reason":"pending one","edits":[{"old":"# Other","new":"# Other spec"}]}' >/dev/null
call task_create '{"title":"Growing, third cut","scope":"s","spec_ref":"SPEC.md#5","acceptance":["a"],"supersedes":"T-0004"}' >/dev/null
out="$("$BIN" status)"
expect "status counts tickets" 'tickets: ' "$out"
expect "status shows pending proposals" 'spec proposals pending: 1' "$out"
expect "status shows open questions" 'Which TeraDesk binary?' "$out"
expect "status names the owner's step" 'incipit spec apply P-0003' "$out"
expect "status finds the superseded ticket still in the queue" 'incipit ticket withdraw T-0004 "superseded by T-' "$out"

echo "smoke: $PASS passed, $FAIL failed"
[ "$FAIL" = 0 ]
