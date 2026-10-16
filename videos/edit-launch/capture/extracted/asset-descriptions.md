# Asset inventory

All screens are real captures of the Edit terminal editor at 2x: 2720x1596 px, 136x38 cells of 20x42 px.
Shown at half size (1360x798) one cell is 10x21 px, and the 2x source keeps zooms up to 2x crisp.

Window rects in source px (x, y, w, h):
- menubar 0,0,2720,42 · statusbar 0,1554,2720,42
- Assistant window 0,42,820,1512 · Editor window 820,42,1360,1512 · Files window 2180,42,540,1512
- Composer crop (includes the pickers that open above it) 0,1100,820,454

Theme colors sampled from the captures: surface #1e1e1e, panel #252525, chrome #2d2d2d, elevated #383838,
border #4b4b4b, accent #3b8eea, accent_soft (drop zone) #253a51, text #d4d4d4.

- assets/chat.png — default three-window layout (Assistant left, editor with dock.rs, Files right); Assistant focused with one sent user message. The Assistant's reply is a placeholder: never show the Assistant message area of this file at readable size.
- assets/drag-bottom.png — the Files window being dragged; the blue drop zone fills the bottom half of the editor (rect 820,798,1360,756). Files still drawn at its original place.
- assets/drag-left.png — the Files window being dragged; the drop zone sits on the left part of the editor.
- assets/dropped-bottom.png — after the drop: Assistant left, editor top right (820,42,1900,756), Files docked under it (820,798,1900,756) with an accent title.
- assets/solo.png — Assistant plus a wide editor (Files closed), editor focused.
- assets/empty.png — default three-window layout (Assistant left, editor with dock.rs, Files right), Assistant focused showing its empty state ("Ask about your code" and tips), empty composer with the "Describe a change…" placeholder, grey send button.
- assets/chip.png — composer with a "dock.rs ×" context chip (row 30: 80,1260,220,42) and a two-line prompt: row 31 "Add a shortcut that swaps the" (x 80..660, y 1302..1344), row 32 "editor with the files panel" (x 80..620, y 1344..1386), caret at x 620; accent send button (x 680..740, y 1386..1428). Card background #383838.
- assets/chip-mode.png — same composer as chip.png with the mode picker open (Agent, Ask, Plan).
- assets/chip-model.png — same composer as chip.png with the model picker open (Auto, Claude Opus 5.5, Claude Sonnet 5.5, GPT-5.6).
- assets/zoom.png — the editor zoomed to full screen on theme.rs: the Theme struct with surface, panel, chrome, elevated, border, accent fields and their doc comments.
- assets/menu.png — File menu open over the three-window layout (background shot).
- assets/search.png — find bar with "PanelId" match highlighted (background shot).
- assets/fonts/ — JetBrains Mono 400/500/700/800 and VT323 400 (OFL), woff2.
