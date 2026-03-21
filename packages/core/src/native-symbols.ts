export const syntaxStyleSymbols = {
  createSyntaxStyle: {
    args: [],
    returns: "ptr",
  },
  destroySyntaxStyle: {
    args: ["ptr"],
    returns: "void",
  },
  syntaxStyleRegister: {
    args: ["ptr", "ptr", "usize", "ptr", "ptr", "u8"],
    returns: "u32",
  },
  syntaxStyleResolveByName: {
    args: ["ptr", "ptr", "usize"],
    returns: "u32",
  },
  syntaxStyleGetStyleCount: {
    args: ["ptr"],
    returns: "usize",
  },
} as const

export const textRuntimeSymbols = {
  createTextBuffer: {
    args: ["u8"],
    returns: "ptr",
  },
  destroyTextBuffer: {
    args: ["ptr"],
    returns: "void",
  },
  textBufferGetLength: {
    args: ["ptr"],
    returns: "u32",
  },
  textBufferGetByteSize: {
    args: ["ptr"],
    returns: "u32",
  },
  textBufferReset: {
    args: ["ptr"],
    returns: "void",
  },
  textBufferClear: {
    args: ["ptr"],
    returns: "void",
  },
  textBufferSetDefaultFg: {
    args: ["ptr", "ptr"],
    returns: "void",
  },
  textBufferSetDefaultBg: {
    args: ["ptr", "ptr"],
    returns: "void",
  },
  textBufferSetDefaultAttributes: {
    args: ["ptr", "ptr"],
    returns: "void",
  },
  textBufferResetDefaults: {
    args: ["ptr"],
    returns: "void",
  },
  textBufferGetTabWidth: {
    args: ["ptr"],
    returns: "u8",
  },
  textBufferSetTabWidth: {
    args: ["ptr", "u8"],
    returns: "void",
  },
  textBufferRegisterMemBuffer: {
    args: ["ptr", "ptr", "usize", "bool"],
    returns: "u16",
  },
  textBufferReplaceMemBuffer: {
    args: ["ptr", "u8", "ptr", "usize", "bool"],
    returns: "bool",
  },
  textBufferClearMemRegistry: {
    args: ["ptr"],
    returns: "void",
  },
  textBufferSetTextFromMem: {
    args: ["ptr", "u8"],
    returns: "void",
  },
  textBufferAppend: {
    args: ["ptr", "ptr", "usize"],
    returns: "void",
  },
  textBufferAppendFromMemId: {
    args: ["ptr", "u8"],
    returns: "void",
  },
  textBufferLoadFile: {
    args: ["ptr", "ptr", "usize"],
    returns: "bool",
  },
  textBufferSetStyledText: {
    args: ["ptr", "ptr", "usize"],
    returns: "void",
  },
  textBufferGetLineCount: {
    args: ["ptr"],
    returns: "u32",
  },
  textBufferGetPlainText: {
    args: ["ptr", "ptr", "usize"],
    returns: "usize",
  },
  textBufferAddHighlightByCharRange: {
    args: ["ptr", "ptr"],
    returns: "void",
  },
  textBufferAddHighlight: {
    args: ["ptr", "u32", "ptr"],
    returns: "void",
  },
  textBufferRemoveHighlightsByRef: {
    args: ["ptr", "u16"],
    returns: "void",
  },
  textBufferClearLineHighlights: {
    args: ["ptr", "u32"],
    returns: "void",
  },
  textBufferClearAllHighlights: {
    args: ["ptr"],
    returns: "void",
  },
  textBufferSetSyntaxStyle: {
    args: ["ptr", "ptr"],
    returns: "void",
  },
  textBufferGetLineHighlightsPtr: {
    args: ["ptr", "u32", "ptr"],
    returns: "ptr",
  },
  textBufferFreeLineHighlights: {
    args: ["ptr", "usize"],
    returns: "void",
  },
  textBufferGetHighlightCount: {
    args: ["ptr"],
    returns: "u32",
  },
  textBufferGetTextRange: {
    args: ["ptr", "u32", "u32", "ptr", "usize"],
    returns: "usize",
  },
  textBufferGetTextRangeByCoords: {
    args: ["ptr", "u32", "u32", "u32", "u32", "ptr", "usize"],
    returns: "usize",
  },
  createTextBufferView: {
    args: ["ptr"],
    returns: "ptr",
  },
  destroyTextBufferView: {
    args: ["ptr"],
    returns: "void",
  },
  textBufferViewSetSelection: {
    args: ["ptr", "u32", "u32", "ptr", "ptr"],
    returns: "void",
  },
  textBufferViewResetSelection: {
    args: ["ptr"],
    returns: "void",
  },
  textBufferViewGetSelectionInfo: {
    args: ["ptr"],
    returns: "u64",
  },
  textBufferViewSetLocalSelection: {
    args: ["ptr", "i32", "i32", "i32", "i32", "ptr", "ptr"],
    returns: "bool",
  },
  textBufferViewUpdateSelection: {
    args: ["ptr", "u32", "ptr", "ptr"],
    returns: "void",
  },
  textBufferViewUpdateLocalSelection: {
    args: ["ptr", "i32", "i32", "i32", "i32", "ptr", "ptr"],
    returns: "bool",
  },
  textBufferViewResetLocalSelection: {
    args: ["ptr"],
    returns: "void",
  },
  textBufferViewSetWrapWidth: {
    args: ["ptr", "u32"],
    returns: "void",
  },
  textBufferViewSetWrapMode: {
    args: ["ptr", "u8"],
    returns: "void",
  },
  textBufferViewSetViewportSize: {
    args: ["ptr", "u32", "u32"],
    returns: "void",
  },
  textBufferViewSetViewport: {
    args: ["ptr", "u32", "u32", "u32", "u32"],
    returns: "void",
  },
  textBufferViewGetVirtualLineCount: {
    args: ["ptr"],
    returns: "u32",
  },
  textBufferViewGetLineInfoDirect: {
    args: ["ptr", "ptr"],
    returns: "void",
  },
  textBufferViewGetLogicalLineInfoDirect: {
    args: ["ptr", "ptr"],
    returns: "void",
  },
  textBufferViewGetSelectedText: {
    args: ["ptr", "ptr", "usize"],
    returns: "usize",
  },
  textBufferViewGetPlainText: {
    args: ["ptr", "ptr", "usize"],
    returns: "usize",
  },
  textBufferViewSetTabIndicator: {
    args: ["ptr", "u32"],
    returns: "void",
  },
  textBufferViewSetTabIndicatorColor: {
    args: ["ptr", "ptr"],
    returns: "void",
  },
  textBufferViewSetTruncate: {
    args: ["ptr", "bool"],
    returns: "void",
  },
  textBufferViewMeasureForDimensions: {
    args: ["ptr", "u32", "u32", "ptr"],
    returns: "bool",
  },
  bufferDrawTextBufferView: {
    args: ["ptr", "ptr", "i32", "i32"],
    returns: "void",
  },
  bufferDrawEditorView: {
    args: ["ptr", "ptr", "i32", "i32"],
    returns: "void",
  },
  createEditorView: {
    args: ["ptr", "u32", "u32"],
    returns: "ptr",
  },
  destroyEditorView: {
    args: ["ptr"],
    returns: "void",
  },
  editorViewSetViewportSize: {
    args: ["ptr", "u32", "u32"],
    returns: "void",
  },
  editorViewSetViewport: {
    args: ["ptr", "u32", "u32", "u32", "u32", "bool"],
    returns: "void",
  },
  editorViewGetViewport: {
    args: ["ptr", "ptr", "ptr", "ptr", "ptr"],
    returns: "void",
  },
  editorViewSetScrollMargin: {
    args: ["ptr", "f32"],
    returns: "void",
  },
  editorViewSetWrapMode: {
    args: ["ptr", "u8"],
    returns: "void",
  },
  editorViewGetVirtualLineCount: {
    args: ["ptr"],
    returns: "u32",
  },
  editorViewGetTotalVirtualLineCount: {
    args: ["ptr"],
    returns: "u32",
  },
  editorViewGetTextBufferView: {
    args: ["ptr"],
    returns: "ptr",
  },
  editorViewGetLineInfoDirect: {
    args: ["ptr", "ptr"],
    returns: "void",
  },
  editorViewGetLogicalLineInfoDirect: {
    args: ["ptr", "ptr"],
    returns: "void",
  },
  createEditBuffer: {
    args: ["u8"],
    returns: "ptr",
  },
  destroyEditBuffer: {
    args: ["ptr"],
    returns: "void",
  },
  editBufferSetText: {
    args: ["ptr", "ptr", "usize"],
    returns: "void",
  },
  editBufferSetTextFromMem: {
    args: ["ptr", "u8"],
    returns: "void",
  },
  editBufferReplaceText: {
    args: ["ptr", "ptr", "usize"],
    returns: "void",
  },
  editBufferReplaceTextFromMem: {
    args: ["ptr", "u8"],
    returns: "void",
  },
  editBufferGetText: {
    args: ["ptr", "ptr", "usize"],
    returns: "usize",
  },
  editBufferInsertChar: {
    args: ["ptr", "ptr", "usize"],
    returns: "void",
  },
  editBufferInsertText: {
    args: ["ptr", "ptr", "usize"],
    returns: "void",
  },
  editBufferDeleteChar: {
    args: ["ptr"],
    returns: "void",
  },
  editBufferDeleteCharBackward: {
    args: ["ptr"],
    returns: "void",
  },
  editBufferDeleteRange: {
    args: ["ptr", "u32", "u32", "u32", "u32"],
    returns: "void",
  },
  editBufferNewLine: {
    args: ["ptr"],
    returns: "void",
  },
  editBufferDeleteLine: {
    args: ["ptr"],
    returns: "void",
  },
  editBufferMoveCursorLeft: {
    args: ["ptr"],
    returns: "void",
  },
  editBufferMoveCursorRight: {
    args: ["ptr"],
    returns: "void",
  },
  editBufferMoveCursorUp: {
    args: ["ptr"],
    returns: "void",
  },
  editBufferMoveCursorDown: {
    args: ["ptr"],
    returns: "void",
  },
  editBufferGotoLine: {
    args: ["ptr", "u32"],
    returns: "void",
  },
  editBufferSetCursor: {
    args: ["ptr", "u32", "u32"],
    returns: "void",
  },
  editBufferSetCursorToLineCol: {
    args: ["ptr", "u32", "u32"],
    returns: "void",
  },
  editBufferSetCursorByOffset: {
    args: ["ptr", "u32"],
    returns: "void",
  },
  editBufferGetCursorPosition: {
    args: ["ptr", "ptr"],
    returns: "void",
  },
  editBufferGetId: {
    args: ["ptr"],
    returns: "u16",
  },
  editBufferGetTextBuffer: {
    args: ["ptr"],
    returns: "ptr",
  },
  editBufferDebugLogRope: {
    args: ["ptr"],
    returns: "void",
  },
  editBufferUndo: {
    args: ["ptr", "ptr", "usize"],
    returns: "usize",
  },
  editBufferRedo: {
    args: ["ptr", "ptr", "usize"],
    returns: "usize",
  },
  editBufferCanUndo: {
    args: ["ptr"],
    returns: "bool",
  },
  editBufferCanRedo: {
    args: ["ptr"],
    returns: "bool",
  },
  editBufferClearHistory: {
    args: ["ptr"],
    returns: "void",
  },
  editBufferClear: {
    args: ["ptr"],
    returns: "void",
  },
  editBufferGetNextWordBoundary: {
    args: ["ptr", "ptr"],
    returns: "void",
  },
  editBufferGetPrevWordBoundary: {
    args: ["ptr", "ptr"],
    returns: "void",
  },
  editBufferGetEOL: {
    args: ["ptr", "ptr"],
    returns: "void",
  },
  editBufferOffsetToPosition: {
    args: ["ptr", "u32", "ptr"],
    returns: "bool",
  },
  editBufferPositionToOffset: {
    args: ["ptr", "u32", "u32"],
    returns: "u32",
  },
  editBufferGetLineStartOffset: {
    args: ["ptr", "u32"],
    returns: "u32",
  },
  editBufferGetTextRange: {
    args: ["ptr", "u32", "u32", "ptr", "usize"],
    returns: "usize",
  },
  editBufferGetTextRangeByCoords: {
    args: ["ptr", "u32", "u32", "u32", "u32", "ptr", "usize"],
    returns: "usize",
  },
  editorViewSetSelection: {
    args: ["ptr", "u32", "u32", "ptr", "ptr"],
    returns: "void",
  },
  editorViewResetSelection: {
    args: ["ptr"],
    returns: "void",
  },
  editorViewGetSelection: {
    args: ["ptr"],
    returns: "u64",
  },
  editorViewSetLocalSelection: {
    args: ["ptr", "i32", "i32", "i32", "i32", "ptr", "ptr", "bool", "bool"],
    returns: "bool",
  },
  editorViewUpdateSelection: {
    args: ["ptr", "u32", "ptr", "ptr"],
    returns: "void",
  },
  editorViewUpdateLocalSelection: {
    args: ["ptr", "i32", "i32", "i32", "i32", "ptr", "ptr", "bool", "bool"],
    returns: "bool",
  },
  editorViewResetLocalSelection: {
    args: ["ptr"],
    returns: "void",
  },
  editorViewGetSelectedTextBytes: {
    args: ["ptr", "ptr", "usize"],
    returns: "usize",
  },
  editorViewGetCursor: {
    args: ["ptr", "ptr", "ptr"],
    returns: "void",
  },
  editorViewGetText: {
    args: ["ptr", "ptr", "usize"],
    returns: "usize",
  },
  editorViewGetVisualCursor: {
    args: ["ptr", "ptr"],
    returns: "void",
  },
  editorViewMoveUpVisual: {
    args: ["ptr"],
    returns: "void",
  },
  editorViewMoveDownVisual: {
    args: ["ptr"],
    returns: "void",
  },
  editorViewDeleteSelectedText: {
    args: ["ptr"],
    returns: "void",
  },
  editorViewSetCursorByOffset: {
    args: ["ptr", "u32"],
    returns: "void",
  },
  editorViewGetNextWordBoundary: {
    args: ["ptr", "ptr"],
    returns: "void",
  },
  editorViewGetPrevWordBoundary: {
    args: ["ptr", "ptr"],
    returns: "void",
  },
  editorViewGetEOL: {
    args: ["ptr", "ptr"],
    returns: "void",
  },
  editorViewGetVisualSOL: {
    args: ["ptr", "ptr"],
    returns: "void",
  },
  editorViewGetVisualEOL: {
    args: ["ptr", "ptr"],
    returns: "void",
  },
  editorViewSetPlaceholderStyledText: {
    args: ["ptr", "ptr", "usize"],
    returns: "void",
  },
  editorViewSetTabIndicator: {
    args: ["ptr", "u32"],
    returns: "void",
  },
  editorViewSetTabIndicatorColor: {
    args: ["ptr", "ptr"],
    returns: "void",
  },
  ...syntaxStyleSymbols,
} as const

export const nativeSpanFeedSymbols = {
  createNativeSpanFeed: {
    args: ["ptr"],
    returns: "ptr",
  },
  attachNativeSpanFeed: {
    args: ["ptr"],
    returns: "i32",
  },
  destroyNativeSpanFeed: {
    args: ["ptr"],
    returns: "void",
  },
  streamWrite: {
    args: ["ptr", "ptr", "u64"],
    returns: "i32",
  },
  streamCommit: {
    args: ["ptr"],
    returns: "i32",
  },
  streamDrainSpans: {
    args: ["ptr", "ptr", "u32"],
    returns: "u32",
  },
  streamClose: {
    args: ["ptr"],
    returns: "i32",
  },
  streamReserve: {
    args: ["ptr", "u32", "ptr"],
    returns: "i32",
  },
  streamCommitReserved: {
    args: ["ptr", "u32"],
    returns: "i32",
  },
  streamSetOptions: {
    args: ["ptr", "ptr"],
    returns: "i32",
  },
  streamGetStats: {
    args: ["ptr", "ptr"],
    returns: "i32",
  },
  streamSetCallback: {
    args: ["ptr", "ptr"],
    returns: "void",
  },
} as const
