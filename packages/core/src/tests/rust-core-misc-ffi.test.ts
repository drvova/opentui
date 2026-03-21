import { execFileSync, spawnSync } from "node:child_process"
import { dirname, join } from "node:path"
import { fileURLToPath } from "node:url"

import { dlopen, ptr, toArrayBuffer } from "bun:ffi"
import { expect, test } from "bun:test"

import { AllocatorStatsStruct, BuildOptionsStruct, EncodedCharStruct } from "../native-structs.js"

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
const runRustCoreMiscSmoke = cargoAvailable ? test : test.skip

runRustCoreMiscSmoke("Rust core misc helpers expose link, unicode, allocator, and callback state", () => {
  execFileSync("cargo", ["build"], { cwd: nativeDir, stdio: "inherit" })

  const lib = dlopen(rustLibPath, {
    setLogCallback: { args: ["ptr"], returns: "void" },
    setEventCallback: { args: ["ptr"], returns: "void" },
    getArenaAllocatedBytes: { args: [], returns: "usize" },
    getBuildOptions: { args: ["ptr"], returns: "void" },
    getAllocatorStats: { args: ["ptr"], returns: "void" },
    linkAlloc: { args: ["ptr", "usize"], returns: "u32" },
    linkGetUrl: { args: ["u32", "ptr", "usize"], returns: "usize" },
    attributesWithLink: { args: ["u32", "u32"], returns: "u32" },
    attributesGetLinkId: { args: ["u32"], returns: "u32" },
    encodeUnicode: { args: ["ptr", "usize", "ptr", "ptr", "u8"], returns: "bool" },
    freeUnicode: { args: ["ptr", "usize"], returns: "void" },
  }).symbols

  lib.setLogCallback(null)
  lib.setEventCallback(null)

  expect(Number(lib.getArenaAllocatedBytes())).toBeGreaterThanOrEqual(0)

  const buildOptionsBuffer = new ArrayBuffer(BuildOptionsStruct.size)
  lib.getBuildOptions(ptr(buildOptionsBuffer))
  const buildOptions = BuildOptionsStruct.unpack(buildOptionsBuffer)
  expect(typeof buildOptions.gpaSafeStats).toBe("boolean")
  expect(typeof buildOptions.gpaMemoryLimitTracking).toBe("boolean")

  const allocatorBuffer = new ArrayBuffer(AllocatorStatsStruct.size)
  lib.getAllocatorStats(ptr(allocatorBuffer))
  const allocatorStats = AllocatorStatsStruct.unpack(allocatorBuffer)
  expect(Number(allocatorStats.activeAllocations)).toBeGreaterThanOrEqual(0)

  const url = new TextEncoder().encode("https://example.com")
  const linkId = lib.linkAlloc(url, url.length)
  expect(linkId).toBeGreaterThan(0)
  const linkOut = new Uint8Array(64)
  const linkLen = Number(lib.linkGetUrl(linkId, linkOut, linkOut.length))
  expect(new TextDecoder().decode(linkOut.slice(0, linkLen))).toBe("https://example.com")

  const attr = lib.attributesWithLink(7, linkId)
  expect(lib.attributesGetLinkId(attr)).toBe(linkId)

  const text = new TextEncoder().encode("A👋")
  const outPtrBuffer = new BigUint64Array(1)
  const outLenBuffer = new BigUint64Array(1)
  expect(lib.encodeUnicode(text, text.length, outPtrBuffer, outLenBuffer, 1)).toBe(true)
  const encodedPtr = Number(outPtrBuffer[0])
  const encodedLen = Number(outLenBuffer[0])
  const raw = toArrayBuffer(encodedPtr, 0, encodedLen * EncodedCharStruct.size)
  const encoded = EncodedCharStruct.unpackList(raw, encodedLen)
  expect(encoded[0]).toEqual({ width: 1, char: "A".codePointAt(0)! })
  expect(encoded[1].width).toBe(2)
  expect(encoded[1].char).toBeGreaterThan(0x80000000)
  lib.freeUnicode(encodedPtr as any, encodedLen)
})
