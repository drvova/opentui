import { defineStruct, defineEnum } from "bun-ffi-structs"
import { ptr, toArrayBuffer, type Pointer } from "bun:ffi"
import { RGBA } from "./lib/RGBA.js"

const rgbaPackTransform = (rgba?: RGBA) => (rgba ? ptr(rgba.buffer) : null)
const rgbaUnpackTransform = (ptr?: Pointer) => (ptr ? RGBA.fromArray(new Float32Array(toArrayBuffer(ptr))) : undefined)

type StyledChunkInput = {
  text: string
  fg?: RGBA | null
  bg?: RGBA | null
  attributes?: number | null
  link?: { url: string } | string | null
}

export const StyledChunkStruct = defineStruct(
  [
    ["text", "char*"],
    ["text_len", "u64", { lengthOf: "text" }],
    [
      "fg",
      "pointer",
      {
        optional: true,
        packTransform: rgbaPackTransform,
        unpackTransform: rgbaUnpackTransform,
      },
    ],
    [
      "bg",
      "pointer",
      {
        optional: true,
        packTransform: rgbaPackTransform,
        unpackTransform: rgbaUnpackTransform,
      },
    ],
    ["attributes", "u32", { default: 0 }],
    ["link", "char*", { default: "" }],
    ["link_len", "u64", { lengthOf: "link" }],
  ],
  {
    mapValue: (chunk: StyledChunkInput): StyledChunkInput => {
      if (!chunk.link || typeof chunk.link === "string") {
        return chunk
      }

      return {
        ...chunk,
        link: chunk.link.url,
      }
    },
  },
)

export const HighlightStruct = defineStruct([
  ["start", "u32"],
  ["end", "u32"],
  ["styleId", "u32"],
  ["priority", "u8", { default: 0 }],
  ["hlRef", "u16", { default: 0 }],
])

export const LogicalCursorStruct = defineStruct([
  ["row", "u32"],
  ["col", "u32"],
  ["offset", "u32"],
])

export const VisualCursorStruct = defineStruct([
  ["visualRow", "u32"],
  ["visualCol", "u32"],
  ["logicalRow", "u32"],
  ["logicalCol", "u32"],
  ["offset", "u32"],
])

const UnicodeMethodEnum = defineEnum({ wcwidth: 0, unicode: 1 }, "u8")

export const TerminalCapabilitiesStruct = defineStruct([
  ["kitty_keyboard", "bool_u8"],
  ["kitty_graphics", "bool_u8"],
  ["rgb", "bool_u8"],
  ["unicode", UnicodeMethodEnum],
  ["sgr_pixels", "bool_u8"],
  ["color_scheme_updates", "bool_u8"],
  ["explicit_width", "bool_u8"],
  ["scaled_text", "bool_u8"],
  ["sixel", "bool_u8"],
  ["focus_tracking", "bool_u8"],
  ["sync", "bool_u8"],
  ["bracketed_paste", "bool_u8"],
  ["hyperlinks", "bool_u8"],
  ["osc52", "bool_u8"],
  ["explicit_cursor_positioning", "bool_u8"],
  ["term_name", "char*"],
  ["term_name_len", "u64", { lengthOf: "term_name" }],
  ["term_version", "char*"],
  ["term_version_len", "u64", { lengthOf: "term_version" }],
  ["term_from_xtversion", "bool_u8"],
])

export const EncodedCharStruct = defineStruct([
  ["width", "u8"],
  ["char", "u32"],
])

export const LineInfoStruct = defineStruct([
  ["startCols", ["u32"]],
  ["startColsLen", "u32", { lengthOf: "startCols" }],
  ["widthCols", ["u32"]],
  ["widthColsLen", "u32", { lengthOf: "widthCols" }],
  ["sources", ["u32"]],
  ["sourcesLen", "u32", { lengthOf: "sources" }],
  ["wraps", ["u32"]],
  ["wrapsLen", "u32", { lengthOf: "wraps" }],
  ["widthColsMax", "u32"],
])

export const MeasureResultStruct = defineStruct([
  ["lineCount", "u32"],
  ["widthColsMax", "u32"],
])

export const CursorStateStruct = defineStruct([
  ["x", "u32"],
  ["y", "u32"],
  ["visible", "bool_u8"],
  ["style", "u8"],
  ["blinking", "bool_u8"],
  ["r", "f32"],
  ["g", "f32"],
  ["b", "f32"],
  ["a", "f32"],
])

export const CursorStyleOptionsStruct = defineStruct([
  ["style", "u8", { default: 255 }],
  ["blinking", "u8", { default: 255 }],
  [
    "color",
    "pointer",
    {
      optional: true,
      packTransform: rgbaPackTransform,
      unpackTransform: rgbaUnpackTransform,
    },
  ],
  ["cursor", "u8", { default: 255 }],
])

