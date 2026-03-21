import { execFileSync, spawnSync } from "node:child_process"
import { dirname, join } from "node:path"
import { fileURLToPath } from "node:url"

import { dlopen, ptr, toArrayBuffer } from "bun:ffi"
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
const runRustBufferDrawSmoke = cargoAvailable ? test : test.skip

runRustBufferDrawSmoke("Rust OptimizedBuffer draws TextBufferView and EditorView into the char grid", () => {
  execFileSync("cargo", ["build"], { cwd: nativeDir, stdio: "inherit" })

  const lib = dlopen(rustLibPath, {
    createOptimizedBuffer: { args: ["u32", "u32", "bool", "u8", "ptr", "usize"], returns: "ptr" },
    destroyOptimizedBuffer: { args: ["ptr"], returns: "void" },
    getBufferWidth: { args: ["ptr"], returns: "u32" },
    getBufferHeight: { args: ["ptr"], returns: "u32" },
    bufferGetCharPtr: { args: ["ptr"], returns: "ptr" },
    bufferGetFgPtr: { args: ["ptr"], returns: "ptr" },
    bufferGetBgPtr: { args: ["ptr"], returns: "ptr" },
    bufferGetAttributesPtr: { args: ["ptr"], returns: "ptr" },
    bufferClear: { args: ["ptr", "ptr"], returns: "void" },
    createTextBuffer: { args: ["u8"], returns: "ptr" },
    destroyTextBuffer: { args: ["ptr"], returns: "void" },
    textBufferRegisterMemBuffer: { args: ["ptr", "ptr", "usize", "bool"], returns: "u16" },
    textBufferSetTextFromMem: { args: ["ptr", "u8"], returns: "void" },
    createTextBufferView: { args: ["ptr"], returns: "ptr" },
    destroyTextBufferView: { args: ["ptr"], returns: "void" },
    bufferDrawTextBufferView: { args: ["ptr", "ptr", "i32", "i32"], returns: "void" },
    createEditBuffer: { args: ["u8"], returns: "ptr" },
    destroyEditBuffer: { args: ["ptr"], returns: "void" },
    editBufferSetText: { args: ["ptr", "ptr", "usize"], returns: "void" },
    createEditorView: { args: ["ptr", "u32", "u32"], returns: "ptr" },
    destroyEditorView: { args: ["ptr"], returns: "void" },
    bufferDrawEditorView: { args: ["ptr", "ptr", "i32", "i32"], returns: "void" },
  }).symbols

  const buffer = lib.createOptimizedBuffer(8, 3, false, 0, null, 0)
  expect(buffer).not.toBeNull()
  expect(lib.getBufferWidth(buffer)).toBe(8)
  expect(lib.getBufferHeight(buffer)).toBe(3)

  const bg = new Float32Array([0, 0, 0, 1])
  lib.bufferClear(buffer, bg)

  const textBuffer = lib.createTextBuffer(0)
  const mem = new TextEncoder().encode("Hi")
  const memId = lib.textBufferRegisterMemBuffer(textBuffer, mem, mem.length, false)
  lib.textBufferSetTextFromMem(textBuffer, memId)
  const view = lib.createTextBufferView(textBuffer)
  lib.bufferDrawTextBufferView(buffer, view, 1, 0)

  const chars = new Uint32Array(toArrayBuffer(lib.bufferGetCharPtr(buffer), 0, 8 * 3 * 4))
  expect(chars[1]).toBe("H".codePointAt(0))
  expect(chars[2]).toBe("i".codePointAt(0))

  const edit = lib.createEditBuffer(0)
  lib.editBufferSetText(edit, new TextEncoder().encode("Ed"), 2)
  const editor = lib.createEditorView(edit, 8, 3)
  lib.bufferDrawEditorView(buffer, editor, 0, 1)
  expect(chars[8]).toBe("E".codePointAt(0))
  expect(chars[9]).toBe("d".codePointAt(0))

  lib.destroyEditorView(editor)
  lib.destroyEditBuffer(edit)
  lib.destroyTextBufferView(view)
  lib.destroyTextBuffer(textBuffer)
  lib.destroyOptimizedBuffer(buffer)
})
