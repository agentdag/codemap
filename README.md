# codemap

> A local-first tool that builds an editable architecture map of any codebase.

**Status: early / work in progress.** The core engine is being built stage by stage.

## What it does

codemap analyzes a codebase and turns it into a navigable graph — files, modules,
types, functions, and the relationships between them. The long-term goal is a tool that:

- **Extracts** a language-agnostic graph of your code automatically (tree-sitter today,
  language-server resolution later).
- **Detects** candidate service/module boundaries.
- **Lets you curate** a semantic overlay on top — group code into services, label nodes,
  draw relationships — that stays bound to the code as the code changes.

The extracted graph is always ground truth, regenerated from code. Human curation lives
in a separate, version-controlled overlay. The two are composed for the view, never mixed.

## Current capabilities

- A stable intermediate representation (IR) for code graphs, with deterministic
  serialization (`crates/core-ir`).
- A structural extractor for **Go** — files, packages, structs, interfaces, methods,
  functions, and intra-module imports (`crates/extract`).
- A CLI to run it (`crates/cli`).

## Build & run

```bash
cargo build
cargo run -p cli -- analyze ./fixtures/go-sample
```

This prints the extracted IR as canonical JSON.

## Architecture

- **Rust engine** (`crates/`) — extraction, the IR, and (later) graph analysis,
  the overlay, and codegen.
- **Two-layer model** — extracted graph (ground truth) + semantic overlay (human-authored).
- Design docs live in [`docs/specs/`](docs/specs); decisions in `docs/adr/`.

## Roadmap

- [x] IR + deterministic serialization
- [x] Go structural extractor + CLI
- [ ] Call graph (heuristic edges)
- [ ] Second language (TypeScript) to prove the extractor boundary
- [ ] Language-server resolution for precise call edges
- [ ] Interactive graph viewer
- [ ] Semantic overlay (grouping, pins, labels) with anchoring/reconciliation
- [ ] Service-boundary detection
- [ ] Framework-aware scaffolding

## CI 

- Two things to add later, not now:

> OS matrix — when the Tauri app arrives and cross-platform matters, swap runs-on: ubuntu-latest for runs-on: `${{ matrix.os }}` with `strategy.matrix.os: [ubuntu-latest, macos-latest, windows-latest]`. 

> A JS job — once apps/viewer exists, add a second job for pnpm install + typecheck + build. No point until the viewer is in.

## License

See [LICENSE](LICENSE).