# Every site of a duplicated shape uses the shared one

Filed by: implementer (shifting-loop-runtime 02)

## Spec says
`code-review` skill baseline: "**Duplicated Code**: the same logic shape appears in more than one hunk or file in the change. → extract the shared shape, call it from both."

## Assumed
The smell covers every site with the shape, not only the sites a finding names. When one version of the shape already exists, every site with it uses that version. In the shift loop, the Script Bridge already tells a script run's success (exit `0` with valid stdout) from its failure. Every script whose contract only distinguishes success from failure reads its run that way: precheck, verify, judge, commit and rollback. So success means the same for all of them. Select and work give exit `2` and exit `3` meanings of their own, so they have a different shape and read the exit status directly.

## Proposed spec text
`code-review` skill baseline, Duplicated Code: "Every site with the shape counts, not only the ones a finding names. Any design that leaves one version of the shape fixes it, for example:
- each site calls the version that already exists, such as the one split of a script run into success and failure;
- the shape is extracted into a new function that every site calls."
