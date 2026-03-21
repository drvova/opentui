import { execFileSync, spawnSync } from "node:child_process"
import { dirname, join } from "node:path"
import process from "node:process"
import { fileURLToPath } from "node:url"
import { test, expect } from "bun:test"

const __filename = fileURLToPath(import.meta.url)
const __dirname = dirname(__filename)
const packageRoot = join(__dirname, "..", "..")
const nativeDir = join(packageRoot, "native")
const rustLibPath = join(nativeDir, "target", "debug", process.platform === "darwin" ? "libopentui.dylib" : process.platform === "win32" ? "opentui.dll" : "libopentui.so")

const cargoAvailable = spawnSync("cargo", ["--version"], { cwd: nativeDir, stdio: "ignore" }).status === 0

const runOverlaySmoke = cargoAvailable ? test : test.skip

runOverlaySmoke("SyntaxStyle wrapper works through the Rust overlay loader path", () => {
  execFileSync("cargo", ["build"], { cwd: nativeDir, stdio: "inherit" })

  const code = `
    import { SyntaxStyle } from "./src/syntax-style.ts"
    import { RGBA } from "./src/lib/RGBA.ts"
    const style = SyntaxStyle.create()
    const id = style.registerStyle("keyword", { fg: RGBA.fromValues(1, 0, 0, 1), bold: true })
    const resolved = style.resolveStyleId("keyword")
    const count = style.getStyleCount()
    style.destroy()
    if (id === 0 || resolved !== id || count !== 1) {
      throw new Error(\`unexpected \${id} \${resolved} \${count}\`)
    }
  `

  execFileSync(process.execPath, ["--eval", code], {
    cwd: packageRoot,
    env: {
      ...process.env,
      OTUI_RUST_LIB_PATH: rustLibPath,
      OTUI_RUST_SYMBOL_GROUPS: "syntaxStyle",
    },
    stdio: "inherit",
  })

  expect(true).toBe(true)
})
