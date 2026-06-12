# ADR 0002 — TypeScript structural extraction & the LanguageExtractor boundary

Status: accepted
Date: 2026-06-12

## Context
Second language (TypeScript), added to prove the `LanguageExtractor` boundary
generalizes beyond Go. Structure + imports landed first (containment/import edges
are `resolved`); a later additive pass added the heuristic call graph (`calls`
edges are `heuristic`). See ADR 0001 for the Go mapping and shared decisions.

## Boundary outcome (the point of this stage)
TypeScript was expressed **without changing the `LanguageExtractor` trait or the
core-ir IR types.** Two language-shape differences were absorbed additively:

1. **Lexical methods.** TS methods nest inside their class, so `Class`→`Method`
   containment is known at parse time. To let `extract_file` emit that itself
   (instead of a Go-style receiver pass in the linker), `FileExtraction` gained an
   `edges: Vec<Edge>` field for intra-file resolved edges. Go leaves it empty.
   The linker treats any node that is a `contains` target in `edges` as
   *already parented* and skips re-parenting it. This is a payload extension, not
   a trait/IR change (the same way `calls` was added in the previous stage).
2. **File-based imports.** Go imports target packages (`File`→`Module`); TS
   imports target files (`File`→`File`). The linker now resolves imports per the
   producing file's language tag.

Conclusion: the boundary held. No STOP was required.

## Decision — TypeScript → IR NodeKind
| TS construct                  | NodeKind   | qualifiedName | notes |
|-------------------------------|------------|---------------|-------|
| `.ts` / `.tsx` file           | File       | ""            | tsx uses the TSX grammar |
| directory                     | Module     | "" ; `name` = dir's final component (root = `.`) |
| `function foo()`              | Function   | `foo`         | incl. `export function` |
| `class C {}`                  | Class      | `C`           | incl. `export class` |
| method in a class             | Method     | `C.method`    | `Class`→`Method` emitted from `extract_file` |
| `interface I {}`              | Interface  | `I`           | |
| `type T = ...`                | TypeAlias  | `T`           | |

Language tag on nodes/ids is `ts` (e.g. `ts:models/user.ts#User.greet`).

## Decision — imports (`resolved`, `File`→`File`)
Relative specifiers (`./x`, `../x`) resolve against the importing file's
directory, trying `<p>.ts`, `<p>.tsx`, then `<p>/index.ts`; a hit emits
`File`→`File`. Bare / `node_modules` specifiers are skipped (like Go stdlib).

## Decision — heuristic call graph (`confidence = heuristic`)
Mirrors the Go calls stage: no type checker, so **every** `calls` edge is
`heuristic`. `extract_file` collects raw, unresolved `CallRef`s per body
(recursing into nested calls, so `console.log(u.greet())` yields the inner
`u.greet()`); `extract_repo` resolves them against the full node set:
- **Bare `foo(...)`** — (1a) a Function named `foo` in the **same file**; else
  (1b) an imported symbol `foo`, resolved through the file's *named* import
  bindings (`local → imported name + specifier`) to the exported Function in the
  target file. This is TS's analog of Go's package-qualified call.
- **`obj.method(...)`** — no types, so match `method` against `Method` nodes by
  name, preferring the caller's module; multiple matches **fan out** (one edge
  each, never silently picked).
- **Unresolvable / external / builtins** (`console.log`, `node_modules`, no
  matching repo node) — skipped. No `External` nodes.

Repeated source→target calls merge into one edge with multiple sorted sites.
Call-graph deferrals: `new X()` constructors (they parse as `new_expression`,
not calls, so naturally excluded), dynamic/computed calls, namespace-import
member calls (`ns.foo()`), default-import and re-export bindings.

## Decision — `go.mod` is now optional
`extract_repo` no longer requires `go.mod`; it's only read for Go import
resolution. A TypeScript-only repo (no `go.mod`) extracts normally. Behavior with
a `go.mod` present is unchanged, so the Go golden is byte-identical.

## Known gaps (deferred — NOT extracted this stage)
- arrow-function consts (`const f = () => {}`), object-method shorthand
- enums, namespaces/modules, decorators
- `abstract class`, computed method names, getters/setters as distinct kinds
- `.d.ts` declaration files (still parsed if walked; nothing special done)
- `export … from "x"` re-exports and `import type` are not collected as imports
- tsconfig `paths`/baseUrl aliases (only literal relative specifiers resolve)
- interface method signatures (only the `Interface` node is emitted)

## Module-name caveat
TS has no package clause, so a directory's `Module.name` is its final path
component; the repo-root directory is named `.`. (Go names modules by package
clause — see ADR 0001.) The `FileExtraction.package_name` field is reused as the
"module display name" for both languages.
