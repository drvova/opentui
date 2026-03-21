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
const runRustTextBufferSmoke = cargoAvailable ? test : test.skip

runRustTextBufferSmoke("Rust TextBuffer cdylib supports set/append/readback primitives", () => {
  execFileSync("cargo", ["build"], { cwd: nativeDir, stdio: "inherit" })

  const lib = dlopen(rustLibPath, {
    createTextBuffer: { args: ["u8"], returns: "ptr" },
    destroyTextBuffer: { args: ["ptr"], returns: "void" },
    textBufferGetLength: { args: ["ptr"], returns: "u32" },
    textBufferGetByteSize: { args: ["ptr"], returns: "u32" },
    textBufferReset: { args: ["ptr"], returns: "void" },
    textBufferClear: { args: ["ptr"], returns: "void" },
    textBufferRegisterMemBuffer: { args: ["ptr", "ptr", "usize", "bool"], returns: "u16" },
    textBufferReplaceMemBuffer: { args: ["ptr", "u8", "ptr", "usize", "bool"], returns: "bool" },
    textBufferClearMemRegistry: { args: ["ptr"], returns: "void" },
    textBufferSetTextFromMem: { args: ["ptr", "u8"], returns: "void" },
    textBufferAppend: { args: ["ptr", "ptr", "usize"], returns: "void" },
    textBufferAppendFromMemId: { args: ["ptr", "u8"], returns: "void" },
    textBufferGetLineCount: { args: ["ptr"], returns: "u32" },
    textBufferGetPlainText: { args: ["ptr", "ptr", "usize"], returns: "usize" },
    textBufferGetTextRange: { args: ["ptr", "u32", "u32", "ptr", "usize"], returns: "usize" },
    textBufferGetTextRangeByCoords: {
      args: ["ptr", "u32", "u32", "u32", "u32", "ptr", "usize"],
      returns: "usize",
    },
    textBufferGetTabWidth: { args: ["ptr"], returns: "u8" },
    textBufferSetTabWidth: { args: ["ptr", "u8"], returns: "void" },
  }).symbols

  const textBuffer = lib.createTextBuffer(0)
  expect(textBuffer).not.toBeNull()
  expect(textBuffer).not.toBe(0)

  const initial = new TextEncoder().encode("Hello\r\nWorld")
  const memId = lib.textBufferRegisterMemBuffer(textBuffer, initial, initial.length, false)
  expect(memId).not.toBe(0xffff)

  lib.textBufferSetTextFromMem(textBuffer, memId)
  expect(lib.textBufferGetLength(textBuffer)).toBe(10)
  expect(lib.textBufferGetLineCount(textBuffer)).toBe(2)

  const appended = new TextEncoder().encode("\nRust")
  lib.textBufferAppend(textBuffer, appended, appended.length)
  expect(lib.textBufferGetLength(textBuffer)).toBe(14)
  expect(lib.textBufferGetLineCount(textBuffer)).toBe(3)

  const out = new Uint8Array(64)
  const len = Number(lib.textBufferGetPlainText(textBuffer, out, out.length))
  expect(new TextDecoder().decode(out.slice(0, len))).toBe("Hello\nWorld\nRust")
  expect(lib.textBufferGetByteSize(textBuffer)).toBe(len)

  const range = new Uint8Array(16)
  const rangeLen = Number(lib.textBufferGetTextRange(textBuffer, 6, 11, range, range.length))
  expect(new TextDecoder().decode(range.slice(0, rangeLen))).toBe("World")

  const coordRange = new Uint8Array(16)
  const coordLen = Number(lib.textBufferGetTextRangeByCoords(textBuffer, 1, 0, 1, 5, coordRange, coordRange.length))
  expect(new TextDecoder().decode(coordRange.slice(0, coordLen))).toBe("World")

  expect(lib.textBufferGetTabWidth(textBuffer)).toBe(4)
  lib.textBufferSetTabWidth(textBuffer, 8)
  expect(lib.textBufferGetTabWidth(textBuffer)).toBe(8)

  const replacement = new TextEncoder().encode("Reset")
  expect(lib.textBufferReplaceMemBuffer(textBuffer, memId, replacement, replacement.length, false)).toBe(true)
  lib.textBufferClear(textBuffer)
  expect(lib.textBufferGetLength(textBuffer)).toBe(0)
  lib.textBufferSetTextFromMem(textBuffer, memId)

  const resetOut = new Uint8Array(16)
  const resetLen = Number(lib.textBufferGetPlainText(textBuffer, resetOut, resetOut.length))
  expect(new TextDecoder().decode(resetOut.slice(0, resetLen))).toBe("Reset")

  lib.textBufferReset(textBuffer)
  expect(lib.textBufferGetLength(textBuffer)).toBe(0)
  lib.textBufferSetTextFromMem(textBuffer, memId)
  expect(lib.textBufferGetLength(textBuffer)).toBe(0)
  lib.textBufferClearMemRegistry(textBuffer)
  lib.destroyTextBuffer(textBuffer)
})
