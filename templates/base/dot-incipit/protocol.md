# Team protocol

Agents talk to each other in two fixed blocks. Details live in the ticket and
in evidence files; the blocks carry ids, paths and verdicts only.

## HANDOVER (lead to worker)

```
HANDOVER
ticket: T-0007
role: implementer
order: implement | build | verify | close | audit | security | research | plan
scope: <one sentence>
spec: SPEC.md#<section>
accept: <"ticket" or the criteria that matter for this order>
stop: <when to stop and report instead of continuing>
```

No worker starts without a complete block. The worker reads the ticket itself
with `task_get`; ticket and spec text are never pasted into the block.

## STATUS (worker to lead)

```
STATUS
ticket: T-0007
role: implementer
verdict: DONE | PASS | FAIL | BLOCKED
state: <ticket state now>
evidence: <path, or "-">
note: <one line, or "-">
```

- `DONE` is for architect, implementer and researcher, `PASS` and `FAIL` for
  tester, auditor and security, `BLOCKED` for everyone.
- The lead reads the evidence file only on `FAIL` or `BLOCKED`.
- The lead confirms `state` with `task_get` before the next step.

## Ticket states

```
open -> in_progress -> implemented -> accepted -> closed
         ^   |  ^            |
         |   v  |            +-- builder withdraws its work: back to in_progress
         | blocked
         +-- FAIL verdict or FAIL audit: back to open

any unfinished state -> withdrawn   (owner only)
```

Every ticket has a `role`: who builds it. `implementer` tickets are built by
the implementer and verified by the tester. `tester` tickets (test tools,
harness, checks) are built and verified by the tester and always need an
audit, so that the builder is never the only judge.

| Step | Tool | Role |
|---|---|---|
| create | `task_create` | architect |
| add a clarification or criterion, raise priority, require audit (open or blocked only) | `task_amend` | architect |
| open -> in_progress -> implemented, blocked | `task_status` | the ticket's role |
| implemented -> accepted, or back to open | `task_verdict` | tester |
| audit result, FAIL sends back to open | `task_audit` | auditor |
| security review result, FAIL sends back to open | `task_security` | security (optional role) |
| accepted -> closed | `task_close` | tester |
| one-line note | `task_comment` | all but the lead |
| read | `task_list`, `task_get`, `decision_list` | everyone |
| record a decision, question, answer or note | `decision_log` | lead |
| propose a spec change | `spec_propose` | architect |
| take a ticket out of the queue | `incipit ticket withdraw <id> <reason>` | owner |
| apply or reject a spec proposal | `incipit spec apply <id>`, `incipit spec reject <id>` | owner |
| switch an optional role on | `incipit role add security\|researcher` | owner |
| see what is due | `incipit status` | owner |

## Optional roles

`optional_roles` in `.incipit/config.json` lists the roles a project uses
beyond the five core ones.

- **security** reviews tickets marked `security_required`. Such a ticket
  cannot close without its PASS, recorded after the last implementation.
  Which tickets need it is `security_policy` in the config.
- **researcher** answers one precise question from the primary source and
  writes the answer to `docs/research/`. It has no ticket state of its own.

## Shells

Implementer and tester have a shell. Auditor, security and researcher have a
read-only git shell: the guard lets through one read-only git command per
call and nothing else. The lead has no shell. No role may change git
configuration or write below `.git/`.

## Decision log

The lead keeps no memory between sessions. Decisions of the owner, open
questions and their answers go into the decision log (`decision_log`), which
is append-only: a question is closed by an answer that refers to it.
`incipit decisions` shows it in a terminal.

## Tickets only grow

A ticket is never rewritten. What it asks for is fixed when it is created;
afterwards it can gain clarifications and criteria, nothing can be removed or
reworded. Asking for less takes a new ticket with `supersedes`, and the owner
withdrawing the old one. Every weakening therefore passes the owner.

## Closing and receipts

`task_close` is refused unless a PASS verdict, for tickets with
`audit_required` a PASS audit, and for tickets with `security_required` a
PASS security review, was recorded after the last implementation.
Verdicts and audits need a non-empty evidence file below `.incipit/evidence/`.
The file is hashed when it is recorded; if it changes afterwards, closing is
refused until the verdict or audit is recorded again.

Closing writes a receipt into the ticket: the git commit and whether the tree
was clean, the evidence files with their hashes, and a hash over the ticket's
criteria and amendments. `incipit ticket verify` checks all closed tickets
against their receipts.

Builders commit before they report: the implementer before `implemented`, the
tester before the verdict and before closing. They commit with `git add -A`,
which takes along the ticket state and the documents of roles without a
shell.

Ticket files in `.incipit/tasks/`, proposals in `.incipit/proposals/` and the
decision log `.incipit/decisions.json` are written by the ticket server only.
