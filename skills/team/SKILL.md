---
name: team
description: Team phase of an Incipit project. Fills the agent briefs and CLAUDE.md with the project specifics from SPEC.md and .incipit/config.json and checks the role rules. Use when the user runs /incipit:team or when the newproject skill reaches the team phase.
---

# Team

The scaffold copied five generic agent briefs to `.claude/agents/` and a
`CLAUDE.md` skeleton. If the grill session added optional roles (`security`,
`researcher`), their briefs are there too; treat them like the others. This phase makes them specific to the project and
checks that the role rules fit the project as it now is.

Read `SPEC.md`, `.incipit/config.json` and `docs/test-plan.md` first.

## 1. Check the role rules

`.incipit/config.json` is the only place where write paths are defined; the
guard reads it on every tool call. Go through `roles` against the actual
repository:

- Every path that holds checks (the check script, lint scripts, the test
  plan, tests) is writable by the tester and by no role that is checked by
  it. In particular the implementer cannot write any of them.
- Every such path has an owner. A path that no role can write is a finding:
  later changes would all land on the owner.
- The implementer can write everything it needs to build the product, and
  nothing in `legacy/`.
- No role can write `SPEC.md`.
- Every role in `optional_roles` has a brief in `.claude/agents/` and write
  rules in `roles`. If one is missing, run
  `"$HOME/.incipit/bin/incipit" role add <name>`. If `security` is listed,
  `security_policy` is not empty.

Report what you found to the owner as a short table (role, may write, may
not write) and propose any change. Tightening a rule or assigning an unowned
path needs the owner's yes; loosening one does too. Apply approved changes to
the config.

## 2. Fill the briefs

Each file contains `{{PLACEHOLDER}}` markers. Replace every one:

| Marker | Value |
|---|---|
| `{{PROJECT_NAME}}` | `project.name` |
| `{{PROJECT_GOAL}}` | `project.goal`, one sentence |
| `{{LAYOUT}}` | source, tests and docs paths, one line each |
| `{{CHECK_COMMAND}}` | `commands.check` |
| `{{AUDIT_POLICY}}` | `audit_policy` |
| `{{OWNER_DECISIONS}}` | `owner_decisions`, as a short list |
| `{{PROJECT_NOTES}}` | at most five lines this role needs and cannot find faster in the spec: the reference or oracle, a hard constraint, a known trap. Write "None." if there is nothing. |

## Rules

- Do not change the frontmatter of the agent files. The `tools` lists are the
  permission model; the guard hook enforces the same rules.
- Do not write path lists into the briefs. The briefs point to the config, so
  a later change of the rules cannot leave a brief out of date.
- Do not copy the spec into the briefs. A brief points to `SPEC.md` sections;
  it does not repeat them. Every line in a brief is loaded on every agent
  start, so keep them short.
- Do not invent roles. The optional roles `security` and `researcher` are
  added with `incipit role add <name>` after the owner's yes; anything else
  the project seems to need goes to the owner as a recommendation.

## Finish

Run `grep -rn '{{' CLAUDE.md .claude/agents/` and confirm it finds nothing.
Then run `"$HOME/.incipit/bin/incipit" phase done team`.
