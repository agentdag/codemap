# IR Specification (Phase 0)
schemaVersion: 0.1.0

## Purpose
A language-neutral model of a codebase. Extractors produce it; the viewer,
overlay, and codegen all consume it. This is the single source of truth.

## Model
A directed, typed property graph.
- Nodes = things in code (files, modules, symbols).
- Edges = relationships (containment, imports, calls).
- `contains` edges form a strict TREE (every node has exactly one parent except
  the root). `imports` and `calls` are the cross-cutting GRAPH on top.

## Identity  (the most important decision)
IDs are READABLE LOGICAL STRINGS, not hashes:
- File:   `file:<relpath>`
- Module: `module:<relpath-or-package>`
- Symbol: `<lang>:<relpath>#<qualifiedName>`
    e.g. `ts:src/users/user.service.ts#UserService.create`
- Edge:   `<sourceId>=[<kind>]=><targetId>`

Why readable, not hashed: golden-test diffs stay human-reviewable — that is your
safety net for catching AI mistakes. (A hashed index form can be added later for
storage; the logical id stays canonical.)

Stability: an id is stable while path + qualifiedName are unchanged. Moves/renames
CHANGE the id; the IR does not track identity across moves — the later overlay
reconciliation step re-binds. Do not try to make ids survive moves here.

Edge cases (handle explicitly):
- Anonymous fns/closures: qualifiedName = `<enclosing>.<#index>` by source order.
  Known instability point — documented, accepted for Phase 0.
- Overloads / same-name methods: append `#<index>` by source order.

## Node
{ id, kind, name, qualifiedName, language,
  location { file, range { startLine, startCol, endLine, endCol } },
  contentHash, metadata: map<string, json> }
- name: short display name.  contentHash: blake3 of the node's source slice
  (file contents for File nodes); used later for incremental change detection.

### NodeKind (Phase 0; extensible)
File, Module, Function, Method, Class, Interface, TypeAlias, Variable
(reserved for later: Endpoint, Middleware, Controller, Entity, External)

## Edge
{ id, source, target, kind, confidence, sites: CallSite[], metadata }
- CallSite { file, range }

### EdgeKind (Phase 0; extensible)
contains, imports, calls
(reserved: routesTo, emits, consumes, dependsOn, references)

### Confidence
- resolved  — syntactic/semantic fact (containment, imports; later LSP-resolved calls)
- heuristic — inferred by name/scope matching (Phase-0 calls, pre-LSP)
- asserted  — human-authored (from the overlay; never produced by extractors)
CARDINAL RULE: a `calls` edge from Phase-0 tree-sitter is `heuristic`.
Never label a guess `resolved`.

## Document
{ schemaVersion, root { repoPath, analyzedAt (ISO8601), toolVersion },
  nodes[], edges[] }

## Serialization (determinism is mandatory)
JSON. nodes sorted by id; edges sorted by id; map keys sorted.
Same input -> byte-identical output. Shuffled input -> identical output.
Golden tests enforce this.

## Out of scope for Phase 0
LSP resolution, service detection, overlay, codegen, domain node kinds.
Enums are designed to extend without breaking existing ids.