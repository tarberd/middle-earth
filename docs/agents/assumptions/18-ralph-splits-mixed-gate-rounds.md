# Ralph splits mixed gate rounds

Filed by: orchestrator (shifting-loop-runtime 01)

## Spec says
`ralph` skill, step C: "**If findings exist and the Orchestrator agrees** … Loop back to **Step A**" and "**If findings exist but the Orchestrator determines they are invalid or false positives** … Overrule the critic, record the ruling as an assumption, and proceed to Step 3." It doesn't say what happens when a round has both.

## Assumed
When a round has both kinds, the orchestrator overrules the invalid findings, records each as an assumption, and loops back to step A with only the accepted ones. It proceeds to step 3 only when no accepted finding remains.

## Proposed spec text
`ralph` skill, step C: "Split the findings: overrule the invalid ones and loop back with the accepted ones. Proceed to Step 3 once no accepted finding remains."
