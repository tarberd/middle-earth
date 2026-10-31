# A path whose parent is a file is not missing

Filed by: implementer (shifting-loop-runtime 02)

## Spec says
`.scratch/shifting-loop-runtime/assumptions/29-an-uninspectable-loop-path-is-an-error.md`: "A loop directory or script counts as missing only when the file system reports that it doesn't exist, which includes a dangling symbolic link."

## Assumed
When part of a loop path is a file instead of a directory, for example a `.shifting` that is a file, the file system reports "not a directory", not that the path doesn't exist. The check stops with the typed error naming the path and that cause, before any script runs.

## Proposed spec text
§3: "A path counts as missing only when the file system reports that it doesn't exist. Any other report, \"not a directory\" included, is shown with the path."
