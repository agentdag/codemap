# Contributing to codemap

Thanks for your interest. The project is early and the architecture is evolving.

## Development workflow

We build in small, reviewable stages. The rule of thumb:

1. **Spec first.** Significant components get a short design doc in `docs/specs/`
   before implementation.
2. **Tests as the contract.** Extractors ship with golden-file tests against fixtures
   in `fixtures/`. The IR has determinism tests. Don't merge code you can't verify.
3. **Bounded changes.** Keep PRs focused on one component.
4. **Conventions.** Coding and file-organization conventions live in `CLAUDE.md` —
   follow them.

## Getting set up

```bash
cargo build
cargo test
```

## Proposing changes

Open an issue describing the change before large work. For new languages or codegen
targets, follow the existing plugin boundaries in `crates/extract`.