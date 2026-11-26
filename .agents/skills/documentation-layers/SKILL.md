---
name: documentation-layers
description: Rules for where to put and how to write documentation and comments in this repository. Use when deciding what belongs in README, CONTRIBUTING, AGENTS.md (root or per directory), docs/architecture.md, ADRs, file header comments (`//!`), rustdoc (`///`), line comments (`//`), or TODO/FIXME/HACK; when adding or removing comments; and when updating documentation along with a code change.
---

# documentation-layers

Documentation is split into four layers by how fine-grained the described subject is, plus three kinds of documents that sit outside the layers. The layers follow the Context, Container, Component, and Code levels of the C4 model.

## Layers

| Layer     | Subject                                                    | Location                                                                                   |
| --------- | ---------------------------------------------------------- | ------------------------------------------------------------------------------------------ |
| Context   | The system and the outside world; domain terms             | `README.md` (for users), the Context section of `docs/architecture.md`                     |
| Container | The applications and data stores that make up the product and the development and test environment, and the communication between them | The Container section of `docs/architecture.md` |
| Component | Responsibilities and relations of modules and types | Spanning directories: the Component section of `docs/architecture.md`. Within a directory: comments |
| Code      | The behavior itself                                        | Code, tests, CI workflows, compose files, `Cargo.toml`                                     |

The Code layer holds code only. Code is the most direct documentation of behavior. The other layers must not contradict the code, and must not repeat what the code already says.

Comments do not belong to the Code layer. A comment is a description from an upper layer, placed at the relevant spot in the code. The subject a comment describes decides its layer. Most comments belong to the Component layer. Comments that describe a user's flow of operations, such as `// Scenario:` and `// Given` in E2E tests, belong to the Context layer.

## Documents outside the layers

| Kind       | Content                                                     | Location                                           |
| ---------- | ----------------------------------------------------------- | -------------------------------------------------- |
| History    | The situation at the time of a decision, the decision, and the alternatives considered | `docs/adrs/`                         |
| Rules      | How to write code, tests, commits, and documentation        | `AGENTS.md` (root and per directory), this skill   |
| Procedures | Setting up the development environment; running tests and seeds | `CONTRIBUTING.md`                              |

ADRs sit outside not because they contain reasons, but because they are records fixed at the time of a decision. Write the current conclusion and a short reason in the layers, so that the current design is understandable without reading ADRs. Do not rewrite existing ADRs until the user asks.

## Deciding where to put information

For each piece of information, decide in this order.

1. Can code express it? If naming, types, function decomposition, or tests can express it, change the code.
2. Is it a structural fact or a rule for how to write? Decide with this question:
   - If some code contradicted the statement, would a reader of the code be misled? If so, it is a structural fact; write it in one of the layers (e.g. an Issue allowing only one upload at a time, or getters starting with `get_` panicking when the value is absent).
   - If the code still reads correctly but the project would suffer, it is a rule; write it in AGENTS.md (e.g. the three-way split of a Component, writing expected values as literals, how to split commits).
   - If one sentence mixes both, split it. For the `try_` prefix, the meaning (it returns `Option`) is a fact, and how to name things is a rule.
3. If it is a fact, choose the layer by how fine-grained its subject is.
4. If you want to keep how a decision was reached or which alternatives were rejected, add a new ADR.

### Boundary rules

- `docs/architecture.md` holds only the Context, the Containers, and the Component structure that spans directories. Do not create a per-directory architecture document.
- Within a directory, write structure that is hard to read from the code in the `//!` of the module that owns it. This covers invariants spread over several files and not enforced in one place, and conventions that have no shared type.
- Do not copy into documents structure the code already shows. This includes state diagrams that can be drawn from an enum, lists of types or getters, and processing order that one function makes clear.
- For a Rust directory, write its responsibility in one line in the `//!` of the directory's module file (such as `mod.rs`). For non-Rust directories (`datas/`, `docker/`, `.github/`, `docs/`), write their responsibility in the Container section of `docs/architecture.md`.
- AGENTS.md holds only "how to write". Write rules that derive from the structure as references to architecture.md or comments, and do not copy the facts.
- Write rules that apply only to one directory in that directory's `AGENTS.md`, and put a `CLAUDE.md` symlink pointing to `AGENTS.md` next to it.
- Keep the body of each rule in AGENTS.md or this skill. CONTRIBUTING holds only procedures and references.
- Define terms in the Context section of `docs/architecture.md`. Comments may use those terms without explaining them.
- The HTML in `docs/` is written from the markdown of the same name. Follow the rules in AGENTS.md for how to write it.

### Where each kind of information goes

Each row holds only information with a reason the code cannot show it. When the reason no longer applies, delete the statement.

