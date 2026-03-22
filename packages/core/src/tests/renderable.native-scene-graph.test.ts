import { expect, test } from "bun:test"

import { OptimizedBuffer } from "../buffer.js"
import { BoxRenderable } from "../renderables/Box.js"
import { CodeRenderable } from "../renderables/Code.js"
import { TextRenderable } from "../renderables/Text.js"
import { TextareaRenderable } from "../renderables/Textarea.js"
import { RGBA } from "../lib/RGBA.js"
import { SyntaxStyle } from "../syntax-style.js"
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

test("Renderable uses native z-index child order for traversal", async () => {
  const { renderer, renderOnce } = await createTestRenderer({ width: 80, height: 24 })

  const parent = new BoxRenderable(renderer, { width: 20, height: 10 })
  const back = new BoxRenderable(renderer, { id: "back", width: 5, height: 2, zIndex: 10 })
  const front = new BoxRenderable(renderer, { id: "front", width: 5, height: 2, zIndex: 0 })

  renderer.root.add(parent)
  parent.add(back)
  parent.add(front)
  await renderOnce()

  ;(parent as any).ensureZIndexSorted()
  expect((parent as any)._getVisibleChildren()).toEqual([front.num, back.num])

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

test("native render plan carries ancestor clip state for hit-grid registration", async () => {
  const { renderer, renderOnce } = await createTestRenderer({ width: 80, height: 24 })

  const container = new BoxRenderable(renderer, {
    width: 10,
    height: 4,
    overflow: "hidden",
  })
  const child = new BoxRenderable(renderer, {
    width: 6,
    height: 2,
    left: 8,
    position: "absolute",
  })

  renderer.root.add(container)
  container.add(child)
  await renderOnce()

  const rootHandle = (renderer.root as any).sceneNodeHandle
  const commands = renderer.sceneNodeBuildRenderPlan(rootHandle)
  const childCommand = commands.find((command) => command.renderableNum === child.num)

  expect(childCommand).toBeTruthy()
  expect(commands.every((command) => command.kind === 0)).toBe(true)
  expect(childCommand).toMatchObject({
    hasClip: 1,
    clipX: container.x,
    clipY: container.y,
    clipWidth: container.width,
    clipHeight: container.height,
  })

  renderer.destroy()
})

test("native render plan carries effective ancestor opacity on render commands", async () => {
  const { renderer, renderOnce } = await createTestRenderer({ width: 80, height: 24 })

  const parent = new BoxRenderable(renderer, {
    width: 10,
    height: 4,
    opacity: 0.5,
  })
  const child = new BoxRenderable(renderer, {
    width: 6,
    height: 2,
    opacity: 0.4,
  })

  renderer.root.add(parent)
  parent.add(child)
  await renderOnce()

  const rootHandle = (renderer.root as any).sceneNodeHandle
  const commands = renderer.sceneNodeBuildRenderPlan(rootHandle)
  const parentCommand = commands.find((command) => command.renderableNum === parent.num)
  const childCommand = commands.find((command) => command.renderableNum === child.num)

  expect(parentCommand?.opacity).toBeCloseTo(0.5)
  expect(childCommand?.opacity).toBeCloseTo(0.2)

  renderer.destroy()
})

test("scene graph can draw a plain BoxRenderable directly through the native path", async () => {
  const { renderer, renderOnce } = await createTestRenderer({ width: 80, height: 24 })

  const box = new BoxRenderable(renderer, {
    width: 8,
    height: 4,
    border: true,
    title: "Hi",
    shouldFill: true,
  })

  renderer.root.add(box)
  await renderOnce()

  const buffer = OptimizedBuffer.create(12, 6, renderer.widthMethod)
  expect(renderer.sceneNodeDrawBox((box as any).sceneNodeHandle, buffer.ptr, 0, 0, box.width, box.height)).toBe(true)

  const frame = new TextDecoder().decode(buffer.getRealCharBytes(true))
  expect(frame).toContain("Hi")
  expect(frame).toContain("┌")

  buffer.destroy()
  renderer.destroy()
})

test("scene graph can draw a plain TextRenderable directly through the native path", async () => {
  const { renderer, renderOnce } = await createTestRenderer({ width: 80, height: 24 })

  const text = new TextRenderable(renderer, {
    width: 10,
    height: 2,
    content: "Hello",
  })

  renderer.root.add(text)
  await renderOnce()

  const buffer = OptimizedBuffer.create(12, 4, renderer.widthMethod)
  expect(renderer.sceneNodeDrawTextBufferView((text as any).sceneNodeHandle, buffer.ptr, 0, 0)).toBe(true)

  const frame = new TextDecoder().decode(buffer.getRealCharBytes(true))
  expect(frame).toContain("Hello")

  buffer.destroy()
  renderer.destroy()
})

test("scene graph can draw a plain CodeRenderable directly through the native text-view path", async () => {
  const { renderer, renderOnce } = await createTestRenderer({ width: 80, height: 24 })

  const syntaxStyle = SyntaxStyle.fromStyles({
    default: { fg: RGBA.fromValues(1, 1, 1, 1) },
  })
  const code = new CodeRenderable(renderer, {
    width: 12,
    height: 2,
    content: "const x = 1",
    syntaxStyle,
    conceal: false,
  })

  renderer.root.add(code)
  await renderOnce()

  const buffer = OptimizedBuffer.create(16, 4, renderer.widthMethod)
  expect(renderer.sceneNodeDrawTextBufferView((code as any).sceneNodeHandle, buffer.ptr, 0, 0)).toBe(true)

  const frame = new TextDecoder().decode(buffer.getRealCharBytes(true))
  expect(frame).toContain("const x = 1")

  buffer.destroy()
  syntaxStyle.destroy()
  renderer.destroy()
})

test("scene graph can draw a plain TextareaRenderable directly through the native editor-view path", async () => {
  const { renderer, renderOnce } = await createTestRenderer({ width: 80, height: 24 })

  const textarea = new TextareaRenderable(renderer, {
    width: 12,
    height: 3,
    initialValue: "hello",
  })

  renderer.root.add(textarea)
  await renderOnce()

  const buffer = OptimizedBuffer.create(16, 4, renderer.widthMethod)
  expect(renderer.sceneNodeDrawEditorView((textarea as any).sceneNodeHandle, buffer.ptr, 0, 0)).toBe(true)

  const frame = new TextDecoder().decode(buffer.getRealCharBytes(true))
  expect(frame).toContain("hello")

  buffer.destroy()
  renderer.destroy()
})
