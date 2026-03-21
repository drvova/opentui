import { expect, test } from "bun:test"

import { createAbiManifest, readAbiManifest } from "../../scripts/native-abi"

test("native ABI manifest stays aligned with Zig exports and Bun loader symbols", () => {
  const current = createAbiManifest()
  const recorded = readAbiManifest()

  expect(current).toEqual(recorded)
  expect(current.missingFromNativeExports).toHaveLength(0)
})
