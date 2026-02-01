# Cite-Otter Roadmap

This migration roadmap translates `tmp/anystyle`’s structure (
`docs/migration/structure.md`) into an incremental Rustimplementation so the
final release meets the stated `v1.0.0` SemVer goal. Each milestone builds on
the prior (base features + tests first) and references the reference project’s
modules, CLI surface, and validation suite.

## SemVer Timeline

- **`v0.1.0` – Fundamentals & validation**
  - Port the workspace-level scaffolding described in
    `docs/migration/structure.md` (`lib/`, `res/`, `spec/` equivalents) into
    `src/` modules, starting with parser/core, CLI entry, and configuration
    structs.
  - Implement base CLI commands (`parse`, `help`) that mirror the Ruby tool’s
    behavior; parsing output should already support JSON and simple text
    fallbacks.
  - Reimplement the AnyStyle parser/finder runtime singletons inside Rust,
    wiring `nom`/ `regex` tokenization and stubbed labeling to match the Ruby
    singleton expectations.
  - Establish the test harness: unit tests for parser components and CLI
    integration tests derived from `spec/**/*_spec.rb` outputs. Tests must run
    via `cargo test` and should assert the same labeled fields the reference
    parser produces.
  - Release artifact: `cite-otter v0.1.0` with documented CLI usage and a
    passing core test suite.

- **`v0.5.0` – Training, finder, and richer input**
  - Add training data management mirroring `res/parser/*.xml` and
    `res/finder/*.ttx`, either via bundled fixtures or an importer helper.
    Provide logic to load/apply these datasets to the parser/finder models.
  - Implement finder flows (`find` command) that detect references in generic
    text, label them, and hand them to the parser for full metadata extraction.
  - Support base dictionary adapters (in-memory, file-backed) with pluggable
    configurations, aligning with `lib/anystyle/dictionary` behaviors.
  - Expand CLI serialization output modes (JSON, XML) and add optional `format`
    marshalling helpers to match `AnyStyle::Format`.
  - Augment test coverage with training/check validation: port `rake check`,
    `rake delta`, and `rake train` expectations into Rust integration tests
    using representative datasets.
  - Parser heuristics now emit the same metadata tokens `AnyStyle` expects
    (e.g., container-title/volume/issue/genre/edition) and the
    training/check/delta helpers can be invoked programmatically to produce
    `target/reports/*.json`.
  - Release: `cite-otter v0.5.0` with finder/training parity, dictionary
    adapters, and enhanced test suite confirming dataset-driven outputs.
  - Persisted training reports now include JSON/BibTeX/CSL renderings of a
    curated sample reference set, captured via the new helper utilities in
    `src/cli.rs`, so formatting parity can be inspected without re-running the
    CLI sample command.
  - Update the release documentation (`README.md`, `docs/migration/REFERENCE.md`
    and `change logs`) to describe the CLI/training parity, where reports/models
    reside, and how to verify `train/check/delta` before tagging.

- **`v1.0.0` – Polished release with documentation & parity**
  - Finalize CLI parity (`parse`, `find`, `train`, `check`, `delta`) with
    precise flag sets, help text, and error handling informed by the reference
    docs.
  - Ensure logging, observability, and validation mirrors the Ruby rake tasks:
    output token/sequence error rates, surface delta results, and keep
    config/adapter overrides in sync with `docs/migration/structure.md`
    observations.
  - Produce comprehensive documentation (`README.md`, `REFERENCE.md`, `docs/…`)
    explaining feature parity, training data, and references to the AnyStyle
    project as required by the migration instructions.
  - Run full test suite (`cargo test`, `cargo fmt`, any additional validation
    scripts) and document release steps in `release.toml`.
  - Tag and publish `cite-otter v1.0.0`, emphasizing that every behavior
    referenced in `tmp/anystyle` is now implemented in Rust, with tests and docs
    covering the migration claims.

## Testing & Validation (cross-Cutting)

- Mirror the Ruby `spec/` expectations by translating their assertions into Rust
  unit/integration tests. Each CLI command should have at least one scenario
  capturing a real-world reference string.
- Create fixture files derived from `tmp/anystyle/res/` to drive parser/finder
  checks; tests should call the same validation entry points (`check`, `delta`)
  and compare JSON/XML outputs to known good values.
- Ensure continuous validation via `cargo test` for each release phase; add
  `just test` or similar scripts if needed to keep commands consistent with the
  Ruby `rake spec`/ `check` flows.

## References

- `docs/migration/structure.md` for high-level architecture and module
  expectations.
- `tmp/anystyle` for CLI behavior, training data layout, and Rake orchestration
  logic.

*** End Patch
