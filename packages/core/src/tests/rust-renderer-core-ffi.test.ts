import { execFileSync, spawnSync } from "node:child_process"
import { dirname, join } from "node:path"
import { fileURLToPath } from "node:url"

import { dlopen, toArrayBuffer } from "bun:ffi"
import { expect, test } from "bun:test"

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
const runRustRendererCoreSmoke = cargoAvailable ? test : test.skip

runRustRendererCoreSmoke("Rust renderer core APIs expose buffers and hit-grid state", () => {
  execFileSync("cargo", ["build"], { cwd: nativeDir, stdio: "inherit" })

  const lib = dlopen(rustLibPath, {
    createRenderer: { args: ["u32", "u32", "bool", "bool"], returns: "ptr" },
    destroyRenderer: { args: ["ptr"], returns: "void" },
    setUseThread: { args: ["ptr", "bool"], returns: "void" },
    setBackgroundColor: { args: ["ptr", "ptr"], returns: "void" },
    setRenderOffset: { args: ["ptr", "u32"], returns: "void" },
    updateStats: { args: ["ptr", "f64", "u32", "f64"], returns: "void" },
    updateMemoryStats: { args: ["ptr", "u32", "u32", "u32"], returns: "void" },
    render: { args: ["ptr", "bool"], returns: "void" },
    getNextBuffer: { args: ["ptr"], returns: "ptr" },
    getCurrentBuffer: { args: ["ptr"], returns: "ptr" },
    resizeRenderer: { args: ["ptr", "u32", "u32"], returns: "void" },
    addToHitGrid: { args: ["ptr", "i32", "i32", "u32", "u32", "u32"], returns: "void" },
    addToCurrentHitGridClipped: { args: ["ptr", "i32", "i32", "u32", "u32", "u32"], returns: "void" },
    checkHit: { args: ["ptr", "u32", "u32"], returns: "u32" },
    clearCurrentHitGrid: { args: ["ptr"], returns: "void" },
    getHitGridDirty: { args: ["ptr"], returns: "bool" },
    hitGridPushScissorRect: { args: ["ptr", "i32", "i32", "u32", "u32"], returns: "void" },
    hitGridPopScissorRect: { args: ["ptr"], returns: "void" },
    hitGridClearScissorRects: { args: ["ptr"], returns: "void" },
    setDebugOverlay: { args: ["ptr", "bool", "u8"], returns: "void" },
    dumpBuffers: { args: ["ptr", "u64"], returns: "void" },
    dumpHitGrid: { args: ["ptr"], returns: "void" },
    dumpStdoutBuffer: { args: ["ptr", "u64"], returns: "void" },
    setHyperlinksCapability: { args: ["ptr", "bool"], returns: "void" },
    clearGlobalLinkPool: { args: [], returns: "void" },
    getBufferWidth: { args: ["ptr"], returns: "u32" },
    getBufferHeight: { args: ["ptr"], returns: "u32" },
    bufferGetCharPtr: { args: ["ptr"], returns: "ptr" },
  }).symbols

  const renderer = lib.createRenderer(4, 2, false, false)
  expect(renderer).not.toBeNull()
  lib.setUseThread(renderer, true)
  lib.setBackgroundColor(renderer, new Float32Array([0, 0, 1, 1]))
  lib.setRenderOffset(renderer, 3)
  lib.updateStats(renderer, 1.5, 60, 0.25)
  lib.updateMemoryStats(renderer, 1, 2, 3)

  const current = lib.getCurrentBuffer(renderer)
  const next = lib.getNextBuffer(renderer)
  expect(lib.getBufferWidth(current)).toBe(4)
  expect(lib.getBufferHeight(current)).toBe(2)
  expect(current).not.toBe(next)

  lib.addToHitGrid(renderer, 1, 0, 2, 2, 42)
  expect(lib.checkHit(renderer, 1, 0)).toBe(42)
  expect(lib.checkHit(renderer, 2, 1)).toBe(42)
  expect(lib.getHitGridDirty(renderer)).toBe(true)
  lib.clearCurrentHitGrid(renderer)
  expect(lib.checkHit(renderer, 1, 0)).toBe(0)

  lib.addToCurrentHitGridClipped(renderer, 0, 0, 1, 1, 7)
  expect(lib.checkHit(renderer, 0, 0)).toBe(7)
  lib.hitGridPushScissorRect(renderer, 0, 0, 1, 1)
  lib.hitGridPopScissorRect(renderer)
  lib.hitGridClearScissorRects(renderer)

  lib.render(renderer, true)
  lib.resizeRenderer(renderer, 8, 4)
  expect(lib.getBufferWidth(lib.getCurrentBuffer(renderer))).toBe(8)
  expect(lib.getBufferHeight(lib.getCurrentBuffer(renderer))).toBe(4)

  const chars = new Uint32Array(toArrayBuffer(lib.bufferGetCharPtr(lib.getCurrentBuffer(renderer)), 0, 8 * 4 * 4))
  expect(chars.length).toBe(32)
  lib.setDebugOverlay(renderer, true, 2)
  lib.dumpBuffers(renderer, Date.now())
  lib.dumpHitGrid(renderer)
  lib.dumpStdoutBuffer(renderer, Date.now())
  lib.setHyperlinksCapability(renderer, true)
  lib.clearGlobalLinkPool()

  lib.destroyRenderer(renderer)
})
