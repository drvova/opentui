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
const runRustTextBufferViewSmoke = cargoAvailable ? test : test.skip

runRustTextBufferViewSmoke("Rust TextBufferView cdylib supports selection and wrap-count primitives", () => {
  execFileSync("cargo", ["build"], { cwd: nativeDir, stdio: "inherit" })

  const lib = dlopen(rustLibPath, {
    createTextBuffer: { args: ["u8"], returns: "ptr" },
    destroyTextBuffer: { args: ["ptr"], returns: "void" },
    textBufferRegisterMemBuffer: { args: ["ptr", "ptr", "usize", "bool"], returns: "u16" },
    textBufferSetTextFromMem: { args: ["ptr", "u8"], returns: "void" },
    createTextBufferView: { args: ["ptr"], returns: "ptr" },
    destroyTextBufferView: { args: ["ptr"], returns: "void" },
    textBufferViewSetSelection: { args: ["ptr", "u32", "u32", "ptr", "ptr"], returns: "void" },
    textBufferViewResetSelection: { args: ["ptr"], returns: "void" },
    textBufferViewGetSelectionInfo: { args: ["ptr"], returns: "u64" },
    textBufferViewUpdateSelection: { args: ["ptr", "u32", "ptr", "ptr"], returns: "void" },
    textBufferViewSetWrapMode: { args: ["ptr", "u8"], returns: "void" },
    textBufferViewSetWrapWidth: { args: ["ptr", "u32"], returns: "void" },
    textBufferViewGetVirtualLineCount: { args: ["ptr"], returns: "u32" },
    textBufferViewGetSelectedText: { args: ["ptr", "ptr", "usize"], returns: "usize" },
    textBufferViewGetPlainText: { args: ["ptr", "ptr", "usize"], returns: "usize" },
  }).symbols

  const textBuffer = lib.createTextBuffer(0)
  const mem = new TextEncoder().encode("Hello World")
  const memId = lib.textBufferRegisterMemBuffer(textBuffer, mem, mem.length, false)
  lib.textBufferSetTextFromMem(textBuffer, memId)

  const view = lib.createTextBufferView(textBuffer)
  expect(view).not.toBeNull()
  expect(typeof lib.textBufferViewGetSelectionInfo(view) === "bigint" ? lib.textBufferViewGetSelectionInfo(view) : BigInt(lib.textBufferViewGetSelectionInfo(view))).toBe(0xffff_ffff_ffff_ffffn)

  lib.textBufferViewSetSelection(view, 6, 11, null, null)
  const packed = lib.textBufferViewGetSelectionInfo(view)
  expect(typeof packed === "bigint" ? packed : BigInt(packed)).toBe((6n << 32n) | 11n)

  const selected = new Uint8Array(16)
  const selectedLen = Number(lib.textBufferViewGetSelectedText(view, selected, selected.length))
  expect(new TextDecoder().decode(selected.slice(0, selectedLen))).toBe("World")

  lib.textBufferViewUpdateSelection(view, 8, null, null)
  const selected2 = new Uint8Array(16)
  const selectedLen2 = Number(lib.textBufferViewGetSelectedText(view, selected2, selected2.length))
  expect(new TextDecoder().decode(selected2.slice(0, selectedLen2))).toBe("Wo")

  const plain = new Uint8Array(16)
  const plainLen = Number(lib.textBufferViewGetPlainText(view, plain, plain.length))
  expect(new TextDecoder().decode(plain.slice(0, plainLen))).toBe("Hello World")

  lib.textBufferViewSetWrapMode(view, 1)
  lib.textBufferViewSetWrapWidth(view, 5)
  expect(lib.textBufferViewGetVirtualLineCount(view)).toBe(3)

  lib.textBufferViewResetSelection(view)
  const resetPacked = lib.textBufferViewGetSelectionInfo(view)
  expect(typeof resetPacked === "bigint" ? resetPacked : BigInt(resetPacked)).toBe(0xffff_ffff_ffff_ffffn)

  lib.destroyTextBufferView(view)
  lib.destroyTextBuffer(textBuffer)
})
