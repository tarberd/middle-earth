# Issue tracker: Local Markdown

Issues and specs for this repo live as markdown files in `.scratch/`.

## Conventions

- One feature per directory: `.scratch/<feature-slug>/`
- The spec is `.scratch/<feature-slug>/spec.md`
- Implementation issues are one file per ticket at `.scratch/<feature-slug>/issues/<NN>-<slug>.md`, numbered from `01`, never a single combined tickets file
- Triage state is recorded as a `Status:` line near the top of each issue file (see `triage-labels.md` for the role strings)
- Comments and conversation history append to the bottom of the file under a `## Comments` heading

## Assumptions

An **assumption** records an interpretation an agent chose where a document it works from is silent, ambiguous or contradicts itself. It mirrors shifting's own assumptions (ADR-0012). Record every one, whatever its reach or lifetime: a gap an implementer fills, a review finding the orchestrator overrules, a fix the orchestrator directs that no document spells out, a process choice no doc dictates.

- **Where**, by the document the assumption interprets:
  - the ticket or its feature spec → `.scratch/<feature-slug>/assumptions/<NN>-<slug>.md`;
  - a repo-wide document (`CODING_STANDARDS.md`, an ADR, `GLOSSARY.md`, `AGENTS.md`, `docs/agents/*`, a skill) → `docs/agents/assumptions/<NN>-<slug>.md`.
- **One assumption per file**, never a combined file. `NN` has at least two digits and is one more than the highest number present in that directory.
- **Format**:

  ```markdown
  # <title>

  Filed by: <role> (<feature-slug> <ticket NN>)

  ## Spec says
  "Nothing", or one or two quoted passages, each naming its document.

  ## Assumed

  ## Proposed spec text
  ```

  `<role>` is the filing agent's job, e.g. `implementer` or `orchestrator`. For a repo-wide document, `Proposed spec text` is the text proposed for that document.
- **Describe behaviour** in `Assumed` and `Proposed spec text`, using the document's own vocabulary, so the record stays true as code moves. A short code pattern may illustrate a standards assumption; files, functions, tests and commits stay out of the record.
- **Provisional spec**: every agent reads both directories when its context starts and follows every pending assumption. A finding an assumption already settles is settled.
- **Write once**: an agent never edits or deletes an assumption. An agent that disagrees with one files a new assumption citing the old one by path.
- **Review** belongs to the human, at any time: one assumption, several, or mid-ticket. Accept (fold its proposed text into the document), correct (write the answer into the document) or dismiss, then delete the file. The next fresh context sees the result.
- **Commit** an assumption with the work that relies on it, or on its own when nothing relies on it yet.

## When a skill says "publish to the issue tracker"

Create a new file under `.scratch/<feature-slug>/` (creating the directory if needed).

## When a skill says "fetch the relevant ticket"

Read the file at the referenced path. The user will normally pass the path or the issue number directly.

## Wayfinding operations

Used by `/wayfinder`. The **map** is a file with one **child** file per ticket.

- **Map**: `.scratch/<effort>/map.md` (the Notes / Decisions-so-far / Fog body).
- **Child ticket**: `.scratch/<effort>/issues/NN-<slug>.md`, numbered from `01`, with the question in the body. A `Type:` line records the ticket type (`research`/`prototype`/`grilling`/`task`); a `Status:` line records `claimed`/`resolved`.
- **Blocking**: a `Blocked by: NN, NN` line near the top. A ticket is unblocked when every file it lists is `resolved`.
- **Frontier**: scan `.scratch/<effort>/issues/` for files that are open, unblocked, and unclaimed; first by number wins.
- **Claim**: set `Status: claimed` and save before any work.
- **Resolve**: append the answer under an `## Answer` heading, set `Status: resolved`, then append a context pointer (gist + link) to the map's Decisions-so-far in `map.md`.
