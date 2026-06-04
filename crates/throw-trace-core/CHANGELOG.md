# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.7](https://github.com/9wick/throw-trace/compare/throw-trace-core-v0.1.6...throw-trace-core-v0.1.7) - 2026-05-31

### Fixed

- use call site location instead of name-based matching in propagation
- handle duplicate calls and partial instanceof termination
- improve catch block handling and add member call propagation

### Other

- apply cargo fmt
- allow similar_names for caller/callee in test
- apply cargo fmt

## [0.1.6](https://github.com/9wick/throw-trace/compare/throw-trace-core-v0.1.5...throw-trace-core-v0.1.6) - 2026-05-25

### Added

- add fix command for auto-inserting @throws declarations

## [0.1.1](https://github.com/9wick/throw-trace/compare/throw-trace-core-v0.1.0...throw-trace-core-v0.1.1) - 2026-05-14

### Fixed

- resolve clippy warnings and format issues

### Other

- apply cargo fmt
