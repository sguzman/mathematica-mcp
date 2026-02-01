Based on the `docs/migration/structure.md` template, this document records the layout, tooling, and expectations of the Ruby `AnyStyle` reference implementation (see `tmp/anystyle`) so the Rust port knows what to mirror.

# Summary

- **Reference path**: `tmp/anystyle`
- **Project description**: `AnyStyle` is a smart parser for bibliographic references. Its CLI, parser, finder, and training workflows ingest unstructured citation text and emit labeled metadata (JSON/XML). The Ruby repo keeps `lib` for core logic, `res` for training/tagged data, and a `spec` suite that codifies expected parsing behavior.

# Language and ecosystem

- **Primary language / runtime**: Ruby (MRI, 2.2+). The code relies on Ruby’s object model, gems, and Rake tasks for orchestration. It also uses C bindings indirectly via gems like `wapiti`.
- **Version constraints**: `s.required_ruby_version = '>= 2.2'` in `anystyle.gemspec`; the tooling assumes `bundler` + `rake` for task execution.

# Dependencies

| Name | Purpose | Version / Constraint | Notes |
| ---- | ------- | -------------------- | ----- |
| `bibtex-ruby` | Runtime parsing/formatting of BibTeX records | `~>6.0` | Called from formatters. |
| `anystyle-data` | Default parser/finder models and training corpus | `~>1.3` | Defines core datasets in `res/`. |
| `wapiti` | Conditional random field library powering training/labeling | `~>2.1` | Exposes `Wapiti::Dataset` helpers used throughout parser/finder/training. |
| `namae` | Name parsing helpers for author normalization | `~>1.0` | Used in `lib/anystyle/normalizer.rb`. |
| `rake` | Task runner | Bundled (Gemfile) | Defines build, train, check, and helper tasks. |
| `rspec` | Test framework | Bundled (Gemfile) | Executes `spec/**/*_spec.rb`. |
| `yaml` | Configuration helpers | Bundled (Gemfile) | Used by parsers and data loading. |
| `simplecov` / `simplecov-lcov` | Coverage reporting (coverage group) | Latest | Optional coverage instrumentation. |
| `debug`, `ruby-prof`, `gnuplot` | Debugging/profiling (debug/profile groups) | >=1.0 for `debug` | Opt-in profiling utilities. |
| `lmdb`, `redis`, `redis-namespace`, `edtf`, `bibtex-ruby`, `citeproc`, `unicode-scripts`, `cld3` | Optional/extra feature gems | (group :extra) | Provide alternate dictionary adapters, language detection, and extended metadata support. |

# Build process

- **Commands**: `bundle install` to sync gems; `bundle exec rake build` calls `gem build anystyle-parser.gemspec` after cleaning. The default Rake task runs `rspec` via `RSpec::Core::RakeTask`.
- **Artifacts produced**: A Ruby gem (`anystyle-parser-<version>.gem`) plus `*.gem` artifacts cleared by `CLEAN.include('*.gem')`.
- **Special steps**: Rake orchestrates `train`, `find`, `check`, and `delta` commands that load `lib/anystyle` and the `res` data sets; `train` reruns `Wapiti` training and saves new models, while `check` compares predictions against tagged XML/TTX files in `res/parser` and `res/finder`.

# Run and runtime behavior

- **Launch commands**: The CLI is exposed through the `anystyle` gem (`bundle exec anystyle parse …`, `anystyle find …`). Rake tasks like `bundle exec rake find[input]` provide auxiliary flows. `lib/anystyle/cli.rb` (placeholder for future CLI) is referenced indirectly through Rake (`task :find` / `:train`).
- **Runtime assumptions**: The parser loads `tmp/anystyle/res/parser/*.xml` and `res/finder/*.ttx` for training/evaluation; `AnyStyle.parser` and `.finder` provide shared singleton instances with configurable threads (default 4). Optional dictionary adapters assume either memory-backed Ruby hashes, GDBM, or Redis, depending on configuration (see `lib/anystyle/dictionary`).
- **Observability**: Logging occurs via standard output/STDERR (see Rake task prints). There is no centralized telemetry; validation feedback is text-based token/sequence error rates.

# Testing and validation

- **Test suites**: `bundle exec rspec` runs the unit/regression suite under `spec/`. The default `rake` task delegates to `RSpec::Core::RakeTask`.
- **Test data**: Tagged parser documents live in `res/parser/*.xml`; finder datasets are under `res/finder/*.ttx`. These feed both `rake check` and `delta` tasks.
- **Validation steps**: `rake check` iterates parser or finder datasets, prints error rates, and compares labeled output to curated ground truth. `rake delta` calculates and stores the delta between predictions and gold data for manual inspection (example outputs in `tmp/anystyle`).
- **Rust validation**: `tests/reference_training.rs` now drives the same training/check/delta flow via the Rust CLI helpers (`cite_otter::cli::{training_report, validation_report, delta_report}`) and asserts the JSON report files in `target/reports` contain parser/finder statistics.

# Attribution

- Thanks to the original `AnyStyle` project (`https://github.com/inukshuk/anystyle`) for the parser/finder architecture and the Res training artifacts. This REFERENCE document mirrors the structure they built so the Rust port stays aligned while re-implementing the core experience.
