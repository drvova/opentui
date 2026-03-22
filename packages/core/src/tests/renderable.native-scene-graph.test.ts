import { expect, test } from "bun:test"

import { BoxRenderable } from "../renderables/Box.js"
import { createTestRenderer } from "../testing/test-renderer.js"

function expectNativeLayoutParity(renderer: { sceneNodeGetLayout: (handle: bigint | number) => any }, renderable: BoxRenderable) {
  const handle = (renderable as any).sceneNodeHandle
  expect(handle).toBeTruthy()

  const nativeLayout = renderer.sceneNodeGetLayout(handle)
  const yogaLayout = renderable.getLayoutNode().getComputedLayout()

  expect(nativeLayout).toEqual({
    left: yogaLayout.left,
    top: yogaLayout.top,
    width: yogaLayout.width,
    height: yogaLayout.height,
  })
}

test("Renderable mirrors tree mutations into the native scene graph", async () => {
  const { renderer, renderOnce } = await createTestRenderer({ width: 80, height: 24 })

  const parent = new BoxRenderable(renderer, { width: 20, height: 10 })
  const child = new BoxRenderable(renderer, { width: 5, height: 2 })

  renderer.root.add(parent)
  parent.add(child)
  await renderOnce()

  const parentHandle = (parent as any).sceneNodeHandle
  const childHandle = (child as any).sceneNodeHandle

  expect(parentHandle).toBeTruthy()
  expect(childHandle).toBeTruthy()
  expect(renderer.sceneNodeGetChildCount(parentHandle)).toBe(1)

  parent.remove(child.id)
  await renderOnce()

  expect(renderer.sceneNodeGetChildCount(parentHandle)).toBe(0)

  renderer.destroy()
})

test("Renderable child reads prefer native scene graph order", async () => {
  const { renderer, renderOnce } = await createTestRenderer({ width: 80, height: 24 })

  const parent = new BoxRenderable(renderer, { width: 20, height: 10 })
  const first = new BoxRenderable(renderer, { id: "first", width: 5, height: 2 })
  const second = new BoxRenderable(renderer, { id: "second", width: 5, height: 2 })
  const inserted = new BoxRenderable(renderer, { id: "inserted", width: 5, height: 2 })

  renderer.root.add(parent)
  parent.add(first)
  parent.add(second)
  parent.insertBefore(inserted, second)
  await renderOnce()

  expect(parent.getChildren().map((child) => child.id)).toEqual(["first", "inserted", "second"])
  expect(parent.getChildrenCount()).toBe(3)

  renderer.destroy()
})

test("native scene graph matches Yoga layout for spacing and border styles", async () => {
  const { renderer, renderOnce } = await createTestRenderer({ width: 80, height: 24 })

  const container = new BoxRenderable(renderer, {
    width: 30,
    height: 10,
    border: true,
    padding: 1,
    gap: 2,
    flexDirection: "row",
  })
  const first = new BoxRenderable(renderer, { width: 5, height: 2 })
  const second = new BoxRenderable(renderer, { width: 7, height: 2, marginTop: 1 })

  renderer.root.add(container)
  container.add(first)
  container.add(second)
  await renderOnce()

  expectNativeLayoutParity(renderer, container)
  expectNativeLayoutParity(renderer, first)
  expectNativeLayoutParity(renderer, second)

  renderer.destroy()
})

test("native scene graph matches Yoga layout for wrap and alignment styles", async () => {
  const { renderer, renderOnce } = await createTestRenderer({ width: 80, height: 24 })

  const container = new BoxRenderable(renderer, {
    width: 12,
    height: 8,
    flexDirection: "row",
    flexWrap: "wrap",
    justifyContent: "space-between",
    alignItems: "center",
    rowGap: 1,
    columnGap: 1,
  })
  const first = new BoxRenderable(renderer, { width: 5, height: 2 })
  const second = new BoxRenderable(renderer, { width: 5, height: 2 })
  const third = new BoxRenderable(renderer, { width: 5, height: 2 })

  renderer.root.add(container)
  container.add(first)
  container.add(second)
  container.add(third)
  await renderOnce()

  expectNativeLayoutParity(renderer, container)
  expectNativeLayoutParity(renderer, first)
  expectNativeLayoutParity(renderer, second)
  expectNativeLayoutParity(renderer, third)

  renderer.destroy()
})
