import { spawnSync, type SpawnSyncReturns } from "node:child_process"
import { copyFileSync, existsSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from "fs"
import { dirname, join, resolve } from "path"
import { fileURLToPath } from "url"
import process from "process"
import path from "path"

interface Variant {
  platform: string
  arch: string
  rustTarget: string
  libraryFileName: string
}

interface PackageJson {
  name: string
  version: string
  license?: string
  repository?: any
  description?: string
  homepage?: string
  author?: string
  bugs?: any
  keywords?: string[]
  module?: string
  main?: string
  types?: string
  type?: string
  dependencies?: Record<string, string>
  devDependencies?: Record<string, string>
  optionalDependencies?: Record<string, string>
  peerDependencies?: Record<string, string>
}

const __filename = fileURLToPath(import.meta.url)
const __dirname = dirname(__filename)
const rootDir = resolve(__dirname, "..")
const licensePath = path.resolve(__dirname, "../../../LICENSE")
const packageJson: PackageJson = JSON.parse(readFileSync(join(rootDir, "package.json"), "utf8"))

const args = process.argv.slice(2)
const buildLib = args.find((arg) => arg === "--lib")
const buildNative = args.find((arg) => arg === "--native")
const isDev = args.includes("--dev")
const buildAll = args.includes("--all")
const explicitVariants = readFlagValues("--variant")
const explicitPlatform = readFlagValue("--platform")
const explicitArch = readFlagValue("--arch")
const explicitRustTarget = readFlagValue("--target")

const variants: Variant[] = [
  { platform: "darwin", arch: "x64", rustTarget: "x86_64-apple-darwin", libraryFileName: "libopentui.dylib" },
  { platform: "darwin", arch: "arm64", rustTarget: "aarch64-apple-darwin", libraryFileName: "libopentui.dylib" },
  { platform: "linux", arch: "x64", rustTarget: "x86_64-unknown-linux-gnu", libraryFileName: "libopentui.so" },
  { platform: "linux", arch: "arm64", rustTarget: "aarch64-unknown-linux-gnu", libraryFileName: "libopentui.so" },
  { platform: "win32", arch: "x64", rustTarget: "x86_64-pc-windows-msvc", libraryFileName: "opentui.dll" },
  { platform: "win32", arch: "arm64", rustTarget: "aarch64-pc-windows-msvc", libraryFileName: "opentui.dll" },
]

if (!buildLib && !buildNative) {
  console.error("Error: Please specify --lib, --native, or both")
  process.exit(1)
}

function readFlagValue(flag: string): string | null {
  const direct = args.find((arg) => arg.startsWith(`${flag}=`))
  if (direct) {
    return direct.slice(flag.length + 1)
  }

  const index = args.indexOf(flag)
  if (index === -1 || index === args.length - 1) {
    return null
  }

  return args[index + 1]
}

function readFlagValues(flag: string): string[] {
  const values: string[] = []

  args.forEach((arg, index) => {
    if (arg.startsWith(`${flag}=`)) {
      values.push(arg.slice(flag.length + 1))
      return
    }

    if (arg === flag && index < args.length - 1) {
      values.push(args[index + 1])
    }
  })

  return values
}

const replaceLinks = (text: string): string => {
  return packageJson.homepage
    ? text.replace(
        /(\[.*?\]\()(\.\/.*?\))/g,
        (_, p1: string, p2: string) => `${p1}${packageJson.homepage}/blob/HEAD/${p2.replace("./", "")}`,
      )
    : text
}

const requiredFields: (keyof PackageJson)[] = ["name", "version", "license", "repository", "description"]
const missingRequired = requiredFields.filter((field) => !packageJson[field])
if (missingRequired.length > 0) {
  console.error(`Error: Missing required fields in package.json: ${missingRequired.join(", ")}`)
  process.exit(1)
}

const hostPlatform = process.platform
const hostArch = process.arch === "arm64" ? "arm64" : process.arch === "x64" ? "x64" : process.arch
const hostVariant = variants.find((variant) => variant.platform === hostPlatform && variant.arch === hostArch)

function resolveVariants(): Variant[] {
  if (explicitVariants.length > 0 && (explicitPlatform || explicitArch || explicitRustTarget)) {
    console.error("Error: --variant cannot be combined with --platform, --arch, or --target.")
    process.exit(1)
  }

  if (explicitVariants.length > 0) {
    return explicitVariants.map((value) => {
      const variant = variants.find((candidate) => `${candidate.platform}-${candidate.arch}` === value)
      if (!variant) {
        console.error(`Error: Unsupported native variant ${value}.`)
        process.exit(1)
      }
      return variant
    })
  }

  if (buildAll) {
    return variants.filter((candidate) => candidate.platform === hostPlatform)
  }

  if (explicitPlatform || explicitArch) {
    if (!explicitPlatform || !explicitArch) {
      console.error("Error: --platform and --arch must be provided together.")
      process.exit(1)
    }

    const variant = variants.find((candidate) => candidate.platform === explicitPlatform && candidate.arch === explicitArch)
    if (!variant) {
      console.error(`Error: Unsupported native variant ${explicitPlatform}-${explicitArch}.`)
      process.exit(1)
    }

    if (explicitRustTarget && explicitRustTarget !== variant.rustTarget) {
      console.error(`Error: --target ${explicitRustTarget} does not match ${explicitPlatform}-${explicitArch} (${variant.rustTarget}).`)
      process.exit(1)
    }

    return [variant]
  }

  if (explicitRustTarget) {
    const variant = variants.find((candidate) => candidate.rustTarget === explicitRustTarget)
    if (!variant) {
      console.error(`Error: Unsupported Rust target ${explicitRustTarget}.`)
      process.exit(1)
    }
    return [variant]
  }

  if (!hostVariant) {
    console.error(`Error: Unsupported host platform ${hostPlatform}-${hostArch}.`)
    process.exit(1)
  }

  return [hostVariant]
}

if (buildNative) {
  const requestedVariants = resolveVariants()
  const profileDir = isDev ? "debug" : "release"
  const nativeDirRoot = join(rootDir, "node_modules", "@opentui")

  const runOrFail = (command: string, commandArgs: string[], env?: NodeJS.ProcessEnv): void => {
    const result: SpawnSyncReturns<Buffer> = spawnSync(command, commandArgs, {
      cwd: join(rootDir, "native"),
      stdio: "inherit",
      env: env ?? process.env,
    })

    if (result.error) {
      console.error(`Error: Failed to run ${command}.`)
      process.exit(1)
    }

    if (result.status !== 0) {
      console.error(`Error: Command failed: ${command} ${commandArgs.join(" ")}`)
      process.exit(1)
    }
  }

  for (const variant of requestedVariants) {
    const rustupArgs = ["target", "add", variant.rustTarget]
    runOrFail("rustup", rustupArgs)

    const cargoArgs = variant.platform === "win32" && hostPlatform !== "win32" ? ["xwin", "build"] : ["build"]
    if (!isDev) {
      cargoArgs.push("--release")
    }
    cargoArgs.push("--target", variant.rustTarget)

    const buildEnv = { ...process.env }
    if (variant.rustTarget === "aarch64-unknown-linux-gnu" && hostPlatform === "linux") {
      buildEnv.CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER =
        buildEnv.CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER || "aarch64-linux-gnu-gcc"
    }

    console.log(`Building Rust native ${isDev ? "dev" : "prod"} library for ${variant.platform}-${variant.arch}...`)
    runOrFail("cargo", cargoArgs, buildEnv)

    const nativeName = `${packageJson.name}-${variant.platform}-${variant.arch}`
    const nativeDir = join(nativeDirRoot, nativeName)
    const src = join(rootDir, "native", "target", variant.rustTarget, profileDir, variant.libraryFileName)

    if (!existsSync(src)) {
      console.error(`Error: Expected native library ${src} was not produced by Cargo.`)
      process.exit(1)
    }

    rmSync(nativeDir, { recursive: true, force: true })
    mkdirSync(nativeDir, { recursive: true })
    copyFileSync(src, join(nativeDir, variant.libraryFileName))

    const indexTsContent = `const module = await import("./${variant.libraryFileName}", { with: { type: "file" } })
const path = module.default
export default path;
`
    writeFileSync(join(nativeDir, "index.ts"), indexTsContent)

    writeFileSync(
      join(nativeDir, "package.json"),
      JSON.stringify(
        {
          name: nativeName,
          version: packageJson.version,
          description: `Prebuilt ${variant.platform}-${variant.arch} binaries for ${packageJson.name}`,
          main: "index.ts",
          types: "index.ts",
          license: packageJson.license,
          author: packageJson.author,
          homepage: packageJson.homepage,
          repository: packageJson.repository,
          bugs: packageJson.bugs,
          keywords: [...(packageJson.keywords ?? []), "prebuild", "prebuilt"],
          os: [variant.platform],
          cpu: [variant.arch],
        },
        null,
        2,
      ),
    )

    writeFileSync(
      join(nativeDir, "README.md"),
      replaceLinks(`## ${nativeName}\n\n> Prebuilt ${variant.platform}-${variant.arch} binaries for \`${packageJson.name}\`.`),
    )

    if (existsSync(licensePath)) copyFileSync(licensePath, join(nativeDir, "LICENSE"))
    console.log("Built:", nativeName)
  }
}

if (buildLib) {
  console.log("Building library...")

  const distDir = join(rootDir, "dist")
  rmSync(distDir, { recursive: true, force: true })
  mkdirSync(distDir, { recursive: true })

  const externalDeps: string[] = [
    ...Object.keys(packageJson.optionalDependencies || {}),
    ...Object.keys(packageJson.peerDependencies || {}),
  ]

  // Build main entry point
  if (!packageJson.module) {
    console.error("Error: 'module' field not found in package.json")
    process.exit(1)
  }

  const entryPoints: string[] = [
    packageJson.module,
    "src/3d.ts",
    "src/testing.ts",
    "src/runtime-plugin.ts",
    "src/runtime-plugin-support.ts",
  ]

  // Build main entry points with code splitting
  // External patterns to prevent bundling tree-sitter assets and default-parsers
  // to allow standalone executables to work
  const externalPatterns = [
    ...externalDeps,
    "*.wasm",
    "*.scm",
    "./lib/tree-sitter/assets/*",
    "./lib/tree-sitter/default-parsers",
    "./lib/tree-sitter/default-parsers.ts",
  ]

  spawnSync(
    "bun",
    [
      "build",
      "--target=bun",
      "--splitting",
      "--outdir=dist",
      "--sourcemap",
      ...externalPatterns.flatMap((dep) => ["--external", dep]),
      ...entryPoints,
    ],
    {
      cwd: rootDir,
      stdio: "inherit",
    },
  )

  // Build parser worker as standalone bundle (no splitting) so it can be loaded as a Worker
  // Make web-tree-sitter external so it loads from node_modules with its WASM file
  spawnSync(
    "bun",
    [
      "build",
      "--target=bun",
      "--outdir=dist",
      "--sourcemap",
      ...externalDeps.flatMap((dep) => ["--external", dep]),
      "--external",
      "web-tree-sitter",
      "src/lib/tree-sitter/parser.worker.ts",
    ],
    {
      cwd: rootDir,
      stdio: "inherit",
    },
  )

  // Post-process to fix Bun's duplicate export issue
  // See: https://github.com/oven-sh/bun/issues/5344
  // and: https://github.com/oven-sh/bun/issues/10631
  console.log("Post-processing bundled files to fix duplicate exports...")
  const bundledFiles = [
    "dist/index.js",
    "dist/3d.js",
    "dist/testing.js",
    "dist/runtime-plugin.js",
    "dist/runtime-plugin-support.js",
    "dist/lib/tree-sitter/parser.worker.js",
  ]
  for (const filePath of bundledFiles) {
    const fullPath = join(rootDir, filePath)
    if (existsSync(fullPath)) {
      let content = readFileSync(fullPath, "utf8")
      const helperExportPattern = /^export\s*\{([^}]*(?:__toESM|__commonJS|__export|__require)[^}]*)\};\s*$/gm

      let modified = false
      content = content.replace(helperExportPattern, (match, exports) => {
        const exportsList = exports.split(",").map((e: string) => e.trim())
        const helpers = ["__toESM", "__commonJS", "__export", "__require"]
        const nonHelpers = exportsList.filter((e: string) => !helpers.includes(e))

        if (nonHelpers.length > 0) {
          modified = true
          const helperExports = exportsList.filter((e: string) => helpers.includes(e))
          return `export { ${helperExports.join(", ")} };`
        }
        return match
      })

      if (modified) {
        writeFileSync(fullPath, content)
        console.log(`  Fixed duplicate exports in ${filePath}`)
      }
    }
  }

  console.log("Generating TypeScript declarations...")

  const tsconfigBuildPath = join(rootDir, "tsconfig.build.json")

  const tscResult: SpawnSyncReturns<Buffer> = spawnSync("bunx", ["tsc", "-p", tsconfigBuildPath], {
    cwd: rootDir,
    stdio: "inherit",
  })

  if (tscResult.status !== 0) {
    console.error("Error: TypeScript declaration generation failed")
    process.exit(1)
  } else {
    console.log("TypeScript declarations generated")
  }

  const treeSitterSrcDir = join(rootDir, "src", "lib", "tree-sitter")

  const copyAssets = (src: string, dest: string) => {
    mkdirSync(dest, { recursive: true })
    const entries = readdirSync(src, { withFileTypes: true })
    for (const entry of entries) {
      const srcPath = join(src, entry.name)
      const destPath = join(dest, entry.name)
      if (entry.isDirectory()) {
        copyAssets(srcPath, destPath)
      } else if (entry.isFile() && (entry.name.endsWith(".wasm") || entry.name.endsWith(".scm"))) {
        copyFileSync(srcPath, destPath)
      }
    }
  }

  copyAssets(join(treeSitterSrcDir, "assets"), join(distDir, "assets"))
  console.log("  Copied tree-sitter assets (*.wasm, *.scm) to dist/assets/")

  // Configure exports for multiple entry points
  const exports = {
    ".": {
      import: "./index.js",
      require: "./index.js",
      types: "./index.d.ts",
    },
    "./3d": {
      import: "./3d.js",
      require: "./3d.js",
      types: "./3d.d.ts",
    },
    "./testing": {
      import: "./testing.js",
      require: "./testing.js",
      types: "./testing.d.ts",
    },
    "./runtime-plugin": {
      import: "./runtime-plugin.js",
      require: "./runtime-plugin.js",
      types: "./runtime-plugin.d.ts",
    },
    "./runtime-plugin-support": {
      import: "./runtime-plugin-support.js",
      require: "./runtime-plugin-support.js",
      types: "./runtime-plugin-support.d.ts",
    },
    "./parser.worker": {
      import: "./lib/tree-sitter/parser.worker.js",
      require: "./lib/tree-sitter/parser.worker.js",
      types: "./lib/tree-sitter/parser.worker.d.ts",
    },
  }

  const optionalDeps: Record<string, string> = Object.fromEntries(
    variants.map(({ platform, arch }) => [`${packageJson.name}-${platform}-${arch}`, packageJson.version]),
  )

  writeFileSync(
    join(distDir, "package.json"),
    JSON.stringify(
      {
        name: packageJson.name,
        module: "index.js",
        main: "index.js",
        types: "index.d.ts",
        type: packageJson.type,
        version: packageJson.version,
        description: packageJson.description,
        keywords: packageJson.keywords,
        license: packageJson.license,
        author: packageJson.author,
        homepage: packageJson.homepage,
        repository: packageJson.repository,
        bugs: packageJson.bugs,
        exports,
        dependencies: packageJson.dependencies,
        devDependencies: packageJson.devDependencies,
        peerDependencies: packageJson.peerDependencies,
        optionalDependencies: {
          ...packageJson.optionalDependencies,
          ...optionalDeps,
        },
      },
      null,
      2,
    ),
  )

  writeFileSync(join(distDir, "README.md"), replaceLinks(readFileSync(join(rootDir, "README.md"), "utf8")))
  if (existsSync(licensePath)) copyFileSync(licensePath, join(distDir, "LICENSE"))

  console.log("Library built at:", distDir)
}
