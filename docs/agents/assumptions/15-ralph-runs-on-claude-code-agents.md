# Ralph runs on Claude Code agents

Filed by: orchestrator (shifting-loop-runtime 01)

## Spec says
`ralph` skill: "Antigravity provides automatic reactive wakeup: after dispatching a subagent via `invoke_subagent`, stop calling tools immediately and end the turn." and "The orchestrator configures subagent model tiers passed to `invoke_subagent` (`Model: 'inherit' | 'pro' | 'flash' | 'flash_lite'`)".

## Assumed
In Claude Code:
- `invoke_subagent` is the Agent tool, starting a new general-purpose agent for every dispatch;
- `send_message` is SendMessage, which is never used;
- `inherit` means passing no model;
- reactive wakeup is the Agent tool's completion notification;
- parallel critics are two Agent calls in one message.

## Proposed spec text
`ralph` skill: a note mapping each Antigravity tool to its Claude Code equivalent, as above.
