import { execFileSync, spawnSync } from "node:child_process"
import { dirname, join } from "node:path"
import { fileURLToPath } from "node:url"

import { dlopen, ptr, toArrayBuffer } from "bun:ffi"
import { expect, test } from "bun:test"

import { NativeSpanFeedOptionsStruct, NativeSpanFeedStatsStruct, SpanInfoStruct } from "../zig-structs.js"

const __filename = fileURLToPath(import.meta.url)
const __dirname = dirname(__filename)
const packageRoot = join(__dirname, "..", "..")
const nativeDir = join(packageRoot, "native")
const rustLibPath = join(
  nativeDir,
  "target",
  "debug",
  process.platform === "darwin" ? "libopentui.dylib" : process.platform === "win32" ? "opentui.dll" : "libopentui.so",
)

const cargoAvailable = spawnSync("cargo", ["--version"], { cwd: nativeDir, stdio: "ignore" }).status === 0
const runRustNativeSpanFeedSmoke = cargoAvailable ? test : test.skip

runRustNativeSpanFeedSmoke("Rust NativeSpanFeed cdylib supports create/write/commit/drain/close", () => {
  execFileSync("cargo", ["build"], { cwd: nativeDir, stdio: "inherit" })

  const lib = dlopen(rustLibPath, {
    createNativeSpanFeed: { args: ["ptr"], returns: "ptr" },
    attachNativeSpanFeed: { args: ["ptr"], returns: "i32" },
    destroyNativeSpanFeed: { args: ["ptr"], returns: "void" },
    streamWrite: { args: ["ptr", "ptr", "u64"], returns: "i32" },
    streamCommit: { args: ["ptr"], returns: "i32" },
    streamDrainSpans: { args: ["ptr", "ptr", "u32"], returns: "u32" },
    streamClose: { args: ["ptr"], returns: "i32" },
    streamGetStats: { args: ["ptr", "ptr"], returns: "i32" },
  }).symbols

  const options = NativeSpanFeedOptionsStruct.pack({
    chunkSize: 64,
    initialChunks: 1,
    autoCommitOnFull: true,
  })

  const streamPtr = lib.createNativeSpanFeed(ptr(options))
  expect(streamPtr).not.toBe(0)
  expect(streamPtr).not.toBeNull()
  expect(lib.attachNativeSpanFeed(streamPtr)).toBe(0)

  const data = new TextEncoder().encode("rust-span-feed")
  expect(lib.streamWrite(streamPtr, data, data.length)).toBe(0)
  expect(lib.streamCommit(streamPtr)).toBe(0)

  const outBuffer = new Uint8Array(SpanInfoStruct.size * 8)
  const count = lib.streamDrainSpans(streamPtr, outBuffer, 8)
  expect(count).toBe(1)

  const [span] = SpanInfoStruct.unpackList(outBuffer.buffer, count)
  expect(span.len).toBe(data.length)
  const chunk = new Uint8Array(toArrayBuffer(span.chunkPtr, 0, span.offset + span.len))
  expect(new TextDecoder().decode(chunk.slice(span.offset, span.offset + span.len))).toBe("rust-span-feed")

  const statsBuffer = new Uint8Array(NativeSpanFeedStatsStruct.size)
  expect(lib.streamGetStats(streamPtr, statsBuffer)).toBe(0)
  const stats = NativeSpanFeedStatsStruct.unpack(statsBuffer.buffer)
  expect(Number(stats.spansCommitted)).toBe(1)
  expect(Number(stats.bytesWritten)).toBe(data.length)

  expect(lib.streamClose(streamPtr)).toBe(0)
  lib.destroyNativeSpanFeed(streamPtr)
})
