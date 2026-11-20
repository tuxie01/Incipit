---
name: toolchain
description: Toolchain phase of an Incipit project. Derives build, lint and test tooling from SPEC.md, proves the check paths and maps every acceptance criterion to a check. Use when the user runs /incipit:toolchain or when the newproject skill reaches the toolchain phase.
---

# Toolchain and tests

The spec says what "done" means. This phase builds the means to check it, so
that the tester later proves acceptance with commands instead of opinions.

Read `SPEC.md` and `.incipit/config.json` first. If the grill phase is not
done, stop and say so.

This phase runs before the team exists, so you write the first version of the
check scripts, the test plan and the harness yourself. After activation they
belong to the tester.

## 1. Settle the toolchain

From the spec's constraints, work out the concrete toolchain: language
version and standard, compiler and linker settings that matter, build tool,
linter and formatter, test framework, and any emulator, simulator or
measuring tool the acceptance criteria need.

Present all of it to the owner as one list with your recommendation per item,
and get approval before you build on it. That includes the choices that look
like details (a language standard, a stack size, leaving a tool out): the
owner approves the list, you do not decide parts of it silently. Where a real
alternative exists, ask about it in its own question. Do not install anything
system-wide without asking.

When you take tooling from a neighbouring project, read the parts you need,
not whole scripts.

## 2. Map criteria to checks

Write `docs/test-plan.md` with one row per acceptance criterion:

| AC | Check | Kind | Tool or command | Status | Evidence |
|---|---|---|---|---|---|

- **Kind** is `automated`, `measured` or `inspected`.
- **Status** is `ready` (the check exists and runs), `planned` (the check
  path is proven, the test case is still to be written) or `NOT PROVEN`
  (nobody has shown yet that this can be checked here).
- Every `AC-n` from the spec appears.
- Where a criterion needs a tool that does not exist yet (a harness, a
  comparison against a reference, a measurement script), list the tool.

## 3. Prove the check paths

The spec marks each criterion's check path as proven or unproven. Take the
unproven ones and try each: can this environment actually observe what the
criterion asks for? A small experiment is enough; it does not have to be the
final test.

- It works: mark the row `planned` or `ready`.
- It does not work, or you could not find out: mark the row `NOT PROVEN`, say
  what you tried, and take the criterion back to the owner at the end of the
  phase. The owner changes the criterion, accepts an inspection instead, or
  keeps it as an open risk. Update `SPEC.md` with the decision.

Never write a check that asserts something you have not seen the environment
report. A check that cannot fail is worse than no check.

## 4. Build the check entry point

Create `scripts/check.sh`: one command that runs the stages build, lint and
automated tests (plus any project-specific stage) in that order.

- Every stage prints exactly one summary line: `PASS`, `FAIL` or `SKIPPED`
  with a short reason.
- A stage whose prerequisite failed is `SKIPPED`, not silently left out. The
  script always reaches its last line.
- The script exits non-zero if any stage failed or was skipped.

This is the command the tester runs for every ticket. Make every script you
create executable (`chmod +x`).

## 5. Build the harness, not the test suite

Build what is needed to run checks at all: the project and test skeleton, the
harness, and one working check per kind of check the plan uses, so each
mechanism is shown to work. Give the harness a self-test that passes without
the product. Stop there: the remaining test cases and tools stay in the test
plan as `planned`, and the tester writes them ticket by ticket.

No fake results: do not write tests that pass without testing anything, and do
not stub a tool so that it reports success. A check for something not built
yet may fail; it must fail for the right reason.

Record where third-party parts of the harness come from (source, version or
commit, licence) in a file next to them.

## 6. Record it

In `.incipit/config.json` set `commands.build`, `commands.lint`,
`commands.test` and `commands.check` (normally `scripts/check.sh`). If the
layout changed, update `layout.*` and the roles' `write_allow` and
`write_deny` to match. Every check script and lint script you created must be
writable by the tester and closed to the implementer: add each such path to
the tester's `write_allow` and the implementer's `write_deny`. Change only
these keys.

## Finish

Run `scripts/check.sh`. The phase is done when:

- the script runs to its last line and every stage reports;
- every `FAIL` and `SKIPPED` is a real, explained one (typically: nothing to
  build yet);
- the owner has approved the toolchain list and decided every `NOT PROVEN`
  criterion.

Report to the owner: the result per stage, what was built and tried, what is
`planned`, and the pins and third-party parts that need confirmation. Then
run `"$HOME/.incipit/bin/incipit" phase done toolchain`.
