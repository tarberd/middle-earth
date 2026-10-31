# A layer that adds nothing to the call is a Middle Man

Filed by: implementer (shifting-loop-runtime 02)

## Spec says
`code-review` skill baseline: "**Middle Man**: a class or function that mostly just delegates onward. → cut it, call the real target direct."

`code-review` skill baseline: "**Message Chains**: long `a.b().c().d()` navigation the caller shouldn't depend on. → hide the walk behind one method on the first object."

## Assumed
A function delegates onward when all it does is pass its arguments, or values that other functions of its type build from them, to one more function, and its callers already call those functions themselves elsewhere. It is an extra path to calls that already exist, so it is cut, and its callers build the value and call the target directly. In the shift loop, running a script for an attempt with some input was such a layer: the attempt's invocation and the run that names the script were already called directly when select and commit or rollback run.

A layer stays when it adds something to the call. Examples are the task reference as a line on stdin, the stderr sink, or the error naming the script. A method that only hides a walk through its own fields, which its callers would otherwise follow themselves, also stays, since it is the Message Chains fix.

## Proposed spec text
`code-review` skill baseline, Middle Man: "Every layer in a call chain adds something to the call, such as data the target receives or handling of its result, or hides a walk through its own fields from its callers. Any design that removes a layer adding nothing satisfies this, for example:
- callers build the values and call the target directly;
- the layer is merged into its target."
