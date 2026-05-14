#!/usr/bin/env node
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const __dirname = dirname(fileURLToPath(import.meta.url));

const targets = {
  'linux-x64': 'throw-trace-x86_64-unknown-linux-gnu',
  'linux-arm64': 'throw-trace-aarch64-unknown-linux-gnu',
  'darwin-x64': 'throw-trace-x86_64-apple-darwin',
  'darwin-arm64': 'throw-trace-aarch64-apple-darwin',
  'win32-x64': 'throw-trace-x86_64-pc-windows-msvc.exe',
};

// Used by CI smoke tests to exercise the unsupported-platform branch.
const forcedPlatform = process.env.THROW_TRACE_FORCE_PLATFORM;
const key = forcedPlatform ?? `${process.platform}-${process.arch}`;
const binaryName = targets[key];

if (!binaryName) {
  process.stderr.write(`throw-trace: unsupported platform ${key}\n`);
  process.exit(1);
}

const result = spawnSync(
  join(__dirname, '..', 'binaries', binaryName),
  process.argv.slice(2),
  { stdio: 'inherit' },
);

if (result.error) {
  process.stderr.write(`throw-trace: failed to spawn binary: ${result.error.message}\n`);
  process.exit(1);
}

process.exit(result.status ?? 1);
