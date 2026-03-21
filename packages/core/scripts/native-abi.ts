import { createHash } from "node:crypto"
import { existsSync, readdirSync, readFileSync, writeFileSync } from "node:fs"
import { dirname, join, resolve } from "node:path"
import { fileURLToPath } from "node:url"
import process from "node:process"
import { nativeSpanFeedSymbols, textRuntimeSymbols } from "../src/native-symbols.js"

interface AbiManifestGroup {
  symbolCount: number
  symbolHash: string
  bunLoaderSymbols: string[]
  missingFromNativeExports: string[]
}

export interface AbiManifest {
  schemaVersion: 1
  symbolCount: number
  symbolHash: string
  nativeExports: string[]
  bunLoaderSymbols: string[]
  missingFromNativeExports: string[]
  unloadedNativeExports: string[]
  groups: {
    core: AbiManifestGroup
    text: AbiManifestGroup
    nativeSpanFeed: AbiManifestGroup
  }
}

const __filename = fileURLToPath(import.meta.url)
const __dirname = dirname(__filename)
const rootDir = resolve(__dirname, "..")
const zigSourceDir = join(rootDir, "src", "zig")
const zigTsPath = join(rootDir, "src", "zig.ts")
export const abiManifestPath = join(rootDir, "native", "ffi-manifest.json")
const textRuntimeSymbolNames = sortUnique(Object.keys(textRuntimeSymbols))
const nativeSpanFeedSymbolNames = sortUnique(Object.keys(nativeSpanFeedSymbols))

function sortUnique(values: Iterable<string>): string[] {
  return [...new Set(values)].sort((left, right) => left.localeCompare(right))
}

function readUtf8(path: string): string {
  return readFileSync(path, "utf8")
}

function listZigFiles(directory: string): string[] {
  const entries = readdirSync(directory, { withFileTypes: true })
  const files: string[] = []

  for (const entry of entries) {
    const path = join(directory, entry.name)
    if (entry.isDirectory()) {
      files.push(...listZigFiles(path))
      continue
    }

    if (entry.isFile() && entry.name.endsWith(".zig")) {
      files.push(path)
    }
  }

  return files
}

function extractBlock(source: string, anchor: string): string {
  const anchorIndex = source.indexOf(anchor)
  if (anchorIndex === -1) {
    throw new Error(`Unable to find anchor '${anchor}'`)
  }

  const startIndex = source.indexOf("{", anchorIndex)
  if (startIndex === -1) {
    throw new Error(`Unable to find object block for anchor '${anchor}'`)
  }

  let depth = 0
  for (let index = startIndex; index < source.length; index += 1) {
    const char = source[index]
    if (char === "{") {
      depth += 1
      continue
    }

    if (char !== "}") {
      continue
    }

    depth -= 1
    if (depth === 0) {
      return source.slice(startIndex + 1, index)
    }
  }

  throw new Error(`Unterminated object block for anchor '${anchor}'`)
}

export function extractNativeExports(source: string): string[] {
  const exports = source.matchAll(/^(?:pub )?export fn (\w+)\(/gm)
  return sortUnique([...exports].map((match) => match[1]))
}

export function extractInlineLoaderSymbols(source: string): string[] {
  const match = source.match(/const \w+Library = dlopen\(resolvedLibPath, \{/)
  if (!match || match.index === undefined) {
    throw new Error("Unable to find primary dlopen block in zig.ts")
  }

  const block = extractBlock(source, match[0])
  const symbols = block.matchAll(/^\s{4}([A-Za-z0-9_]+): \{$/gm)
  return sortUnique([...symbols].map((match) => match[1]))
}

function createGroupManifest(bunLoaderSymbols: string[], nativeExports: string[]): AbiManifestGroup {
  return {
    symbolCount: bunLoaderSymbols.length,
    symbolHash: createHash("sha256").update(JSON.stringify(bunLoaderSymbols)).digest("hex"),
    bunLoaderSymbols,
    missingFromNativeExports: bunLoaderSymbols.filter((symbol) => !nativeExports.includes(symbol)),
  }
}

export function createAbiManifest(): AbiManifest {
  const nativeExports = sortUnique(listZigFiles(zigSourceDir).flatMap((file) => extractNativeExports(readUtf8(file))))
  const coreBunLoaderSymbols = extractInlineLoaderSymbols(readUtf8(zigTsPath))
  const bunLoaderSymbols = sortUnique([...coreBunLoaderSymbols, ...textRuntimeSymbolNames, ...nativeSpanFeedSymbolNames])

  const missingFromNativeExports = bunLoaderSymbols.filter((symbol) => !nativeExports.includes(symbol))
  const unloadedNativeExports = nativeExports.filter((symbol) => !bunLoaderSymbols.includes(symbol))
  const symbolHash = createHash("sha256").update(JSON.stringify(bunLoaderSymbols)).digest("hex")

  return {
    schemaVersion: 1,
    symbolCount: bunLoaderSymbols.length,
    symbolHash,
    nativeExports,
    bunLoaderSymbols,
    missingFromNativeExports,
    unloadedNativeExports,
    groups: {
      core: createGroupManifest(coreBunLoaderSymbols, nativeExports),
      text: createGroupManifest(textRuntimeSymbolNames, nativeExports),
      nativeSpanFeed: createGroupManifest(nativeSpanFeedSymbolNames, nativeExports),
    },
  }
}

export function readAbiManifest(): AbiManifest {
  if (!existsSync(abiManifestPath)) {
    throw new Error(`ABI manifest not found at ${abiManifestPath}. Run 'bun scripts/native-abi.ts --write'.`)
  }

  return JSON.parse(readUtf8(abiManifestPath)) as AbiManifest
}

function formatManifest(manifest: AbiManifest): string {
  return `${JSON.stringify(manifest, null, 2)}\n`
}

function writeAbiManifest(): void {
  writeFileSync(abiManifestPath, formatManifest(createAbiManifest()))
}

function checkAbiManifest(): void {
  const current = formatManifest(createAbiManifest())
  const existing = existsSync(abiManifestPath) ? readUtf8(abiManifestPath) : ""

  if (current !== existing) {
    throw new Error("ABI manifest is out of date. Run 'bun scripts/native-abi.ts --write'.")
  }
}

function main(): void {
  if (process.argv.includes("--write")) {
    writeAbiManifest()
    return
  }

  if (process.argv.includes("--check")) {
    checkAbiManifest()
    return
  }

  process.stdout.write(formatManifest(createAbiManifest()))
}

if (import.meta.main) {
  main()
}