| Information | Why the code cannot show it | Location |
| --- | --- | --- |
| Purpose and status, features and non-goals, launching, the config file, operation, supported environments, troubleshooting | Users do not read the code | README |
| System context diagram, key points of contracts with external systems, glossary | The behavior of external systems is not in the code. Terms do not map one-to-one to identifiers | Context section of architecture |
| Product Containers and the communication between them, platform differences, external resources touched at runtime | They span several build configurations and resources outside the process | Container section of architecture |
| Development and test environment (xtask, local Redmine, how E2E works, CI), fixture dependencies | They span several tools and non-Rust files | Container section of architecture |
| Layers and dependency direction, data flow, classification of branches that look like failures | Assembling them requires reading the dependencies and calls of every directory | Component section of architecture |
| Responsibility of a directory or file | A name alone does not fix the scope | `//!` (one line) |
| Invariants spread over several files, conventions with no shared type | Reading one place does not reveal the whole constraint | `//!` of the owning module |
| Contracts that callers depend on | Types cannot express them | `///` |
| Reasons for implementation choices, local constraints, known compromises | Code does not show the rejected alternatives or the reasons | `//`, TODO/FIXME/HACK |
| How to write code and tests within a directory | Code that breaks them still reads correctly | The directory's AGENTS.md |
| Repository-wide rules for tests, naming, comments, snapshots, fixtures, commits and PRs, branches | Same as above | Root AGENTS.md, this skill |
| Setting up the development environment, running tests | Procedures for running things live outside the code | CONTRIBUTING |

## Writing comments

The purpose of a comment is to tell the reader the writer's intent beyond what the code says, and to shorten the time needed to understand the code. Writing a comment is not always right. Reading a comment also costs time, so prefer improving the code to convey intent.

### What the reader knows

Assume the reader of a comment knows the layer documents (`README.md`, `docs/architecture.md`) but not the documents outside the layers (such as ADRs). References to ADRs or issues may be used as supplements, but a comment must not depend on reading the reference to get its point. Summarize the needed reason or constraint in the comment itself.

### Order of judgment

1. Check whether the code and the layer documents already convey the target's responsibility, behavior, and intent.
2. Consider whether better naming, types, function decomposition, or API design can convey the intent without a comment.
3. Comment only on information the reader cannot get from the code, or that would cost the reader unnecessary effort to work out.
4. Check that the comment does not restate the Code layer, does not contradict the code, and is understandable without reading the documents outside the layers.

Do not write what is obvious from naming and syntax. Being a public API is not by itself a reason to write rustdoc. Do not compensate for a bad name with a comment; fix the name.

```rust
// bad: restates the Code layer.
// Get the selected issue ID.
let issue_id = self.selected_issue_id;

// bad: compensates for a vague name.
// Keep only synced journals.
let filtered = journals.filter(Journal::is_synced);

// good: the name alone tells what the filter produces.
let synced_journals = journals.filter(Journal::is_synced);
```

### What to comment on

- Reasons for a choice: why this implementation, and why it differs from the approach that looks natural.
- Assumptions and constraints: which inputs or states are expected, and which invariants are relied on.
- Usage contracts: call order, boundary values, errors, panics, side effects.
- Responsibilities and relations: what a file or type covers, and how it relates to neighboring modules.
- Caveats and limits: easily overlooked costs (external calls, waiting time, computational complexity), easy misuse, known incompleteness, current compromises and their reasons.

Where simplifying the code would make it stop behaving as specified, write "why it does this", not "what it does". A good comment states the Component property that would be lost if the branch were removed.

```rust
// bad: restates the condition.
// Do not overwrite if the journal is being edited.
if journal.is_editing() {
    return;
}

// good: states the property lost if the branch were removed.
// Keep journals under editing so that refetched notes do not discard unsent local edits.
if journal.is_editing() {
    return;
}
```

### What not to write

- Do not write the caller's circumstances (timing constraints such as "not passed while the editor is open") in the callee's doc. Put such constraints on the caller side.
- Do not write examples of a specific Usecase in comments on code that many Usecases or Components pass through (such as the runner). Write the reason with a concrete subject and action.

### Kinds and format

- `//!` states the file's or module's responsibility and its neighbors, plus the invariants and conventions described in the boundary rules. Do not spread what fits in one line over several lines, and do not copy the content of `docs/architecture.md`.
- `///` states contracts that callers and users depend on. Do not restate the arguments and return value.
- `//` states the design decisions and local reasons that maintainers of the internals need.
- `TODO` marks work not yet started, `FIXME` a known defect, and `HACK` an intended compromise. Write each so that a future reader can judge its condition and reason from the code alone.

```rust
/// Saves the edit and makes it a target of the next sync.
///
/// Returns `SaveError::IssueNotFound` if the issue does not exist.
fn save_edit(&mut self, issue_id: IssueId) -> Result<(), SaveError> {
    // Do not convert to the remote format here, so the edit stays editable until the sync completes.
    // The conversion happens when the upload starts.
    // ...
}
```

### Accurate and concise

- Keep the information per character high. Short is fine, but do not narrow the meaning. A mapping can be written briefly in type-annotation style, such as `IssueId -> (state, diff)`.
- When a pronoun could refer to more than one noun, rewrite it with the noun.
- State the behavior on empty input, boundary values, duplicates, and errors, which names and summaries tend to leave ambiguous.
- When conditions are complex, show input and output examples instead of piling up prose. Choose examples that are hard to judge, such as empty input, boundary values, or duplicates, not the typical normal case.
- For `bool` or numeric literal arguments whose meaning the caller cannot see, consider a named-argument comment such as `/* use_cache = */ true`. Prefer splitting into a dedicated function or type when possible.
- Imagine where a first-time reader would ask "why", and which points are easy to get wrong.
- When you change code, check the nearby comments too, and fix or delete the ones that are out of date.
