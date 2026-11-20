---
name: feedback
description: Collect feedback on how Incipit worked in this session, to improve the plugin. Use only when the user runs /incipit:feedback.
disable-model-invocation: true
---

# Feedback on Incipit

The owner wants to know where the plugin itself got in the way, so that it
can be fixed. Report only what actually happened in this session. Separate
what you observed from what you suspect: mark each point `O` (observed) or
`S` (suspected). No praise, findings only. Answer in the owner's language.

## What to cover

For each phase or activity that ran in this session (scaffold, import, grill,
toolchain, team, activation, work as lead or worker):

1. Where was a skill or brief unclear, incomplete or contradictory? Quote the
   sentence.
2. Where did you have to guess or deviate from the instruction, and why?
3. Which steps cost time or tokens without need?
4. What was missing that you added yourself?

Then:

- Errors or friction with scripts, the guard hook, the ticket tools or the
  `incipit` CLI, with the exact message.
- Rules that blocked legitimate work, and rules that could be bypassed.
- Questions to the owner that were unnecessary, and questions that should
  have been asked earlier.
- Things the owner had to do by hand that the plugin could have done.
- The three changes to the plugin with the largest benefit, in order.

## Where it goes

Give the report in the chat. If this session can write files (it is not the
lead), also save it as `docs/feedback/incipit-<YYYY-MM-DD>.md` and say so. If
this session is the lead, log one `note` in the decision log saying that
feedback was given and on which date; the owner copies the report from the
chat.
