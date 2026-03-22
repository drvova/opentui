import { execFileSync, spawnSync } from "node:child_process"
import { dirname, join } from "node:path"
import { fileURLToPath } from "node:url"

import { dlopen, ptr } from "bun:ffi"
import { expect, test } from "bun:test"

import { SceneLayoutStruct, SceneStyleStruct } from "../native-structs.js"

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
const runRustSceneGraphSmoke = cargoAvailable ? test : test.skip

runRustSceneGraphSmoke("Rust scene graph APIs support node lifecycle and layout", () => {
  execFileSync("cargo", ["build"], { cwd: nativeDir, stdio: "inherit" })

  const lib = dlopen(rustLibPath, {
    createSceneNode: { args: [], returns: "u64" },
    destroySceneNode: { args: ["u64"], returns: "bool" },
    sceneNodeAppendChild: { args: ["u64", "u64"], returns: "bool" },
    sceneNodeInsertBefore: { args: ["u64", "u64", "u64"], returns: "bool" },
    sceneNodeRemoveChild: { args: ["u64", "u64"], returns: "bool" },
    sceneNodeSetStyle: { args: ["u64", "ptr"], returns: "bool" },
    sceneNodeCalculateLayout: { args: ["u64", "f32", "f32"], returns: "bool" },
    sceneNodeGetLayout: { args: ["u64", "ptr"], returns: "bool" },
    sceneNodeGetChildCount: { args: ["u64"], returns: "usize" },
  }).symbols

  const root = lib.createSceneNode()
  const first = lib.createSceneNode()
  const second = lib.createSceneNode()

  expect(typeof root === "bigint" ? root : BigInt(root)).toBeGreaterThan(0n)

  const rootStyle = SceneStyleStruct.pack({ width: 120, height: 40, widthUnit: 0, heightUnit: 0 })
  const childStyle = SceneStyleStruct.pack({ width: 30, height: 10, widthUnit: 0, heightUnit: 0 })

  expect(lib.sceneNodeSetStyle(root, ptr(rootStyle))).toBe(true)
  expect(lib.sceneNodeSetStyle(first, ptr(childStyle))).toBe(true)
  expect(lib.sceneNodeSetStyle(second, ptr(childStyle))).toBe(true)
  expect(lib.sceneNodeAppendChild(root, first)).toBe(true)
  expect(lib.sceneNodeInsertBefore(root, second, first)).toBe(true)
  expect(Number(lib.sceneNodeGetChildCount(root))).toBe(2)
  expect(lib.sceneNodeCalculateLayout(root, 120, 40)).toBe(true)

  const layoutBuffer = new ArrayBuffer(SceneLayoutStruct.size)
  expect(lib.sceneNodeGetLayout(root, ptr(layoutBuffer))).toBe(true)
  const layout = SceneLayoutStruct.unpack(layoutBuffer)
  expect(layout.width).toBeGreaterThanOrEqual(0)
  expect(layout.height).toBeGreaterThanOrEqual(0)

  expect(lib.sceneNodeRemoveChild(root, second)).toBe(true)
  expect(Number(lib.sceneNodeGetChildCount(root))).toBe(1)

  expect(lib.destroySceneNode(second)).toBe(true)
  expect(lib.destroySceneNode(first)).toBe(true)
  expect(lib.destroySceneNode(root)).toBe(true)
})
