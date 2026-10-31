# Compiler-checked matches are not Repeated Switches

Filed by: orchestrator (shifting-loop-runtime 02)

## Spec says
`code-review` skill baseline: "**Repeated Switches**: the same `switch`/`if`-cascade on the same type recurs across the change. → replace with polymorphism, or one map both sites share."

## Assumed
The smell's harm is a site that a new case slips past. A match over a closed enum without a wildcard arm, or a destructure that names every field, fails to compile when a variant or field is added, so no site can miss it. Several such sites, each with its own purpose (naming an outcome, counting it, printing every count), are left as they are. A shared map would add a list of cases the compiler doesn't check. A site that lists every field or variant without being checked, such as an array of fields, either becomes checked or counts as the smell. Either design satisfies the property, for example:
- an exhaustive match or destructure at each site;
- one shared map that every site uses.

## Proposed spec text
`code-review` skill baseline, Repeated Switches: "Sites the compiler checks for every case, such as exhaustive matches or destructures, don't count: a new case can't be missed at any of them."
