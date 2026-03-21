# NativeSpanFeed Benchmarks

## Benchmark

### Build

Build the Rust native runtime and the TypeScript package first so the benchmark harness loads the current platform library.

```bash
cd packages/core
bun run build
```

### Run

```bash
cd packages/core
bun run bench:native
```

```bash
bun src/benchmark/native-span-feed-benchmark.ts --bytes=100000 --iters=1000 --chunk=65536 --initial=2
```

### Options

Defaults are optimized (batch drain + reserve path + chunk release flags) with no
additional flags required.

- `--bytes=<n>` total bytes produced by the native runtime per iteration (default: 100000)
- `--iters=<n>` base iteration count (suite scenarios scale from this; defaults are optimized)
- `--suite=<quick|default|large|all>` run a scenario suite
- `--chunk=<n>` chunk size in bytes
- `--initial=<n>` initial chunk count
- `--auto=<0|1>` enable auto-commit on full chunks (default: 1)
- `--commit=<n>` commit every N bytes (0 disables)
- `--pattern=<str>` override the default ANSI pattern (single-run)
- `--pattern-type=<ansi|ascii|binary|random>` choose pattern kind (single-run)
- `--pattern-size=<n>` pattern size in bytes (single-run)
- `--stdout` write received bytes to stdout
- `--reuse` reuse a single stream across iterations (may grow memory)
- `--mem` enable memory tracking
- `--mem-sample=<n>` sample memory every N iterations (default: 1)
- `--mem` enable memory tracking
- `--mem-sample=<n>` sample memory every N iterations (default: 1)
- `--json[=<path>]` write results to JSON (default: `latest-<suite>-bench-run.json` when `--suite` is set, otherwise `latest-bench-run.json`)
