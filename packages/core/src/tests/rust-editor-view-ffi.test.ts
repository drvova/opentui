import { execFileSync, spawnSync } from "node:child_process"
import { dirname, join } from "node:path"
import { fileURLToPath } from "node:url"

import { dlopen, ptr } from "bun:ffi"
import { expect, test } from "bun:test"

import { LineInfoStruct, StyledChunkStruct, VisualCursorStruct } from "../zig-structs.js"

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
const runRustEditorViewSmoke = cargoAvailable ? test : test.skip

runRustEditorViewSmoke("Rust EditorView cdylib supports viewport, wrap, selection, and text passthrough", () => {
  execFileSync("cargo", ["build"], { cwd: nativeDir, stdio: "inherit" })

  const lib = dlopen(rustLibPath, {
    createEditBuffer: { args: ["u8"], returns: "ptr" },
    destroyEditBuffer: { args: ["ptr"], returns: "void" },
    editBufferSetText: { args: ["ptr", "ptr", "usize"], returns: "void" },
    createEditorView: { args: ["ptr", "u32", "u32"], returns: "ptr" },
    destroyEditorView: { args: ["ptr"], returns: "void" },
    editorViewSetViewportSize: { args: ["ptr", "u32", "u32"], returns: "void" },
    editorViewSetViewport: { args: ["ptr", "u32", "u32", "u32", "u32", "bool"], returns: "void" },
    editorViewGetViewport: { args: ["ptr", "ptr", "ptr", "ptr", "ptr"], returns: "void" },
    editorViewSetScrollMargin: { args: ["ptr", "f32"], returns: "void" },
    editorViewSetWrapMode: { args: ["ptr", "u8"], returns: "void" },
    editorViewGetVirtualLineCount: { args: ["ptr"], returns: "u32" },
    editorViewGetTotalVirtualLineCount: { args: ["ptr"], returns: "u32" },
    editorViewGetTextBufferView: { args: ["ptr"], returns: "ptr" },
    editorViewSetSelection: { args: ["ptr", "u32", "u32", "ptr", "ptr"], returns: "void" },
    editorViewResetSelection: { args: ["ptr"], returns: "void" },
    editorViewGetSelection: { args: ["ptr"], returns: "u64" },
    editorViewUpdateSelection: { args: ["ptr", "u32", "ptr", "ptr"], returns: "void" },
    editorViewGetSelectedTextBytes: { args: ["ptr", "ptr", "usize"], returns: "usize" },
    editorViewGetCursor: { args: ["ptr", "ptr", "ptr"], returns: "void" },
    editorViewGetText: { args: ["ptr", "ptr", "usize"], returns: "usize" },
    editorViewGetLineInfoDirect: { args: ["ptr", "ptr"], returns: "void" },
    editorViewGetLogicalLineInfoDirect: { args: ["ptr", "ptr"], returns: "void" },
    editorViewSetLocalSelection: {
      args: ["ptr", "i32", "i32", "i32", "i32", "ptr", "ptr", "bool", "bool"],
      returns: "bool",
    },
    editorViewUpdateLocalSelection: {
      args: ["ptr", "i32", "i32", "i32", "i32", "ptr", "ptr", "bool", "bool"],
      returns: "bool",
    },
    editorViewResetLocalSelection: { args: ["ptr"], returns: "void" },
    editorViewGetVisualCursor: { args: ["ptr", "ptr"], returns: "void" },
    editorViewMoveUpVisual: { args: ["ptr"], returns: "void" },
    editorViewMoveDownVisual: { args: ["ptr"], returns: "void" },
    editorViewDeleteSelectedText: { args: ["ptr"], returns: "void" },
    editorViewSetCursorByOffset: { args: ["ptr", "u32"], returns: "void" },
    editorViewGetNextWordBoundary: { args: ["ptr", "ptr"], returns: "void" },
    editorViewGetPrevWordBoundary: { args: ["ptr", "ptr"], returns: "void" },
    editorViewGetEOL: { args: ["ptr", "ptr"], returns: "void" },
    editorViewGetVisualSOL: { args: ["ptr", "ptr"], returns: "void" },
    editorViewGetVisualEOL: { args: ["ptr", "ptr"], returns: "void" },
    editorViewSetPlaceholderStyledText: { args: ["ptr", "ptr", "usize"], returns: "void" },
    editorViewSetTabIndicator: { args: ["ptr", "u32"], returns: "void" },
    editorViewSetTabIndicatorColor: { args: ["ptr", "ptr"], returns: "void" },
  }).symbols

  const edit = lib.createEditBuffer(0)
  lib.editBufferSetText(edit, new TextEncoder().encode("Hello World"), 11)

  const view = lib.createEditorView(edit, 40, 10)
  expect(view).not.toBeNull()
  expect(lib.editorViewGetTextBufferView(view)).not.toBe(0)

  const x = new Uint32Array(1)
  const y = new Uint32Array(1)
  const width = new Uint32Array(1)
  const height = new Uint32Array(1)
  lib.editorViewGetViewport(view, x, y, width, height)
  expect(width[0]).toBe(40)
  expect(height[0]).toBe(10)

  lib.editorViewSetViewportSize(view, 80, 20)
  lib.editorViewSetViewport(view, 2, 4, 80, 20, true)
  lib.editorViewGetViewport(view, x, y, width, height)
  expect(x[0]).toBe(2)
  expect(y[0]).toBe(4)
  expect(width[0]).toBe(80)
  expect(height[0]).toBe(20)

  expect(lib.editorViewGetVirtualLineCount(view)).toBe(1)
  lib.editorViewSetWrapMode(view, 1)
  lib.editorViewSetViewportSize(view, 5, 20)
  expect(lib.editorViewGetVirtualLineCount(view)).toBe(3)
  expect(lib.editorViewGetTotalVirtualLineCount(view)).toBe(3)
  lib.editorViewSetViewport(view, 0, 0, 5, 20, true)

  const lineInfoBuffer = new ArrayBuffer(LineInfoStruct.size)
  lib.editorViewGetLineInfoDirect(view, ptr(lineInfoBuffer))
  const lineInfo = LineInfoStruct.unpack(lineInfoBuffer)
  expect((lineInfo.startCols as number[]).length).toBeGreaterThan(0)
  const logicalInfoBuffer = new ArrayBuffer(LineInfoStruct.size)
  lib.editorViewGetLogicalLineInfoDirect(view, ptr(logicalInfoBuffer))
  const logicalInfo = LineInfoStruct.unpack(logicalInfoBuffer)
  expect((logicalInfo.startCols as number[]).length).toBeGreaterThan(0)

  lib.editorViewSetSelection(view, 6, 11, null, null)
  let packed = lib.editorViewGetSelection(view)
  expect(typeof packed === "bigint" ? packed : BigInt(packed)).toBe((6n << 32n) | 11n)

  const selected = new Uint8Array(16)
  const selectedLen = Number(lib.editorViewGetSelectedTextBytes(view, selected, selected.length))
  expect(new TextDecoder().decode(selected.slice(0, selectedLen))).toBe("World")

  lib.editorViewUpdateSelection(view, 8, null, null)
  const selected2 = new Uint8Array(16)
  const selectedLen2 = Number(lib.editorViewGetSelectedTextBytes(view, selected2, selected2.length))
  expect(new TextDecoder().decode(selected2.slice(0, selectedLen2))).toBe("Wo")

  const row = new Uint32Array(1)
  const col = new Uint32Array(1)
  lib.editorViewGetCursor(view, row, col)
  expect(row[0]).toBe(0)
  expect(col[0]).toBe(0)

  const visualCursorBuffer = new ArrayBuffer(VisualCursorStruct.size)
  lib.editorViewGetVisualCursor(view, ptr(visualCursorBuffer))
  expect(VisualCursorStruct.unpack(visualCursorBuffer).visualRow).toBe(0)

  expect(lib.editorViewSetLocalSelection(view, 0, 0, 5, 1, null, null, false, false)).toBe(true)
  const localSelected = new Uint8Array(16)
  const localSelectedLen = Number(lib.editorViewGetSelectedTextBytes(view, localSelected, localSelected.length))
  expect(localSelectedLen).toBeGreaterThan(0)
  expect(lib.editorViewUpdateLocalSelection(view, 0, 0, 2, 0, null, null, false, false)).toBe(true)
  lib.editorViewResetLocalSelection(view)

  lib.editorViewSetCursorByOffset(view, 6)
  lib.editorViewGetVisualCursor(view, ptr(visualCursorBuffer))
  expect(VisualCursorStruct.unpack(visualCursorBuffer).offset).toBe(6)

  lib.editorViewGetNextWordBoundary(view, ptr(visualCursorBuffer))
  expect(VisualCursorStruct.unpack(visualCursorBuffer).offset).toBeGreaterThanOrEqual(6)
  lib.editorViewGetPrevWordBoundary(view, ptr(visualCursorBuffer))
  expect(VisualCursorStruct.unpack(visualCursorBuffer).offset).toBeLessThanOrEqual(6)
  lib.editorViewGetEOL(view, ptr(visualCursorBuffer))
  expect(VisualCursorStruct.unpack(visualCursorBuffer).offset).toBeGreaterThanOrEqual(6)
  lib.editorViewGetVisualSOL(view, ptr(visualCursorBuffer))
  expect(VisualCursorStruct.unpack(visualCursorBuffer).visualCol).toBe(0)
  lib.editorViewGetVisualEOL(view, ptr(visualCursorBuffer))
  expect(VisualCursorStruct.unpack(visualCursorBuffer).offset).toBeGreaterThanOrEqual(6)

  lib.editorViewMoveUpVisual(view)
  lib.editorViewMoveDownVisual(view)

  const placeholder = StyledChunkStruct.packList([{ text: "placeholder", fg: null, bg: null, attributes: 0 }])
  lib.editorViewSetPlaceholderStyledText(view, ptr(placeholder), 1)
  lib.editorViewSetTabIndicator(view, ".".codePointAt(0)!)
  lib.editorViewSetTabIndicatorColor(view, new Float32Array([1, 0, 0, 1]))

  const text = new Uint8Array(16)
  const textLen = Number(lib.editorViewGetText(view, text, text.length))
  expect(new TextDecoder().decode(text.slice(0, textLen))).toBe("Hello World")

  lib.editorViewSetSelection(view, 6, 11, null, null)
  lib.editorViewDeleteSelectedText(view)
  const deletedText = new Uint8Array(16)
  const deletedTextLen = Number(lib.editorViewGetText(view, deletedText, deletedText.length))
  expect(new TextDecoder().decode(deletedText.slice(0, deletedTextLen))).toBe("Hello ")

  lib.editorViewResetSelection(view)
  packed = lib.editorViewGetSelection(view)
  expect(typeof packed === "bigint" ? packed : BigInt(packed)).toBe(0xffff_ffff_ffff_ffffn)

  lib.destroyEditorView(view)
  lib.destroyEditBuffer(edit)
})
