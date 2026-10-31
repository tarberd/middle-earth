# Tests assert on behaviour, never on recorded calls

Filed by: orchestrator (shifting-loop-runtime 02)

## Spec says
`docs/agents/assumptions/21-logging-stub-scripts-are-not-spies.md`, proposed text: "Tests observe behaviour at a real boundary, never through in-process doubles that record the calls made on them."

`CODING_STANDARDS.md` §1: "hypervisor interactions sit behind the `Hypervisor` boundary port (implemented by `VirshHypervisor` in production and an in-memory `FakeHypervisor` in tests) … avoiding mockable traits and interaction-recording spies."

## Assumed
Assumption 21's interpretation stands: logging stub scripts are allowed. Its proposed text, though, would also rule out the in-memory fakes §1 endorses. The property the rule protects is that tests assert on observable behaviour, never on calls an in-process double recorded. What crosses a process boundary is observable behaviour. Each of these satisfies the property:
- an in-memory fake behind a boundary port;
- a stub executable on the far side of a process boundary that logs what it receives.

## Proposed spec text
`CODING_STANDARDS.md` §1: "Tests assert on observable behaviour, never on calls an in-process double recorded. An in-memory fake behind a boundary port qualifies, and so does a stub executable that logs what it receives across a process boundary."
