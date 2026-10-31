# A test is trusted once it has been seen failing

Filed by: orchestrator (shifting-loop-runtime 02)

## Spec says
`docs/agents/assumptions/19-a-type-state-composition-goes-green-whole.md`, proposed `tdd` text: "When types force a whole composition into one slice, write its first seam tests, make them green together, and check each later seam test by breaking the route it covers."

`tdd` skill: "**Red before green.** Write the failing test first, then only enough code to pass it."

## Assumed
Assumption 19's interpretation stands, but its proposed text names one procedure. The property red-before-green protects is that every test has been seen to fail for the behaviour it covers before it is trusted. Any way of seeing it fail satisfies that, for example:
- writing the test before the code;
- when types force a whole composition into one slice, breaking the route the test covers and watching it fail.

## Proposed spec text
`tdd` skill, Rules of the loop: "Every test is seen failing for the behaviour it covers before it is trusted. Write it before the code when you can; when types force a whole composition into one slice, break the route it covers and watch it fail instead."
