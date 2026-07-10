# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.9](https://github.com/9wick/throw-trace/compare/v0.1.8...v0.1.9) - 2026-07-10

### Added

- synchronize generated throws catalog
- render propagation trace with source locations in diagnostics

### Other

- migrate core throw catalog cases
- Merge remote-tracking branch 'origin/main' into feat/sync-throw-catalog
- Merge pull request #27 from 9wick/improve-error-reporting

### Changed

- make `fix` synchronize generated `@throws` declarations by adding missing entries and removing stale entries

### Other

- add before/expected golden catalog coverage for direct throw synchronization

## [0.1.8](https://github.com/9wick/throw-trace/compare/v0.1.7...v0.1.8) - 2026-07-09

### Added

- add persistent cache

### Fixed

- resolve type parameter throws against declared constraint
- suggest declarable unknown type in help, document unknown handling
- scope diagnostics cache invalidation to actual dependencies
- preserve file format in fix command and separate CLI exit codes

### Other

- apply cargo fmt
- catalog closure attribution as false positive, add cross-file rethrow coverage
- Merge pull request #22 from 9wick/fix-review-findings
- document fix command and exit codes in README

## [0.1.7](https://github.com/9wick/throw-trace/compare/v0.1.6...v0.1.7) - 2026-05-31

### Fixed

- use call site location instead of name-based matching in propagation
- skip unreachable throw-e after if/else that both terminate
- detect reachable catch-param rethrow in instanceof branch
- treat catch param rethrow in instanceof branch as non-terminating
- handle duplicate calls and partial instanceof termination
- improve catch block handling and add member call propagation

### Other

- fix redundant closure and manual assert clippy warnings
- inline format args to fix clippy warning
- add catalog e2e tests for throw pattern coverage

## [0.1.6](https://github.com/9wick/throw-trace/compare/v0.1.5...v0.1.6) - 2026-05-25

### Added

- add fix command for auto-inserting @throws declarations

### Other

- Merge pull request #14 from 9wick/feat/fix-command

## [0.1.5](https://github.com/9wick/throw-trace/compare/v0.1.4...v0.1.5) - 2026-05-16

### Other

- minor whitespace fix

## [0.1.4](https://github.com/9wick/throw-trace/compare/v0.1.3...v0.1.4) - 2026-05-16

### Other

- add LICENSE link to README

## [0.1.3](https://github.com/9wick/throw-trace/compare/v0.1.2...v0.1.3) - 2026-05-16

### Other

- clarify license text

## [0.1.2](https://github.com/9wick/throw-trace/compare/v0.1.1...v0.1.2) - 2026-05-15

### Other

- add crates.io and npm badges

## [0.1.1](https://github.com/9wick/throw-trace/compare/v0.1.0...v0.1.1) - 2026-05-14

### Fixed

- resolve clippy warnings and format issues

### Other

- apply cargo fmt
- update install instructions for npm and cargo
