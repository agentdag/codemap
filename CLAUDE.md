# codemap — project context for Claude Code

## What this is
A local-first desktop app that builds an editable architecture map of any codebase.
- Extracts a ground-truth code graph (tree-sitter + LSP) — deterministic, never AI-guessed.
- Detects candidate service boundaries (graph clustering + structural signals).
- Lets a human curate a semantic overlay (groups, pins, labels, manual edges) stored in
  `.codemap/overlay.json`, anchored to stable node ids.
- Later: framework-aware scaffolding; an AI/RAG explanation layer on top.

## Architecture (do not deviate without an ADR in docs/adr/)
- TWO LAYERS: (1) extracted graph = ground truth, regenerated from code, never hand-edited;
  (2) semantic overlay = human-authored, persisted separately, anchored to stable ids.
- Rust core engine in `crates/`; TS/React frontend in `apps/desktop` (Tauri); IR types
  are generated from Rust into TS.
- The IR (`crates/core-ir`) is the single source of truth every component reads/writes.

## Cardinal rules
- Never emit a confident wrong edge. Edges carry a confidence: resolved vs heuristic.
- Deterministic extraction: same input → byte-identical IR (golden tests enforce this).
- AI/LLM features write to the OVERLAY only — they never mutate the extracted graph.

## Rules for you, the agent
- Implement ONLY the bounded unit in the prompt. Do not scaffold beyond it.
- Every function/module ships with tests. Extractors ship with golden-file tests vs fixtures.
- Do NOT invent architecture decisions. If a design choice is needed and unspecified, STOP and ask me.
- No new dependencies without flagging them first. Match existing conventions.
- Keep changes small and reviewable.

## Conventions
- Rust edition: 2021. Errors: thiserror for libs, anyhow at the CLI boundary.  (adjust)
- Commits: conventional commits.  Specs in docs/specs/, decisions in docs/adr/.

## File organization
- One primary type-group per file. Do not dump a whole crate into lib.rs.
- lib.rs / mod.rs contain only module declarations and re-exports.
- Keep files reviewable (~under 300 lines); split when they grow past that.