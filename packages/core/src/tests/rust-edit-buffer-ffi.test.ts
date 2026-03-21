import { execFileSync, spawnSync } from "node:child_process"
import { dirname, join } from "node:path"
import { fileURLToPath } from "node:url"

import { dlopen, ptr } from "bun:ffi"
import { expect, test } from "bun:test"

import { LogicalCursorStruct } from "../zig-structs.js"

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
const runRustEditBufferSmoke = cargoAvailable ? test : test.skip

runRustEditBufferSmoke("Rust EditBuffer cdylib supports basic text editing and cursor primitives", () => {
  execFileSync("cargo", ["build"], { cwd: nativeDir, stdio: "inherit" })

  const lib = dlopen(rustLibPath, {
    createEditBuffer: { args: ["u8"], returns: "ptr" },
    destroyEditBuffer: { args: ["ptr"], returns: "void" },
    editBufferSetText: { args: ["ptr", "ptr", "usize"], returns: "void" },
    editBufferGetText: { args: ["ptr", "ptr", "usize"], returns: "usize" },
    editBufferInsertText: { args: ["ptr", "ptr", "usize"], returns: "void" },
    editBufferDeleteCharBackward: { args: ["ptr"], returns: "void" },
    editBufferNewLine: { args: ["ptr"], returns: "void" },
    editBufferSetCursorToLineCol: { args: ["ptr", "u32", "u32"], returns: "void" },
    editBufferGetCursorPosition: { args: ["ptr", "ptr"], returns: "void" },
    editBufferMoveCursorLeft: { args: ["ptr"], returns: "void" },
    editBufferMoveCursorRight: { args: ["ptr"], returns: "void" },
    editBufferMoveCursorDown: { args: ["ptr"], returns: "void" },
    editBufferMoveCursorUp: { args: ["ptr"], returns: "void" },
    editBufferGetTextBuffer: { args: ["ptr"], returns: "ptr" },
    editBufferGetId: { args: ["ptr"], returns: "u16" },
    editBufferPositionToOffset: { args: ["ptr", "u32", "u32"], returns: "u32" },
    editBufferOffsetToPosition: { args: ["ptr", "u32", "ptr"], returns: "bool" },
    editBufferGetLineStartOffset: { args: ["ptr", "u32"], returns: "u32" },
    editBufferGetTextRange: { args: ["ptr", "u32", "u32", "ptr", "usize"], returns: "usize" },
    editBufferGetTextRangeByCoords: {
      args: ["ptr", "u32", "u32", "u32", "u32", "ptr", "usize"],
      returns: "usize",
    },
    editBufferClear: { args: ["ptr"], returns: "void" },
  }).symbols

  const buffer = lib.createEditBuffer(0)
  expect(buffer).not.toBeNull()
  expect(lib.editBufferGetId(buffer)).toBeGreaterThan(0)
  expect(lib.editBufferGetTextBuffer(buffer)).not.toBe(0)

  const initial = new TextEncoder().encode("Hello")
  lib.editBufferSetText(buffer, initial, initial.length)

  const cursorBuffer = new ArrayBuffer(LogicalCursorStruct.size)
  lib.editBufferGetCursorPosition(buffer, ptr(cursorBuffer))
  expect(LogicalCursorStruct.unpack(cursorBuffer).offset).toBe(0)

  lib.editBufferSetCursorToLineCol(buffer, 0, 5)
  lib.editBufferInsertText(buffer, new TextEncoder().encode(" World"), 6)

  const out = new Uint8Array(32)
  const len = Number(lib.editBufferGetText(buffer, out, out.length))
  expect(new TextDecoder().decode(out.slice(0, len))).toBe("Hello World")

  lib.editBufferDeleteCharBackward(buffer)
  const out2 = new Uint8Array(32)
  const len2 = Number(lib.editBufferGetText(buffer, out2, out2.length))
  expect(new TextDecoder().decode(out2.slice(0, len2))).toBe("Hello Worl")

  lib.editBufferNewLine(buffer)
  lib.editBufferInsertText(buffer, new TextEncoder().encode("Next"), 4)
  const out3 = new Uint8Array(32)
  const len3 = Number(lib.editBufferGetText(buffer, out3, out3.length))
  expect(new TextDecoder().decode(out3.slice(0, len3))).toBe("Hello Worl\nNext")

  lib.editBufferSetCursorToLineCol(buffer, 1, 0)
  lib.editBufferMoveCursorUp(buffer)
  lib.editBufferGetCursorPosition(buffer, ptr(cursorBuffer))
  expect(LogicalCursorStruct.unpack(cursorBuffer).row).toBe(0)
  lib.editBufferMoveCursorDown(buffer)
  lib.editBufferGetCursorPosition(buffer, ptr(cursorBuffer))
  expect(LogicalCursorStruct.unpack(cursorBuffer).row).toBe(1)

  expect(lib.editBufferPositionToOffset(buffer, 1, 0)).toBe(11)
  expect(lib.editBufferGetLineStartOffset(buffer, 1)).toBe(11)

  const posBuffer = new ArrayBuffer(LogicalCursorStruct.size)
  expect(lib.editBufferOffsetToPosition(buffer, 11, ptr(posBuffer))).toBe(true)
  expect(LogicalCursorStruct.unpack(posBuffer).row).toBe(1)

  const range = new Uint8Array(16)
  const rangeLen = Number(lib.editBufferGetTextRange(buffer, 11, 15, range, range.length))
  expect(new TextDecoder().decode(range.slice(0, rangeLen))).toBe("Next")

  const coordRange = new Uint8Array(16)
  const coordLen = Number(lib.editBufferGetTextRangeByCoords(buffer, 1, 0, 1, 4, coordRange, coordRange.length))
  expect(new TextDecoder().decode(coordRange.slice(0, coordLen))).toBe("Next")

  lib.editBufferClear(buffer)
  const out4 = new Uint8Array(8)
  const len4 = Number(lib.editBufferGetText(buffer, out4, out4.length))
  expect(len4).toBe(0)

  lib.destroyEditBuffer(buffer)
})
