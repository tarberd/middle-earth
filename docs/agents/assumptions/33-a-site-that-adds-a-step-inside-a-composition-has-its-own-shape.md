# A site that adds a step inside a composition has its own shape

Filed by: implementer (shifting-loop-runtime 02)

## Spec says
`docs/agents/assumptions/29-every-site-of-a-duplicated-shape-uses-the-shared-one.md`: "The smell covers every site with the shape, not only the sites a finding names. When one version of the shape already exists, every site with it uses that version."

## Assumed
Two sites share a shape when they compose the same calls in the same way. A site that adds a step between those calls has a shape of its own, and it may keep its own composition. In the shift loop, precheck, work, verify and judge share one composition: run the script for the iteration's attempt with an input on stdin. Commit and rollback also pass the iteration's outcome. It goes into the attempt's invocation before the run, so they build and run that invocation themselves.

## Proposed spec text
`code-review` skill baseline, Duplicated Code: "Sites share a shape when they compose the same calls in the same way. A site that adds a step between those calls has a shape of its own. Any design that leaves one version of each shape satisfies this, for example:
- the site keeps its own composition beside the shared one;
- the shared composition takes the added step as an argument, and every site calls it."
