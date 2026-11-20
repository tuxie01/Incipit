---
name: import
description: Import an existing, unfinished project into an Incipit project as a read-only reference under legacy/, survey it, then start the grill session. Use when the user runs /incipit:import, or when the newproject skill hands over a path of an existing project to load.
argument-hint: "<source-directory>"
---

# Import an existing project

The owner is starting over and wants the old state at hand. The old project is
copied to `legacy/` as a reference: nobody builds on it directly, and the team
takes from it only what the spec later says to adopt. The old project can be
anything: it does not need to have been a Claude Code project.

Argument given to this command: "$ARGUMENTS". That is the source directory;
if the quotes are empty, ask the owner for the path. Talk to the owner in
their language; write files in English.

The plugin's scripts are in `${CLAUDE_PLUGIN_ROOT}/scripts`. If that variable
is empty in your shell, the plugin root is the directory two levels above
this file.

## Preconditions

Run `"$HOME/.incipit/bin/incipit" phase show` in the project.

- `scaffold` is pending or incomplete: stop and point to `/incipit:newproject`.
- `grill` is already done: the spec was written without the old project. Tell
  the owner and ask whether to reset with
  `"$HOME/.incipit/bin/incipit" phase reset grill` so the grill session runs
  again with the import. Do not reset on your own.

## 1. Copy

Run `bash "${CLAUDE_PLUGIN_ROOT}/scripts/import.sh" "<source-directory>"`.

The script copies everything except version-control data, dependencies and
build output, renames old agent instructions (`CLAUDE.md`, `.claude/`,
`.mcp.json`, `AGENTS.md`) if there are any, so they do not become live
instructions here, makes the copy read-only and records the origin in
`.incipit/import.txt`. The source directory itself is not changed. Report the
script's summary line.

## 2. Licence and version control

Before anything is committed, settle with the owner, in one question:

- Whose code is in `legacy/`, and under which licence? Look for licence files
  and headers first and say what you found.
- Should `legacy/` be versioned with the new project, or stay local only?

Recommend keeping it out of version control (add `legacy/` to `.gitignore`)
whenever the licence is unknown or the code is not the owner's. Apply the
owner's decision now. Say one thing clearly: ignoring `legacy/` covers the
reference copy only. Code that is later adopted into the source tree is
committed like any other code, so an open licence question blocks adopting,
not just committing.

If the copy contains files that look like credentials (`.env`, key files,
tokens in configuration), name them to the owner as well.

## 3. Survey

Read the copy and write `docs/legacy-survey.md`. Its job is to save the grill
session, and later the team, from rediscovering the old project. Keep it to
what someone deciding "adopt, rewrite or drop" needs.

Read economically. Start from the file listing, build files and
documentation; then use signatures and targeted searches. Do not print whole
source files to judge their state: a component's state usually shows in its
function list, its TODO markers and what calls it. Limit search output, and
cut long lines (`cut -c1-300`), because one generated or minified line can
flood your context. Quote search patterns such as `'*.c'` so the shell does
not expand them.

- **Purpose:** what the project was meant to do, in two or three sentences,
  and where that is stated (README, notes, code).
- **Context:** which larger project, platform or ecosystem it belongs to, if
  the files say so. Do not infer this from where the folder was located. If
  the files do not say, put it under open questions.
- **Stack:** languages, build system, main dependencies with their versions,
  target platform.
- **Licence:** what the files state, and what the owner said in step 2.
- **Components:** one table row per part.

  | Component | Path | What it does | State | Basis | Notes |
  |---|---|---|---|---|---|

  State is `complete`, `partial`, `stub`, `broken` or `unknown`. Basis is
  `read` (you saw it in the files, give the path) or `derived` (you concluded
  it). You judge by reading: nothing in `legacy/` is built or run, so never
  claim that something works. `complete` means the code is there and looks
  finished, not that it was verified.
- **Tests:** what exists and what it covers.
- **Decisions and knowledge worth keeping:** design notes, hard-won findings,
  measured values, hardware or protocol details, documented dead ends.
- **Unfinished and known problems:** TODO and FIXME markers, half-done
  branches of the code, issues the notes mention.
- **Risks for a restart:** outdated or abandoned dependencies, missing parts
  the old code relied on, licence questions.
- **Not read in depth:** which parts you only skimmed.
- **Open questions for the owner:** what the files do not answer. State them
  as questions, not as guesses.

## 4. Hand over to the grill session

Give the owner a summary of at most ten lines: purpose, how much is there, the
state of the main components, the most important open questions. Then invoke
the `incipit:grill` skill. It reads the survey first and asks only what the
survey leaves open.
