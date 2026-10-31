# The root option is `--root`

Filed by: implementer (shifting-loop-runtime 02)

## Spec says
`spec.md` §11: "Global options: the repository root (default: the current directory); `--max-iterations`". Ticket 02: "the repository root (default: the current directory, resolved through git)".

## Assumed
The option is `--root <dir>`. shift asks git for the top of the work tree containing that directory, so a subdirectory names its repository. Both `--root` and `--max-iterations` may come before or after the subcommand. A subcommand is required until the shift run arrives.

## Proposed spec text
§11: "`--root <dir>` (default `.`) names a directory inside the repository; shift resolves it to the work tree's top through git. Global options may come before or after the subcommand."
