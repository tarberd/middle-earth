# A derived value is computed totally

Filed by: orchestrator (shifting-loop-runtime 02)

## Spec says
`docs/agents/assumptions/20-derived-values-are-not-getters.md`, example: "`pub const fn iterations(&self) -> u64 { self.commits + self.already_done + self.declines }`".

`docs/agents/assumptions/23-arithmetic-overflow-is-a-failure-path.md`: "Production integer arithmetic that can overflow is a failure path".

## Assumed
Assumption 20's interpretation stands, but its example adds `u64` counts unchecked, which assumption 23 rules out. A method that derives a value from public fields is total like any other code. Either of these designs satisfies that:
- a result type wide enough that the computation can't overflow, e.g. six `u64` counts summed as `u128`;
- checked arithmetic with a typed error.

## Proposed spec text
`CODING_STANDARDS.md` §4: "A method that computes a value from public fields isn't a getter; prefer it to storing a derived copy. It is total like any other code, for example through a result type wide enough to hold any result, or checked arithmetic."
