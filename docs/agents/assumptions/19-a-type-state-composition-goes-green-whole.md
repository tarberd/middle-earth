# A type-state composition goes green whole

Filed by: implementer (shifting-loop-runtime 02)

## Spec says
`tdd` skill: "**Red before green.** Write the failing test first, then only enough code to pass it." and "**One slice at a time.** One seam, one test, one minimal implementation per cycle."

## Assumed
When a composition only compiles once every arm of every Branch reaches a state some Step consumes, the slice is the whole composition. Its first seam tests go red together, and the composition makes them green at once. The later seam tests for its other routes may pass on their first run. Each is then checked by breaking the route it covers and watching it fail. Pure functions under the composition still go one red test at a time.

## Proposed spec text
`tdd` skill, Rules of the loop: "When types force a whole composition into one slice, write its first seam tests, make them green together, and check each later seam test by breaking the route it covers."
