# Commit and rollback are each paired with their error in one place

Filed by: implementer (shifting-loop-runtime 02)

## Spec says
`.scratch/shifting-loop-runtime/assumptions/44-commit-and-rollback-routing-cannot-be-miswired.md`: "one step per state type, with each of commit and rollback paired once with its failure error;"

## Assumed
"Paired once" means each of commit and rollback is paired with the error its failure raises in exactly one place. The step for each state that ends the decision goes through that pairing, so the step only picks commit or rollback. It never names an error. The shared run of a settling script, which takes that error as an argument, stays inside the loop's own code. A step that skipped the pairing would have to write a new pairing by hand. It could not pick the wrong one by accident.

Binding the error into the commit and rollback values when the loop is composed would also qualify. Here, though, it would store a function pointer in values that can be compared for equality, and the lint gate rejects comparing function pointers. So the loop keeps one pairing function each for commit and rollback.

## Proposed spec text
§6: "Each of commit and rollback is paired with the error its failure raises in exactly one place, and every step that ends the decision goes through it, so a step picks only commit or rollback. Designs that satisfy this include:
- one function each for commit and rollback that binds its error;
- the error bound into the commit and rollback values when the loop is composed;
- an exhaustive match over a closed set of settlements."
