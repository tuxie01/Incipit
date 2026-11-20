---
name: newproject
description: Set up a new Incipit project in five resumable phases (scaffold, grill, toolchain, team, activate). Use only when the user runs /incipit:newproject.
disable-model-invocation: true
argument-hint: "[target-directory]"
---

# Incipit: new project

You set up a project skeleton in five phases. Each phase is recorded in
`.incipit/state.json`, so the setup can stop and resume at any point.

Argument given to this command: "$ARGUMENTS". If the quotes are empty, the
target directory is the current directory; otherwise it is that path.

Talk to the owner in their language. Write every project file in English.

## Rules

- One phase at a time, in order. Never skip a phase and never start one whose
  predecessor is not done.
- At the start, run `"$HOME/.incipit/bin/incipit" phase show` from the target
  directory and continue with the phase named after `next:`.
- `phase show` checks the files, not only the state: `scaffold: incomplete`
  means files are missing although the phase was marked done. Run the scaffold
  script again in that case; it keeps every existing file.
- A phase is marked done only when its own completion check passed. Mark it
  with `"$HOME/.incipit/bin/incipit" phase done <name>`. The command prints
  the new state, so there is no need to call `phase show` after it.
- You make recommendations; the owner decides. Do not settle open design
  questions yourself.
- If the owner stops, report which phase is next and how to resume
  (`/incipit:newproject` again).

The plugin's scripts are in `${CLAUDE_PLUGIN_ROOT}/scripts`. If that variable
is empty in your shell, the plugin root is the directory two levels above
this file.

## Phase 0: install the binary

Run `bash "${CLAUDE_PLUGIN_ROOT}/scripts/install-server.sh"`. It installs
`~/.incipit/bin/incipit` (ticket server, guard and CLI) or updates an older
version. If it fails, show the error and stop: nothing else works without the
binary.

## Phase 1: scaffold

Run `bash "${CLAUDE_PLUGIN_ROOT}/scripts/scaffold.sh" "<target-directory>"`.
The script copies the templates without overwriting existing files and marks
the phase done itself. Report the list of created files in one short block.

## Optional: import an existing project

After the scaffold phase and before the grill phase, ask the owner once: is
there an existing project to load as starting material, and if so, at which
path? Ask for the path in the same question. If the owner names one, invoke
the `incipit:import` skill with the path; it copies the old state to
`legacy/`, surveys it and starts the grill session itself. If not, continue
with phase 2. Skip the question when `legacy/` already exists.

## Phase 2: grill

Invoke the `incipit:grill` skill. It interviews the owner and writes `SPEC.md`
and `.incipit/config.json`. Done when the owner has approved the spec.

## Phase 3: toolchain

Invoke the `incipit:toolchain` skill. It derives build, lint and test tooling
from the spec, proves the check paths and maps every acceptance criterion to
a check. Done when `scripts/check.sh` runs end to end and the owner has
confirmed any criterion that came back.

## Phase 4: team

Invoke the `incipit:team` skill. It fills the agent briefs and `CLAUDE.md`
with the project specifics and checks the role rules. Done when no
`{{PLACEHOLDER}}` is left.

## Phase 5: activate

Activation makes the lead the main session agent. From the next session on,
the main session can read and delegate only, so everything that needs a
writing session happens here, before the restart.

1. **Summarise** for the owner: project goal in one sentence, ticket backend,
   check command, and the write paths of each role from
   `.incipit/config.json`. Name every path that no role may write.
2. **Check git.** Run `git config user.name` and `git config user.email`. If
   either is empty, ask the owner for the identity to use before committing;
   do not let git guess one.
3. **Commit the setup.** Show `git status --short` and propose a commit of the
   setup on the current branch. Check that nothing ignored by decision
   (`legacy/`, build output) is in it. Commit after the owner's yes.
4. **Ask the owner to confirm activation.**
5. On confirmation run `"$HOME/.incipit/bin/incipit" activate`, then
   `"$HOME/.incipit/bin/incipit" phase done activate`. Both change tracked
   files (`.claude/settings.json`, `.incipit/state.json`): commit them now as
   a second commit, "Activate the lead". The lead cannot commit later.
6. **Tell the owner** to restart Claude Code in the project and give the lead
   its first order, for example "derive the first tickets from SPEC.md". Say
   plainly that the lead's limits and the guard hook take effect with the new
   session, not in this one, and that `incipit deactivate` in a terminal
   returns to a plain owner session.
