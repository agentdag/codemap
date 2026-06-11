# ADR 0001 — Go structural extraction mapping (Phase 0)

Status: accepted
Date: 2026-06-10

## Context
First structural extractor. Scope is **structure + imports only** — no calls, no
LSP, no heuristic name matching. Every edge produced here is `resolved`.

## Decision — Go → IR NodeKind
| Go construct                         | NodeKind   | qualifiedName                         |
|--------------------------------------|------------|---------------------------------------|
| `.go` file                           | File       | "" (empty, per spec)                  |
| directory containing `.go` files     | Module     | "" ; `name` = package name            |
| `func Foo(...)` (no receiver)         | Function   | `Foo`                                 |
| `func (r *T) M(...)`                   | Method     | `T.M` (leading `*` stripped)          |
| `type X struct {...}`                  | Class      | `X`                                   |
| `type X interface {...}`               | Interface  | `X`                                   |
| `type X = Y` / `type X Y` (defined)    | TypeAlias  | `X`                                   |

Package-level `var`/`const` are skipped this stage.

## Decision — edges (all `confidence = resolved`)
- `contains` Module→File, Module→Module (each package dir links to its nearest
  ancestor package dir), File→{Function,Class,Interface,TypeAlias}.
- Method containment: a Method links to the `Class` whose `qualifiedName` equals
  the receiver base type **in the same package (directory)** → `contains`
  Class→Method. If no such Class exists (e.g. a method on a defined non-struct
  type or interface satisfaction), the Method attaches to its File and the
  fallback edge carries `metadata.unresolvedReceiver` = the receiver type. The
  method node's location always stays its own lexical site.
- `imports` File→Module: an import path equal to the go.mod module path, or
  prefixed by `<modulePath>/`, maps to the corresponding repo directory; if that
  directory is a known package, emit File→Module. Stdlib/external imports are
  skipped.

## Decision — under-specified fields (the spec does not cover directories)
These are documented choices, not derived from the spec:
- **Directory relpath**: forward-slash, relative to the repo root; the root
  package directory is `"."`. So module ids look like `module:.`, `module:greet`.
- **Module `contentHash`**: empty string — a directory has no single source
  slice. (`File`/symbol hashes use blake3 over their bytes as the spec requires.)
- **Module `location`**: `file` = the directory relpath, `range` = all zeros.
- **`root.analyzedAt`**: real current UTC time (RFC3339) in normal runs. The
  golden test normalizes it to `1970-01-01T00:00:00Z` before comparing, so
  determinism lives in the test, not the extractor. (Superseded the earlier
  hardcoded-epoch approach.) `root.toolVersion` stays the constant `0.1.0`.
- **`root.repoPath`**: a stable, machine-independent identifier — the final
  component of the canonicalized root (e.g. `../../fixtures/go-sample` →
  `go-sample`) — so the caller's relative/absolute path doesn't leak into the IR.

## Decision — positions
0-based `(line, column)`; column is a byte offset within the line (tree-sitter's
native `Point`).

## Deviation from the task's literal trait signature
`LanguageExtractor::extract_file` returns `Result<FileExtraction, ExtractError>`
rather than a bare `FileExtraction`. Parsing is fallible (parser setup / parse
failure) and the constraints forbid `unwrap()` and require surfacing errors as
`Result`s, so the fallible signature is required to satisfy both. No behavior
changes otherwise.

## Single-tree caveat
`contains` is a strict tree only when the repo root directory is itself a package
(it is the tree root, `module:.`). Repos whose root has no `.go` files produce a
forest of top-level package modules; reconciling that (a synthetic repo root) is
out of scope for this stage.
