import { expect, test } from "bun:test"

import { createAbiManifest, readAbiManifest } from "../../scripts/native-abi"

test("native ABI manifest stays aligned with native exports and Bun loader symbols", () => {
  const current = createAbiManifest()
  const recorded = readAbiManifest()

  expect(current).toEqual(recorded)
  expect(current.missingFromNativeExports).toHaveLength(0)
  expect(current.groups.text.missingFromNativeExports).toHaveLength(0)
  expect(current.groups.nativeSpanFeed.missingFromNativeExports).toHaveLength(0)
})
