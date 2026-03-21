import { execFileSync, spawnSync } from "node:child_process"
import { dirname, join } from "node:path"
import { fileURLToPath } from "node:url"

import { dlopen, ptr } from "bun:ffi"
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
const bunBufferCoreTeardownCrash =
  process.platform === "linux" && process.arch === "x64" && typeof Bun !== "undefined" && Bun.version === "1.3.11"
const runRustBufferCoreSmoke = cargoAvailable && !bunBufferCoreTeardownCrash ? test : test.skip

runRustBufferCoreSmoke("Rust buffer core APIs support basic cell/effect/resize operations", () => {
  execFileSync("cargo", ["build"], { cwd: nativeDir, stdio: "inherit" })

  const lib = dlopen(rustLibPath, {
    createOptimizedBuffer: { args: ["u32", "u32", "bool", "u8", "ptr", "usize"], returns: "ptr" },
    destroyOptimizedBuffer: { args: ["ptr"], returns: "void" },
    bufferSetRespectAlpha: { args: ["ptr", "bool"], returns: "void" },
    bufferGetRespectAlpha: { args: ["ptr"], returns: "bool" },
    bufferGetId: { args: ["ptr", "ptr", "usize"], returns: "usize" },
    bufferGetRealCharSize: { args: ["ptr"], returns: "u32" },
    bufferWriteResolvedChars: { args: ["ptr", "ptr", "usize", "bool"], returns: "u32" },
    bufferSetCell: { args: ["ptr", "u32", "u32", "u32", "ptr", "ptr", "u32"], returns: "void" },
    bufferSetCellWithAlphaBlending: { args: ["ptr", "u32", "u32", "u32", "ptr", "ptr", "u32"], returns: "void" },
    bufferFillRect: { args: ["ptr", "u32", "u32", "u32", "u32", "ptr"], returns: "void" },
    bufferDrawText: { args: ["ptr", "ptr", "usize", "u32", "u32", "ptr", "ptr", "u32"], returns: "void" },
    bufferResize: { args: ["ptr", "u32", "u32"], returns: "void" },
    bufferGetCurrentOpacity: { args: ["ptr"], returns: "f32" },
    bufferPushOpacity: { args: ["ptr", "f32"], returns: "void" },
    bufferPopOpacity: { args: ["ptr"], returns: "void" },
    bufferClearOpacity: { args: ["ptr"], returns: "void" },
    bufferPushScissorRect: { args: ["ptr", "i32", "i32", "u32", "u32"], returns: "void" },
    bufferPopScissorRect: { args: ["ptr"], returns: "void" },
    bufferClearScissorRects: { args: ["ptr"], returns: "void" },
    bufferDrawChar: { args: ["ptr", "u32", "u32", "u32", "ptr", "ptr", "u32"], returns: "void" },
    bufferColorMatrixUniform: { args: ["ptr", "ptr", "f32", "u8"], returns: "void" },
    drawFrameBuffer: { args: ["ptr", "i32", "i32", "ptr", "u32", "u32", "u32", "u32"], returns: "void" },
    getBufferWidth: { args: ["ptr"], returns: "u32" },
    getBufferHeight: { args: ["ptr"], returns: "u32" },
    bufferGetCharPtr: { args: ["ptr"], returns: "ptr" },
  }).symbols

  const id = new TextEncoder().encode("buf")
  const buffer = lib.createOptimizedBuffer(4, 2, false, 0, id, id.length)
  expect(buffer).not.toBeNull()
  expect(lib.bufferGetRespectAlpha(buffer)).toBe(false)
  lib.bufferSetRespectAlpha(buffer, true)
  expect(lib.bufferGetRespectAlpha(buffer)).toBe(true)

  const fg = new Float32Array([1, 1, 1, 1])
  const bg = new Float32Array([0, 0, 0, 1])
  lib.bufferSetCell(buffer, 0, 0, "A".codePointAt(0)!, fg, bg, 7)
  lib.bufferSetCellWithAlphaBlending(buffer, 1, 0, "B".codePointAt(0)!, fg, bg, 8)
  lib.bufferFillRect(buffer, 0, 1, 4, 1, new Float32Array([0, 0, 1, 1]))
  lib.bufferDrawText(buffer, new TextEncoder().encode("CD"), 2, 0, fg, bg, 9)
  lib.bufferDrawChar(buffer, "E".codePointAt(0)!, 0, 1, fg, bg, 1)
  lib.bufferPushOpacity(buffer, 0.5)
  expect(lib.bufferGetCurrentOpacity(buffer)).toBe(0.5)
  lib.bufferPopOpacity(buffer)
  expect(lib.bufferGetCurrentOpacity(buffer)).toBe(1)
  lib.bufferPushScissorRect(buffer, 0, 0, 1, 1)
  lib.bufferPopScissorRect(buffer)
  lib.bufferClearScissorRects(buffer)
  lib.bufferColorMatrixUniform(buffer, new Float32Array(16), 0.5, 3)

  const out = new Uint8Array(32)
  const len = Number(lib.bufferWriteResolvedChars(buffer, out, out.length, true))
  expect(len).toBeGreaterThan(0)
  expect(lib.bufferGetRealCharSize(buffer)).toBeGreaterThan(0)
  const nameOut = new Uint8Array(16)
  const nameLen = Number(lib.bufferGetId(buffer, nameOut, nameOut.length))
  expect(new TextDecoder().decode(nameOut.slice(0, nameLen))).toContain("buf")
  expect(lib.bufferGetCharPtr(buffer)).not.toBe(0)
  expect(lib.bufferGetFgPtr(buffer)).not.toBe(0)
  expect(lib.bufferGetBgPtr(buffer)).not.toBe(0)
  expect(lib.bufferGetAttributesPtr(buffer)).not.toBe(0)

  const resolved = new Uint8Array(32)
  const resolvedLen = Number(lib.bufferWriteResolvedChars(buffer, resolved, resolved.length, true))
  expect(new TextDecoder().decode(resolved.slice(0, resolvedLen))).toContain("ABCD")

  const child = lib.createOptimizedBuffer(2, 1, false, 0, null, 0)
  lib.bufferSetCell(child, 0, 0, "X".codePointAt(0)!, fg, bg, 0)
  lib.bufferSetCell(child, 1, 0, "Y".codePointAt(0)!, fg, bg, 0)
  lib.drawFrameBuffer(buffer, 2, 1, child, 0, 0, 0, 0)
  const resolvedAfter = new Uint8Array(64)
  const resolvedAfterLen = Number(lib.bufferWriteResolvedChars(buffer, resolvedAfter, resolvedAfter.length, true))
  expect(new TextDecoder().decode(resolvedAfter.slice(0, resolvedAfterLen))).toContain("XY")

  lib.bufferResize(buffer, 6, 3)
  expect(lib.getBufferWidth(buffer)).toBe(6)
  expect(lib.getBufferHeight(buffer)).toBe(3)

  lib.destroyOptimizedBuffer(child)
  lib.destroyOptimizedBuffer(buffer)
})
