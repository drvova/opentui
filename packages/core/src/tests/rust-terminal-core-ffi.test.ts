import { execFileSync, spawnSync } from "node:child_process"
import { dirname, join } from "node:path"
import { fileURLToPath } from "node:url"

import { dlopen, ptr } from "bun:ffi"
import { expect, test } from "bun:test"

import { CursorStateStruct, CursorStyleOptionsStruct, TerminalCapabilitiesStruct } from "../native-structs.js"

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
const runRustTerminalCoreSmoke = cargoAvailable ? test : test.skip

runRustTerminalCoreSmoke("Rust terminal core APIs are stateful through the renderer handle", () => {
  execFileSync("cargo", ["build"], { cwd: nativeDir, stdio: "inherit" })

  const lib = dlopen(rustLibPath, {
    createRenderer: { args: ["u32", "u32", "bool", "bool"], returns: "ptr" },
    destroyRenderer: { args: ["ptr"], returns: "void" },
    setCursorPosition: { args: ["ptr", "i32", "i32", "bool"], returns: "void" },
    setCursorColor: { args: ["ptr", "ptr"], returns: "void" },
    getCursorState: { args: ["ptr", "ptr"], returns: "void" },
    setCursorStyleOptions: { args: ["ptr", "ptr"], returns: "void" },
    setKittyKeyboardFlags: { args: ["ptr", "u8"], returns: "void" },
    getKittyKeyboardFlags: { args: ["ptr"], returns: "u8" },
    enableKittyKeyboard: { args: ["ptr", "u8"], returns: "void" },
    disableKittyKeyboard: { args: ["ptr"], returns: "void" },
    enableMouse: { args: ["ptr", "bool"], returns: "void" },
    disableMouse: { args: ["ptr"], returns: "void" },
    setTerminalTitle: { args: ["ptr", "ptr", "usize"], returns: "void" },
    setTerminalEnvVar: { args: ["ptr", "ptr", "usize", "ptr", "usize"], returns: "bool" },
    setupTerminal: { args: ["ptr", "bool"], returns: "void" },
    suspendRenderer: { args: ["ptr"], returns: "void" },
    resumeRenderer: { args: ["ptr"], returns: "void" },
    restoreTerminalModes: { args: ["ptr"], returns: "void" },
    clearTerminal: { args: ["ptr"], returns: "void" },
    writeOut: { args: ["ptr", "ptr", "u64"], returns: "void" },
    copyToClipboardOSC52: { args: ["ptr", "u8", "ptr", "usize"], returns: "bool" },
    clearClipboardOSC52: { args: ["ptr", "u8"], returns: "bool" },
    queryPixelResolution: { args: ["ptr"], returns: "void" },
    getTerminalCapabilities: { args: ["ptr", "ptr"], returns: "void" },
    processCapabilityResponse: { args: ["ptr", "ptr", "usize"], returns: "void" },
    setHyperlinksCapability: { args: ["ptr", "bool"], returns: "void" },
  }).symbols

  const renderer = lib.createRenderer(10, 5, true, false)
  expect(renderer).not.toBeNull()

  lib.setCursorPosition(renderer, 4, 2, true)
  lib.setCursorColor(renderer, new Float32Array([1, 0, 0, 1]))
  const cursorStyle = CursorStyleOptionsStruct.pack({ style: 1, blinking: 1 })
  lib.setCursorStyleOptions(renderer, ptr(cursorStyle))

  const cursorStateBuffer = new ArrayBuffer(CursorStateStruct.size)
  lib.getCursorState(renderer, ptr(cursorStateBuffer))
  const cursorState = CursorStateStruct.unpack(cursorStateBuffer)
  expect(cursorState.x).toBe(4)
  expect(cursorState.y).toBe(2)
  expect(cursorState.visible).toBe(true)
  expect(cursorState.style).toBe(1)
  expect(cursorState.blinking).toBe(true)

  lib.setKittyKeyboardFlags(renderer, 0b1010)
  expect(lib.getKittyKeyboardFlags(renderer)).toBe(0b1010)
  lib.enableKittyKeyboard(renderer, 0b1111)
  expect(lib.getKittyKeyboardFlags(renderer)).toBe(0b1111)
  lib.disableKittyKeyboard(renderer)
  expect(lib.getKittyKeyboardFlags(renderer)).toBe(0)

  const title = new TextEncoder().encode("title")
  lib.setTerminalTitle(renderer, title, title.length)
  const key = new TextEncoder().encode("TERM")
  const value = new TextEncoder().encode("xterm")
  expect(lib.setTerminalEnvVar(renderer, key, key.length, value, value.length)).toBe(true)

  expect(lib.copyToClipboardOSC52(renderer, 1, new TextEncoder().encode("clip"), 4)).toBe(true)
  expect(lib.clearClipboardOSC52(renderer, 1)).toBe(true)
  lib.setupTerminal(renderer, true)
  lib.suspendRenderer(renderer)
  lib.resumeRenderer(renderer)
  lib.restoreTerminalModes(renderer)
  lib.clearTerminal(renderer)
  lib.writeOut(renderer, new TextEncoder().encode("hello"), 5)
  lib.queryPixelResolution(renderer)
  lib.processCapabilityResponse(renderer, new TextEncoder().encode("kitty sixel sync"), 16)
  lib.setHyperlinksCapability(renderer, true)

  const capsBuffer = new ArrayBuffer(TerminalCapabilitiesStruct.size)
  lib.getTerminalCapabilities(renderer, ptr(capsBuffer))
  const caps = TerminalCapabilitiesStruct.unpack(capsBuffer)
  expect(caps.kitty_keyboard).toBe(true)
  expect(caps.kitty_graphics).toBe(true)
  expect(caps.sixel).toBe(true)
  expect(caps.sync).toBe(true)
  expect(caps.sgr_pixels).toBe(true)
  expect(caps.hyperlinks).toBe(true)

  lib.destroyRenderer(renderer)
})
