---
name: grill
description: Grill session for an Incipit project. Interviews the owner one question at a time and writes SPEC.md and .incipit/config.json. Use when the user runs /incipit:grill or when the newproject or import skill reaches the grill phase.
---

# Grill session

Your job is to turn a project idea into a spec that a team of agents can build
and test without asking what was meant. You do that by questioning the owner
until nothing important is left to guess.

## How to ask

- **One question per turn.** Then wait for the answer.
- A question asks for one decision. Several decisions of the same kind (keep
  or drop each of these functions, confirm these criteria) may go out as one
  list to approve, up to about ten items, with your recommendation per item.
  Anything on the list that needs real thought gets its own question instead.
- Keep questions short. Where it helps, offer two or three concrete options
  and say which one you recommend and why, in one sentence. If the owner
  answers outside your options twice in a row, your options are missing the
  point: ask the next questions openly and listen for what the owner is
  optimising for.
- Challenge vague answers. "Fast", "simple", "secure" are not answers: ask for
  the number, the case or the threat.
- Name contradictions as soon as you see them and ask which side wins.
- **Resolve every unknown at once.** If an answer contains a name, an
  abbreviation or a path you cannot place with certainty, read it (if it is a
  path) or ask what it is, in your very next turn. Never map an unfamiliar
  name onto a similar one you know. A wrong guess here spreads through every
  later answer.
- Do not ask what you can read. If the directory already holds code or
  documents, read them first and ask only about what they leave open. If
  `docs/legacy-survey.md` exists, an old project was imported to `legacy/`:
  read the survey before your first question, and start from its open
  questions instead of from zero.
- When the owner points to a neighbouring project, read what the question
  needs and no more: search with limited output and cut long lines, because
  one status file can flood your context.
- Ask in the owner's language. Write files in English.

## Topics

Work through these in order. Skip a topic only when it clearly does not apply,
and say so.

1. **Context.** What this is part of: the larger project, platform or
   ecosystem, the neighbouring projects it must work with, and where they are
   on disk. Ask this first; it decides how to read everything after.
2. **Goal.** What exists at the end, for whom, and what it replaces.
3. **Non-goals.** What is deliberately out of scope.
4. **Existing work.** Only if `legacy/` exists. Go through the survey's
   components and settle for each one: `adopt` (carry the code over, then it
   must pass the same checks as new code), `rewrite` (build anew, the old
   code is reference) or `drop`. Settle which decisions and findings from the
   survey stay valid. If the licence of the old code is open, say so in the
   spec and make "adopt before the licence is settled" an owner decision.
5. **Acceptance criteria.** How anyone can tell the goal is reached. Each
   criterion must be checkable by a command, a measurement or an inspection
   of a named artifact. Number them `AC-1`, `AC-2`, ... For each one, record
   whether the check path is `proven` (you know a concrete way to run this
   check in this environment) or `unproven` (it should be checkable, but
   nobody has shown how yet). Do not guess `proven`. Unproven is fine at this
   stage: the toolchain phase tries each one and brings back what fails.
6. **Reference and oracle.** Is there a specification, an existing system or a
   reference implementation that defines correct behaviour? Pin its version.
7. **Constraints.** Language, platforms, target hardware, dependencies that are
   required or forbidden, licence, security requirements.
8. **Repository layout.** Where source, tests and documentation live. This
   becomes the write paths of the roles.
9. **Toolchain.** Build, lint and test commands, as far as the owner already
   knows them. The toolchain phase fills the gaps.
10. **Version control.** What stays out of the repository (reference copies,
    build output, third-party binaries), and anything the owner wants about
    branches or commits.
11. **Audit policy.** Which tickets need an auditor PASS before they can be
    closed: all, none, or a rule such as "everything touching security or the
    public API".
12. **Team.** Five roles are always there: lead, architect, implementer,
    tester, auditor. Two more are optional; recommend each only if the
    project gives it real work, because every role costs time and tokens.
    - **Security reviewer:** reads tickets for security defects before they
      close. Worth it when the product handles untrusted input, files or
      paths from outside, network data, credentials or privileges. If the
      owner wants it, settle the security policy: which tickets need the
      review, as one sentence the architect can apply.
    - **Researcher:** establishes what a reference, standard or existing
      system actually says before work is built on it. Worth it when topic 6
      named a reference or oracle that the team will keep consulting.
13. **Owner decisions.** Which decisions the team must always bring to the
    owner (scope changes, new dependencies, anything else the owner names).

Not a question: the ticket backend. Only `files` is implemented, so set it and
mention it in the final summary. The same goes for anything where you would
propose the default and expect a plain yes: put it in the summary instead of
spending a turn on it.

## What you write

Write down each settled answer immediately, so an interrupted session loses
nothing. On resume, read `SPEC.md` and `.incipit/config.json` first and
continue with the first open topic.

**`SPEC.md`**: fill the sections of the existing skeleton.

- Facts and decisions, no narrative.
- Everything in the spec comes from the owner or from a file you read. Do not
  add requirements of your own, however sensible. If you think one is
  missing, ask.
- Things that came up and are not settled yet go into section 11, "Open
  points", one line each. Remove a line when it is settled. This is the one
  place for loose ends; do not scatter "Open:" notes through the sections.
- Section 10 lists each legacy component with its decision (`adopt`,
  `rewrite`, `drop`) and its path under `legacy/`.
- Leave no placeholder behind; a section that does not apply says "Not
  applicable" with the reason.

**`.incipit/config.json`**: set
- `project.name` and `project.goal` (one sentence),
- `audit_policy` (the owner's rule, one sentence),
- `security_policy` (one sentence), if the owner chose the security reviewer,
- `owner_decisions` (list),
- `layout.source`, `layout.tests`, `layout.docs`,
- `roles.<role>.write_allow` and `write_deny` to match the layout:
  the implementer writes source but never tests or check scripts, the tester
  writes tests, check scripts and `.incipit/evidence/`, the architect writes
  documentation, the auditor writes `.incipit/evidence/` only, the lead writes
  nothing. With an import, `legacy/` is in the implementer's `write_deny`.

Change only these keys and keep the file valid JSON.

For each optional role the owner chose, run
`"$HOME/.incipit/bin/incipit" role add security` or `... role add researcher`
once. The command adds the role's brief, its write rules and the entry in
`optional_roles`; do not edit those by hand. Record the choice in section 8
of the spec.

## Finish

When every topic is settled and section 11 is empty, or holds only points the
owner has explicitly deferred, show the owner a summary: context, goal,
non-goals, the acceptance criteria with their check path marked proven or
unproven, the defaults you set without asking, the team with its optional
roles, audit and security policy. Ask for
approval. Only after an explicit yes, run
`"$HOME/.incipit/bin/incipit" phase done grill`. If the owner changes
something, update the files and ask again.
