# Incipit

*Deutsche Fassung: [README.de.md](README.de.md)*

Incipit is a plugin for Claude Code that sets up a software project the same
way every time and then runs it with a small team of agents whose rights are
enforced, not just described.

`/incipit:newproject` creates the configuration and the agents, questions you
until the project is defined (the "grill session"), builds the tooling that
checks whether the goal is reached, and hands the project to a lead agent.
From then on the work runs on tickets. A ticket server decides who may do
what: only the tester can close a ticket, and only with recorded evidence.

Version 0.1.5.

## Contents

1. [What problem it solves](#1-what-problem-it-solves)
2. [Systems and requirements](#2-systems-and-requirements)
3. [Installation](#3-installation)
4. [Setting up a project](#4-setting-up-a-project)
5. [The team](#5-the-team)
6. [Tickets](#6-tickets)
7. [Spec changes](#7-spec-changes)
8. [The decision log](#8-the-decision-log)
9. [How agents hand work over](#9-how-agents-hand-work-over)
10. [How the rules are enforced](#10-how-the-rules-are-enforced)
11. [Owner commands](#11-owner-commands)
12. [Files in a project](#12-files-in-a-project)
13. [Working with the team day to day](#13-working-with-the-team-day-to-day)
14. [Upgrading a project](#14-upgrading-a-project)
15. [Limits and open points](#15-limits-and-open-points)
16. [Developing Incipit](#16-developing-incipit)
17. [License](#license)

## 1. What problem it solves

A team of coding agents drifts in predictable ways. The coordinating agent
starts fixing things itself. The agent that wrote the code declares it done.
A failing check is quietly weakened. A ticket is reworded until it matches
what was built. Decisions made in one session are gone in the next.

Incipit answers each of these with a mechanism instead of an appeal:

| Drift | Mechanism |
|---|---|
| The coordinator does the work itself | The lead has no tool to write a file or run a command |
| The builder declares its own work done | Only the tester can close; tester-built work needs an auditor |
| Checks are weakened to pass | Check scripts belong to the tester; the implementer cannot write them |
| Tickets are reworded after the fact | Tickets are append-only; asking for less takes a new ticket and the owner |
| Evidence is edited afterwards | Evidence is hashed when recorded; closing writes a receipt |
| The spec changes unnoticed | No agent can write the spec; changes are proposals the owner applies |
| Decisions vanish between sessions | An append-only decision log that the lead reads at every start |

The person who owns the project is called the **owner** throughout. The owner
decides; agents recommend.

## 2. Systems and requirements

### Operating systems

| System | Status |
|---|---|
| macOS on Apple Silicon | Primary platform. Built and used on a Mac mini M4; the full flow (setup, import, team work, upgrades) has run there. |
| Linux x86_64 | Supported in principle. The binary is built and the test suite (242 cases) passes on Linux. A complete project has not yet been run in a Claude Code session on Linux. |
| macOS on Intel, Linux on ARM | Expected to work, because nothing is architecture-specific. Not tested. |
| Windows | Not supported. The scripts are bash, the hook is a shell command and paths assume `$HOME`. WSL may work; it is untested. |

### Software

- **Claude Code** with plugin support, project agents, the `agent` setting,
  `PreToolUse` hooks that report the calling agent, and project MCP servers.
  Developed against Claude Code 2.1.
- **Rust toolchain** (`cargo`), to build the `incipit` binary. Developed with
  Rust 1.97; the code uses edition 2021 and nothing recent. Not needed on a
  machine that has a prebuilt binary (see section 15).
- **bash** 3.2 or newer. The scripts avoid features newer than the bash that
  ships with macOS.
- **git**. Receipts record the commit; the setup commits before activation.
- **rsync** or **tar** for the import. `rsync` is used when present.
- Network access for the first build, to download three crates and their
  dependencies: `serde`, `serde_json`, `sha2`.

Nothing is installed system-wide. The binary goes to `~/.incipit/bin/incipit`,
build output to `~/.incipit/build`.

## 3. Installation

Install it from GitHub. The repository is its own marketplace:

```
claude plugin marketplace add tuxie01/Incipit
claude plugin install incipit@incipit
```

Or, from a local clone, either load it for one session:

```
git clone https://github.com/tuxie01/Incipit.git
claude --plugin-dir /path/to/Incipit
```

or install the clone permanently:

```
claude plugin marketplace add /path/to/Incipit
claude plugin install incipit@incipit
```

The binary is installed on first use by `/incipit:newproject`. To install or
update it by hand:

```
bash /path/to/Incipit/scripts/install-server.sh
```

The script compares the installed version with the source and does nothing if
they match. It uses a prebuilt binary from `server/dist/` if one matches the
machine (`incipit-Darwin-arm64`, `incipit-Linux-x86_64`), otherwise it builds
with `cargo`.

Optional, so that `incipit` works without the full path:

```
export PATH="$HOME/.incipit/bin:$PATH"
```

The plugin is only needed for the setup. A project that has been set up
carries its agents, hooks and configuration itself and needs only the binary.

## 4. Setting up a project

In an empty or existing directory, in Claude Code:

```
/incipit:newproject
```

The setup runs in five phases. Each is recorded in `.incipit/state.json`, so
it can stop at any point and resume with the same command.

| Phase | What happens | Done when |
|---|---|---|
| scaffold | The templates are copied. Existing files are kept. | All scaffold files exist |
| grill | Interview, writes `SPEC.md` and `.incipit/config.json` | The owner approves the spec |
| toolchain | Build, lint and test tooling; every criterion gets a check | `scripts/check.sh` runs end to end |
| team | Agent briefs and `CLAUDE.md` get the project specifics | No placeholder is left |
| activate | The setup is committed, the lead becomes the main session | The owner confirms |

`incipit phase show` checks the files, not only the state. A folder emptied
by hand while the hidden state file stayed behind is reported as
`scaffold: incomplete`.

### Importing an existing project

To restart an old, unfinished project, answer the question after the scaffold
phase with its path, or run `/incipit:import <directory>` yourself. The old
project can be anything; it does not have to be a Claude Code project.

- The old state is copied to `legacy/`, without version-control data,
  dependencies and build output, and made read-only. The source directory is
  not changed.
- Old agent instructions (`CLAUDE.md`, `.claude/`, `.mcp.json`, `AGENTS.md`)
  are renamed, so they do not become live instructions in the new project.
- The session asks whose code it is, under which licence, and whether
  `legacy/` goes into version control.
- It then reads the copy and writes `docs/legacy-survey.md`: purpose, stack,
  one row per component with its state, what is unfinished, what is worth
  keeping. It judges by reading and never claims that something works.
- The grill session starts from that survey and settles for each component:
  `adopt`, `rewrite` or `drop`.

### The grill session

One question per turn, with a recommendation where a real choice exists. The
session works through: context, goal, non-goals, existing work, acceptance
criteria, reference or oracle, constraints, repository layout, toolchain,
version control, audit policy, team, owner decisions.

Three rules matter most:

- Every acceptance criterion gets an id (`AC-1`, ...) and a note whether its
  check path is `proven` or `unproven`. Unproven is allowed; the next phase
  tries each one.
- A name, abbreviation or path the session cannot place is read or asked
  about at once, never mapped onto something similar it knows.
- Everything in the spec comes from the owner or from a file. The session
  adds no requirements of its own.

### The toolchain phase

Derives the concrete toolchain from the spec and gets the owner's approval
for the whole list, including details such as compiler flags. It writes
`docs/test-plan.md` with one row per criterion, tries every unproven check
path, and builds `scripts/check.sh`: one command that runs build, lint and
tests, prints `PASS`, `FAIL` or `SKIPPED` per stage and always reaches its
last line.

It builds the harness, not the test suite: enough to show that each kind of
check works, with a self-test that passes without the product. A criterion
that cannot be checked in this environment goes back to the owner.

### Activation

Before activation the session checks the git identity, commits the setup, and
after your confirmation runs `incipit activate` and commits that too.
Activation writes `"agent": "lead"` into `.claude/settings.json`: every new
session in the project then starts as the lead. `incipit deactivate` in a
terminal undoes it, for example to rerun the grill session.

## 5. The team

Five roles are always present.

| Role | Writes | On tickets | Starts agents | Shell |
|---|---|---|---|---|
| lead | nothing | reads; keeps the decision log | yes | no |
| architect | documentation | creates, amends; proposes spec changes | no | no |
| implementer | product source | moves its own tickets to `implemented` | no | yes |
| tester | tests, check scripts, evidence | verdict, **close**; builds tester tickets | no | yes |
| auditor | evidence | audit result | no | read-only git |

Two roles are optional. The grill session asks; `incipit role add <name>`
switches one on later.

| Role | Writes | Purpose |
|---|---|---|
| security | evidence | Reviews tickets marked `security_required` for security defects. Such a ticket cannot close without its PASS. Read-only git shell. |
| researcher | `docs/research/` | Answers one precise question from the primary source and writes the answer down with its sources. Has web search. |

The exact write paths of each role are in `.incipit/config.json` under
`roles`, as `write_allow` and `write_deny` lists of path prefixes. A deny
always wins over an allow. The briefs point to that file instead of repeating
the paths, so a change of the rules cannot leave a brief out of date.

The lead is the main session. Workers are started by the lead only, each on
exactly one ticket.

## 6. Tickets

Tickets live as JSON files in `.incipit/tasks/` and are read and changed only
through the ticket server.

### States

```
open -> in_progress -> implemented -> accepted -> closed
         ^   |  ^            |
         |   v  |            +-- builder withdraws its work: back to in_progress
         | blocked
         +-- FAIL verdict, audit or security review: back to open

any unfinished state -> withdrawn   (owner only)
```

### Who builds: the ticket role

Every ticket has a `role`. `implementer` tickets are built by the implementer
and verified by the tester. `tester` tickets are for test tools, harness work
and checks: the tester builds and verifies them, and the server forces
`audit_required`, so the builder is never the only judge.

### Tickets only grow

A ticket is never rewritten. What it asks for is fixed when it is created.
While it is open or blocked, the architect can add a clarification, a further
criterion, a new priority, or turn the audit or security requirement on.
Nothing can be removed or reworded. To ask for less, the architect creates a
new ticket with `supersedes`, and the owner withdraws the old one. Every
weakening therefore passes the owner.

### Closing

`task_close` is refused unless all of this holds:

- the ticket is `accepted`, which takes a PASS verdict of the tester,
- the verdict was recorded after the last implementation,
- for `audit_required`: a PASS audit after the last implementation,
- for `security_required`: a PASS security review after the last
  implementation,
- every evidence file is unchanged since it was recorded.

Verdicts, audits and reviews need a non-empty evidence file below
`.incipit/evidence/`. The file is hashed (SHA-256) at that moment.

### Receipts

Closing writes a receipt into the ticket:

- the git commit, and whether the working tree was clean (ticket files and
  proposals are left out of that check, because the server changes them),
- each evidence file with its hash,
- a hash over the ticket's title, scope, criteria and amendments.

`incipit ticket verify` rechecks all closed tickets against their receipts
and reports any evidence file that is missing or changed.

### Tools

| Tool | Role | Purpose |
|---|---|---|
| `task_list`, `task_get` | everyone | read |
| `task_create` | architect | create a ticket |
| `task_amend` | architect | add to an open or blocked ticket |
| `task_status` | the ticket's role | open, in progress, blocked, implemented |
| `task_comment` | all but the lead | one-line note |
| `task_verdict` | tester | PASS or FAIL with evidence |
| `task_audit` | auditor | PASS or FAIL with evidence |
| `task_security` | security | PASS or FAIL with evidence |
| `task_close` | tester | close and write the receipt |
| `spec_propose` | architect | propose a spec change |
| `decision_log` | lead | append to the decision log |
| `decision_list` | everyone | read the decision log |
| `task_commit` | all but the lead | commit own files and the ticket's state |

## 7. Spec changes

`SPEC.md` is the source of truth, and no agent can write it. When a decision
of the owner changes the spec, the architect files a proposal with
`spec_propose`: a list of exact replacements, each of which must match
exactly one passage of the current file. The server refuses a proposal that
could not be applied.

The owner looks at it and decides:

```
incipit spec show P-0001
incipit spec apply P-0001      # or: incipit spec reject P-0001 "reason"
git diff SPEC.md
```

A proposal is applied completely or not at all. If the spec changed since the
proposal was filed and a passage no longer matches, nothing is applied.

## 8. The decision log

The lead remembers nothing between sessions and writes no files. What the
owner decided and what is still open would otherwise live only in a chat that
is gone after a restart.

The lead therefore keeps a log with `decision_log`: decisions (in the owner's
words), questions put to the owner, answers, and notes for the next session.
Entries are only appended. A question is closed by an answer that refers to
it. At the start of every session the lead reads the log and picks up from
the open questions.

```
incipit decisions          # open questions and the latest entries
incipit decisions open
incipit decisions all
```

## 9. How agents hand work over

Agents talk in two fixed blocks. Details stay in the ticket and in evidence
files; the blocks carry ids, paths and verdicts. This keeps the traffic
between agents small.

Lead to worker:

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

Worker to lead:

```
STATUS
ticket: T-0007
role: implementer
verdict: DONE | PASS | FAIL | BLOCKED
state: <ticket state now>
evidence: <path, or "-">
note: <one line, or "-">
```

The worker reads the ticket itself; ticket and spec text are never pasted
into a block. The lead reads an evidence file only on `FAIL` or `BLOCKED`,
and confirms the ticket state with `task_get` before the next step.

## 10. How the rules are enforced

Three layers, each independent of the others.

1. **Tool lists.** Each brief in `.claude/agents/` lists exactly the tools of
   its role. `task_close` is in the tester's list only. The lead's list has
   no tool that writes a file or runs a command.
2. **Guard hook.** A `PreToolUse` hook runs `incipit guard` before every
   write, shell command and ticket tool call. It identifies the calling agent
   and checks the call against the role: ticket tools against a fixed table,
   file writes against `write_allow` and `write_deny`. Tickets, proposals and
   the decision log can be written by the server only, for everyone including
   the owner session. The team's own configuration (`.claude/`, `.mcp.json`,
   the control files in `.incipit/`) is closed to every role. If the binary
   is missing, the hook refuses the call instead of letting it through.
3. **Ticket server.** Enforces the ticket rules no matter who calls: the
   state machine, the ticket role, the evidence requirement, the hashes.

**Permission prompts.** Claude Code asks the owner before an agent uses a
tool of the ticket server for the first time. An agent running in the
background cannot show that prompt, so its call is refused; this has happened
to `task_close`. Incipit does not pre-approve its own tools. The owner
decides: approve the tool when asked, or add
`"permissions": {"allow": ["mcp__incipit"]}` to `.claude/settings.json`. The
entry skips the prompt only. The guard still checks every call against the
role.

**Read-only git.** Auditor, security reviewer and researcher judge by
reading, but "unchanged since" and "which commits belong to this ticket" can
only be answered from git. Their shell therefore runs exactly one read-only
git command per call (`log`, `show`, `diff`, `ls-files`, `blame` and a few
more) and nothing else: no pipes, redirections, variables or chaining, no
writing options such as `--output`, no `git -c`. This check is exact, because
a command without shell operators is one program with its arguments.

**git itself.** No role may change git configuration (`git config`,
`git -c`) or write below `.git/`, because git configuration decides which
programs git runs, for example hooks. Roles with a full shell cannot change
the index, branches or history either (`add`, `commit`, `checkout`, `reset`,
`stash`, `push` and similar); commits go through `task_commit`.

What is exact and what is not:

- Writes through the Edit and Write tools are checked exactly. Paths are
  resolved including symlinks and compared without regard to letter case, so
  `.Incipit/Tasks` or a link into the tickets directory does not get through.
- Shell commands are checked by matching the command text. That stops direct
  references to protected paths and the use of the `incipit` CLI by a role.
  It does not stop a deliberately disguised command. Roles with a shell
  (implementer, tester) are therefore held by their briefs and by what the
  auditor reads, not by the guard alone.

## 11. Owner commands

Run in a terminal in the project directory. `incipit` finds the project from
the current directory.

```
incipit status                         what is due, and which steps are yours
incipit ticket list [status]
incipit ticket show <id>
incipit ticket withdraw <id> <reason>  take a ticket out of the queue
incipit ticket role <id> implementer|tester
incipit ticket verify                  check closed tickets against receipts
incipit spec list | show <id> | apply <id> | reject <id> [reason]
incipit decisions [open|all]
incipit role add security|researcher
incipit phase [show] | done <name> | reset <name>
incipit activate | deactivate
incipit upgrade
incipit version
```

`incipit status` is the one to remember. It lists tickets, pending proposals,
open questions and the git state, and ends with the steps that are yours,
each with the command to run.

Roles cannot use the CLI: the guard refuses any shell command of an agent
that calls it.

## 12. Files in a project

```
SPEC.md                      what is built and how "done" is checked
CLAUDE.md                    short orientation for every agent
.mcp.json                    starts the ticket server
.claude/settings.json        guard hook, the lead as main agent
.claude/agents/              one brief per role
.incipit/config.json         commands, layout, role rules, policies
.incipit/protocol.md         handover blocks, states, who may do what
.incipit/state.json          setup phases
.incipit/tasks/              tickets (server only)
.incipit/proposals/          spec proposals (server only)
.incipit/decisions.json      decision log (server only)
.incipit/evidence/           evidence files of tester, auditor, security
.incipit/import.txt          origin of an import, if any
docs/test-plan.md            which check proves which criterion
docs/legacy-survey.md        survey of an imported project, if any
docs/research/               notes of the researcher, if any
scripts/check.sh             the one check command
legacy/                      read-only copy of an imported project, if any
```

Everything except `legacy/` (if you chose to ignore it) and build output is
meant to be committed, including the tickets, the decision log and the
evidence. They are the record of how the project was built.

## 13. Working with the team day to day

Start Claude Code in the project directory with a plain `claude`. The session
is the lead. A good first order:

```
Derive the first tickets from SPEC.md and docs/test-plan.md. Foundation
only, at most ten. Then stop and show me the list.
```

Review the list, then let one ticket run through completely before you let
the lead work through several. The usual run of a ticket:

1. The lead hands it to the implementer, who builds, commits and sets it to
   `implemented`.
2. The tester checks every criterion itself, writes the evidence, commits,
   records the verdict and closes.
3. If an audit or security review is required, the tester leaves the ticket
   `accepted`, the lead sends the auditor or the reviewer, then the tester
   again to close.
4. A `FAIL` sends the ticket back to `open` with the evidence file named.

The lead brings to you: anything on the list of owner decisions, scope
changes, gaps or contradictions in the spec, a third `FAIL` on the same
ticket, and every request to weaken a check. Some steps are yours alone, and
the lead gives you the command: withdrawing a ticket, applying a spec
proposal.

Every worker commits its own work with `task_commit`. The server takes only
the files that belong to the calling role, by the write rules (the role with
the longest matching `write_allow` prefix owns a file), plus the ticket file
and the evidence of the named ticket. Files of other roles stay uncommitted,
so workers can run side by side and a ticket's commit holds only what belongs
to it. Roles cannot run `git add`, `git commit` or other commands that change
the index, a branch or the history.

`/incipit:feedback` asks a session where the plugin itself got in the way.

## 14. Upgrading a project

A project carries copies of the briefs and the protocol. After updating the
plugin, quit Claude Code in the project and run:

```
bash /path/to/Incipit/scripts/install-server.sh
cd /path/to/project
incipit upgrade
git add -A && git commit -m "Upgrade Incipit"
```

`incipit upgrade` replaces the tools list in each brief, appends a section
with the rules that changed, and replaces `.incipit/protocol.md`. Text that
was written for the project stays. The command can be run again safely.

## 15. Limits and open points

- **Ticket backend:** files in the repository only. GitHub Issues is planned;
  the interface for further backends exists.
- **Prebuilt binaries:** not published yet, so every machine builds with
  `cargo`. The release workflow `.github/workflows/release.yml` builds them when a
  version tag is pushed; `install-server.sh` does not download them yet.
- **Shell guard:** text matching, as described in section 10.
- **Platforms:** used on macOS on Apple Silicon. Linux is covered by the
  build and the test suite, not by a full project run. Windows is not
  supported.
- **One lead at a time:** two Claude Code sessions in the same project are
  not coordinated with each other beyond the file lock of the ticket store.
- **Project types:** one generic template. Templates for specific kinds of
  project (Rust, embedded, retro) are planned as further directories below
  `templates/`, selected with `INCIPIT_TEMPLATE=<name>`.
- **Evidence without running:** auditor, security reviewer and researcher
  read code and git history but build and run nothing. What can only be
  confirmed by running the product is reported as such and left to the
  tester.

## 16. Developing Incipit

```
.claude-plugin/              plugin and marketplace manifest
skills/                      newproject, import, grill, toolchain, team, feedback
templates/base/              what scaffold copies into a project
templates/optional-agents/   briefs of the optional roles
scripts/                     install-server.sh, scaffold.sh, import.sh
server/                      the incipit binary (Rust)
.github/workflows/           release workflow: builds the binaries on a version tag
```

The binary is one program with three faces: the ticket server (MCP over
stdio, hand-written JSON-RPC), the guard, and the owner CLI. The briefs of
the roles and the protocol are compiled into it, which is how
`incipit upgrade` and `incipit role add` work without the plugin folder.

```
cargo test --release --manifest-path server/Cargo.toml
cargo build --release --manifest-path server/Cargo.toml
bash server/tests/smoke.sh server/target/release/incipit
claude plugin validate .
```

`server/tests/smoke.sh` drives the built binary from outside: the ticket
lifecycle over MCP, every guard decision, the owner commands and the upgrade
of an older project.

When a rule changes, add it to the template briefs and as a dated section in
`server/src/owner.rs` (`update_note`), so that existing projects receive it
through `incipit upgrade`.

## License

Copyright (C) 2026 Ingo Uhlemann

Incipit is free software: you can redistribute it and/or modify it under the
terms of the GNU General Public License, version 3, as published by the Free
Software Foundation. It is distributed in the hope that it will be useful,
but without any warranty; without even the implied warranty of
merchantability or fitness for a particular purpose. See [LICENSE](LICENSE)
for the full text.
