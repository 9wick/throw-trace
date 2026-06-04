# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.7](https://github.com/9wick/throw-trace/compare/throw-trace-ts-v0.1.6...throw-trace-ts-v0.1.7) - 2026-05-31

### Fixed

- skip unreachable throw-e after if/else that both terminate
- detect reachable catch-param rethrow in instanceof branch
- treat catch param rethrow in instanceof branch as non-terminating
- handle duplicate calls and partial instanceof termination
- improve catch block handling and add member call propagation

### Other

- merge match arms to fix clippy match_same_arms warning
- apply cargo fmt

## [0.1.6](https://github.com/9wick/throw-trace/compare/throw-trace-ts-v0.1.5...throw-trace-ts-v0.1.6) - 2026-05-25

### Added

- add fix command for auto-inserting @throws declarations

## [0.1.1](https://github.com/9wick/throw-trace/compare/throw-trace-ts-v0.1.0...throw-trace-ts-v0.1.1) - 2026-05-14

### Fixed

- resolve clippy warnings and format issues

### Other

- apply cargo fmt
