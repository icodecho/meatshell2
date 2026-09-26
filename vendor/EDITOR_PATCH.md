# Native editor patch (Slint 1.16.1)

`i-slint-core` and `i-slint-compiler` are the published 1.16.1 crates, with
upstream license files retained. Both are selected by `[patch.crates-io]`.
They must be upgraded together; do not remove only one override.

Changes from upstream:

- `i-slint-core/items/text.rs` `TextInput::insert()` normalizes inserted line
  endings: CRLF collapses to a single LF and a stray CR becomes LF. Windows
  clipboards carry `\r\n`; the paragraph splitter in `create_text_paragraphs`
  splits on `\n` only and the leftover `\r` made Parley break the line again,
  so pasting multi-line text into any multi-line input grew a phantom blank
  line between every pair of rows. Cursor/undo bookkeeping uses the normalized
  byte length.
- `i-slint-compiler/builtins.slint` and `i-slint-core/items/text.rs` expose three
  opt-in TextInput properties: `syntax-source`, `syntax-spans`, and
  `line-number-width`. Existing inputs use empty/zero defaults.
- `i-slint-core/textlayout/sharedparley.rs` applies color-only ranges through
  Parley, keeping native shaping, wrap, cursor, clipboard, selection, and IME.
  It is shared by the desktop FemtoVG, Software, and Skia renderers.
  Qt's separate native renderer does not implement these properties, so the
  application explicitly enables Winit instead of `backend-default`. Slint and
  slint-build are pinned to the patched version.
- Syntax spans use sorted, non-overlapping UTF-8 byte ranges encoded as
  `start:end:AARRGGBB;`. Invalid ranges are ignored. Source mismatch or active
  preedit disables spans, so debounced results never color the wrong text.
  Selection foreground is applied last and overrides syntax colors.
- A positive `line-number-width` draws logical line numbers only, aligned to
  the first visual baseline of each paragraph. The gutter uses the same source,
  font, width, height, and wrapping as the editable input and is clipped by its
  narrow parent. It draws only visible numbers instead of creating a Slint item
  for every logical line.
- Opt-in inputs cache one layout per item. Text, source, ranges, font, scale,
  wrap, size, alignment, selection, and preedit changes invalidate it; moving
  the viewport or blinking the cursor does not. Cache lifetime is the item
  lifetime. Paragraph drawing skips text outside the clip with overhang margin.

The application owns language detection and lexical tokenization in
`src/app/editor_syntax.rs`; the renderer does not parse programming languages.
Highlighting is lexical, not a language server or semantic analyzer.

Validation: `cargo test --locked editor_ -- --nocapture`. The headless native
render test compares plain/highlighted geometry at two wrapping widths, checks
selection precedence, verifies stale spans cannot affect edits, exercises the
gutter, and prints 10,000-line first/cached paint timings. Its screenshots go to
`target/editor-*.png`. Run this test when rebasing the patch, in addition to
checking IME behavior on the target desktop platforms.