export const GridDrawOptionsStruct = defineStruct([
  ["drawInner", "bool_u8", { default: true }],
  ["drawOuter", "bool_u8", { default: true }],
])

export const SceneLayoutStruct = defineStruct([
  ["left", "f32"],
  ["top", "f32"],
  ["width", "f32"],
  ["height", "f32"],
])

export const TextTableMeasureConfigStruct = defineStruct([
  ["rowCount", "u32", { default: 0 }],
  ["columnCount", "u32", { default: 0 }],
  ["cellPadding", "u32", { default: 0 }],
  ["wrapMode", "u8", { default: 2 }],
  ["columnWidthMode", "u8", { default: 1 }],
  ["columnFitter", "u8", { default: 0 }],
  ["border", "bool_u8", { default: true }],
  ["outerBorder", "bool_u8", { default: true }],
  ["clampAtMost", "bool_u8", { default: true }],
  ["reserved", "u8", { default: 0 }],
])

export const SceneRenderCommandStruct = defineStruct([
  ["kind", "u8", { default: 0 }],
  ["hasClip", "u8", { default: 0 }],
  ["reserved1", "u8", { default: 0 }],
  ["reserved2", "u8", { default: 0 }],
  ["renderableNum", "u32", { default: 0 }],
  ["x", "i32", { default: 0 }],
  ["y", "i32", { default: 0 }],
  ["width", "u32", { default: 0 }],
  ["height", "u32", { default: 0 }],
  ["screenX", "i32", { default: 0 }],
  ["screenY", "i32", { default: 0 }],
  ["clipX", "i32", { default: 0 }],
  ["clipY", "i32", { default: 0 }],
  ["clipWidth", "u32", { default: 0 }],
  ["clipHeight", "u32", { default: 0 }],
  ["opacity", "f32", { default: 1 }],
])

export const SceneStyleStruct = defineStruct([
  ["width", "f32", { default: 0 }],
  ["height", "f32", { default: 0 }],
  ["minWidth", "f32", { default: 0 }],
  ["minHeight", "f32", { default: 0 }],
  ["maxWidth", "f32", { default: 0 }],
  ["maxHeight", "f32", { default: 0 }],
  ["flexGrow", "f32", { default: 0 }],
  ["flexShrink", "f32", { default: 1 }],
  ["flexBasis", "f32", { default: 0 }],
  ["left", "f32", { default: 0 }],
  ["right", "f32", { default: 0 }],
  ["top", "f32", { default: 0 }],
  ["bottom", "f32", { default: 0 }],
  ["marginTop", "f32", { default: 0 }],
  ["marginRight", "f32", { default: 0 }],
  ["marginBottom", "f32", { default: 0 }],
  ["marginLeft", "f32", { default: 0 }],
  ["paddingTop", "f32", { default: 0 }],
  ["paddingRight", "f32", { default: 0 }],
  ["paddingBottom", "f32", { default: 0 }],
  ["paddingLeft", "f32", { default: 0 }],
  ["marginAll", "f32", { default: 0 }],
  ["marginHorizontal", "f32", { default: 0 }],
  ["marginVertical", "f32", { default: 0 }],
  ["paddingAll", "f32", { default: 0 }],
  ["paddingHorizontal", "f32", { default: 0 }],
  ["paddingVertical", "f32", { default: 0 }],
  ["gapAll", "f32", { default: 0 }],
  ["gapRow", "f32", { default: 0 }],
  ["gapColumn", "f32", { default: 0 }],
  ["borderTop", "f32", { default: 0 }],
  ["borderRight", "f32", { default: 0 }],
  ["borderBottom", "f32", { default: 0 }],
  ["borderLeft", "f32", { default: 0 }],
  ["zIndex", "f32", { default: 0 }],
  ["opacity", "f32", { default: 1 }],
  ["translateX", "f32", { default: 0 }],
  ["translateY", "f32", { default: 0 }],
  ["widthUnit", "u8", { default: 3 }],
  ["heightUnit", "u8", { default: 3 }],
  ["minWidthUnit", "u8", { default: 3 }],
  ["minHeightUnit", "u8", { default: 3 }],
  ["maxWidthUnit", "u8", { default: 3 }],
  ["maxHeightUnit", "u8", { default: 3 }],
  ["flexBasisUnit", "u8", { default: 3 }],
  ["leftUnit", "u8", { default: 3 }],
  ["rightUnit", "u8", { default: 3 }],
  ["topUnit", "u8", { default: 3 }],
  ["bottomUnit", "u8", { default: 3 }],
  ["marginTopUnit", "u8", { default: 3 }],
  ["marginRightUnit", "u8", { default: 3 }],
  ["marginBottomUnit", "u8", { default: 3 }],
  ["marginLeftUnit", "u8", { default: 3 }],
  ["paddingTopUnit", "u8", { default: 3 }],
  ["paddingRightUnit", "u8", { default: 3 }],
  ["paddingBottomUnit", "u8", { default: 3 }],
  ["paddingLeftUnit", "u8", { default: 3 }],
  ["marginAllUnit", "u8", { default: 3 }],
  ["marginHorizontalUnit", "u8", { default: 3 }],
  ["marginVerticalUnit", "u8", { default: 3 }],
  ["paddingAllUnit", "u8", { default: 3 }],
  ["paddingHorizontalUnit", "u8", { default: 3 }],
  ["paddingVerticalUnit", "u8", { default: 3 }],
  ["gapAllUnit", "u8", { default: 3 }],
  ["gapRowUnit", "u8", { default: 3 }],
  ["gapColumnUnit", "u8", { default: 3 }],
  ["display", "u8", { default: 0 }],
  ["flexDirection", "u8", { default: 0 }],
  ["positionType", "u8", { default: 0 }],
  ["overflow", "u8", { default: 0 }],
  ["flexWrap", "u8", { default: 0 }],
  ["alignItems", "u8", { default: 4 }],
  ["justifyContent", "u8", { default: 0 }],
  ["alignSelf", "u8", { default: 0 }],
  ["buffered", "bool_u8", { default: false }],
  ["reserved0", "u8", { default: 0 }],
  ["reserved1", "u8", { default: 0 }],
  ["reserved2", "u8", { default: 0 }],
  ["renderableNum", "u32", { default: 0 }],
])

