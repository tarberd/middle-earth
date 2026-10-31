# The task reference arrives as a line

Filed by: implementer (shifting-loop-runtime 02)

## Spec says
`spec.md` §4: "`precheck.sh` | task reference" and "`work.sh` | task reference", and "`0` selected (trimmed stdout becomes `SHIFT_TASK`)".

## Assumed
Precheck and work receive the task reference followed by a newline on stdin, like `echo "$SHIFT_TASK"`, so that `read -r task` succeeds. `SHIFT_TASK` holds the trimmed reference without a newline.

## Proposed spec text
§4: "`precheck.sh` and `work.sh` receive the task reference as one line on stdin."
