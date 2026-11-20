# {{PROJECT_NAME}}

{{PROJECT_GOAL}}

## Where things are

- `SPEC.md`: what is built and how "done" is checked. The source of truth.
- `docs/test-plan.md`: which check proves which acceptance criterion.
- `.incipit/protocol.md`: how the team hands work over, and the ticket states.
- `.incipit/config.json`: commands, layout and the write paths of each role.
- `legacy/` and `docs/legacy-survey.md`, if present: an older project imported
  as a read-only reference. Take from it only what `SPEC.md` section 10 says
  to adopt; adopted code must pass the same checks as new code.

{{LAYOUT}}

## Rules for everyone

- Work happens on tickets. Tickets are read and changed through the `task_*`
  tools only, never as files.
- Check command: `{{CHECK_COMMAND}}`
- No stubs or placeholders that pretend to work. Missing things are reported,
  not faked.
- Files, code and commit messages are in English. Reports to the owner are in
  the owner's language.
- The owner decides scope, spec changes and everything listed as an owner
  decision in the lead's brief. Agents recommend.