export type BuildOptions = {
  gpaSafeStats: boolean
  gpaMemoryLimitTracking: boolean
}

export const BuildOptionsStruct = defineStruct([
  ["gpaSafeStats", "bool_u8"],
  ["gpaMemoryLimitTracking", "bool_u8"],
])

export type AllocatorStats = {
  totalRequestedBytes: number
  activeAllocations: number
  smallAllocations: number
  largeAllocations: number
  requestedBytesValid: boolean
}

export const AllocatorStatsStruct = defineStruct([
  ["totalRequestedBytes", "u64"],
  ["activeAllocations", "u64"],
  ["smallAllocations", "u64"],
  ["largeAllocations", "u64"],
  ["requestedBytesValid", "bool_u8"],
])

export type GrowthPolicy = "grow" | "block"

export type NativeSpanFeedOptions = {
  chunkSize?: number
  initialChunks?: number
  maxBytes?: bigint
  growthPolicy?: GrowthPolicy
  autoCommitOnFull?: boolean
  spanQueueCapacity?: number
}

export type NativeSpanFeedStats = {
  bytesWritten: bigint
  spansCommitted: bigint
  chunks: number
  pendingSpans: number
}

export type SpanInfo = {
  chunkPtr: Pointer
  offset: number
  len: number
  chunkIndex: number
}

export type ReserveInfo = {
  ptr: Pointer
  len: number
}

const GrowthPolicyEnum = defineEnum({ grow: 0, block: 1 }, "u8")

export const NativeSpanFeedOptionsStruct = defineStruct([
  ["chunkSize", "u32", { default: 64 * 1024 }],
  ["initialChunks", "u32", { default: 2 }],
  ["maxBytes", "u64", { default: 0n }],
  ["growthPolicy", GrowthPolicyEnum, { default: "grow" }],
  ["autoCommitOnFull", "bool_u8", { default: true }],
  ["spanQueueCapacity", "u32", { default: 0 }],
])

export const NativeSpanFeedStatsStruct = defineStruct([
  ["bytesWritten", "u64"],
  ["spansCommitted", "u64"],
  ["chunks", "u32"],
  ["pendingSpans", "u32"],
])

export const SpanInfoStruct = defineStruct(
  [
    ["chunkPtr", "pointer"],
    ["offset", "u32"],
    ["len", "u32"],
    ["chunkIndex", "u32"],
    ["reserved", "u32", { default: 0 }],
  ],
  {
    reduceValue: (value: { chunkPtr: Pointer; offset: number; len: number; chunkIndex: number }) => ({
      chunkPtr: value.chunkPtr as Pointer,
      offset: value.offset,
      len: value.len,
      chunkIndex: value.chunkIndex,
    }),
  },
)

export const ReserveInfoStruct = defineStruct(
  [
    ["ptr", "pointer"],
    ["len", "u32"],
    ["reserved", "u32", { default: 0 }],
  ],
  {
    reduceValue: (value: { ptr: Pointer; len: number }) => ({
      ptr: value.ptr as Pointer,
      len: value.len,
    }),
  },
)
