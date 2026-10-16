---
format: 1920x1080
duration: 30s
message: "The editor you grew up with, rebuilt for how you work now."
arc: Hook (1991) → Reveal (2026) → Docking → Assistant → New look → Launch
audience: developers who live in the terminal
mode: autonomous
music: none
bpm: 120
---

## Video direction

- **Palette** (from `frame.md`, dark register only): ground `#1E1E1E`, raised ground `#232323`, text `#D4D4D4`, muted text `#8E8E8E`, hint `#5C5C5C`, hairline `#484848`, the one accent `#3B8EEA`. Frame 1 is the single exception: the 1991 DOS palette (`#0000AA` blue, `#AAAAAA` grey, `#FFFFFF`, `#00AAAA` cyan), shown as a period piece.
- **Type**: JetBrains Mono for everything (local files in `assets/fonts/`), display lowercase weight 800 with negative tracking, kickers uppercase weight 500 tracked 0.14em in the accent. VT323 only inside Frame 1's DOS screen.
- **Music grid**: 120 BPM, one beat = 0.5s, one bar = 2s. Every frame starts on a bar line. Every hard cut, slam, swap and drop lands on a beat (frame-local times that are multiples of 0.5s; 0.25s offsets only for secondary accents).
- **Motion grammar**: one camera. Long-tail eases (`power3.out` / `expo.out` for arrivals, `power2.inOut` for camera glides), snappy in, slow settle. Reveals step in on beats across the whole shot; never front-load. Holds are still: at most a sub-pixel jitter.
- **The isometric world**: real screenshots sit on flat plates in one shared CSS 3D pose (stated in each frame). Plates are flat: a 1px `#484848` edge plus a solid `#141414` side slab for thickness, never a blurred drop shadow, gradient or glow (the accent may glow only in Frame 1's CRT and on the focused window border).
- **Rhythm**: Frame 1 builds, Frame 2 explodes, Frames 3 and 4 are the busy demos, Frame 5 is the held breather, Frame 6 lands the slam and holds still.
- **Never**: em-dashes in copy, a second accent hue, purple/blue AI gradients, bokeh, fake UI not in the captures, the Assistant's placeholder reply text at readable size, slideshow pacing (everything in at t=0 then frozen), screensaver pacing (everything drifting independently).

## Frame 1 — 1991

- scene: A blue MS-DOS Edit screen in CRT glow, cursor blinking, a line types; on the downbeat it glitches apart and collapses to a CRT power-off line
- voiceover: ""
- duration: 4s
- transition_in: cut
- status: animated
- src: compositions/frames/01-dos.html
- type: hook
- persuasion: Nostalgia as pattern interrupt
- beat: curiosity + recognition
- blueprint: typewriter-reveal (Adapt)
- asset_candidates:
- focal: the DOS Edit screen (HTML recreation, typography only)
- roles: none (typography-only period piece)
- sfx: key-press, glitch-1
- rules: discrete-text-sequence, context-sensitive-cursor, chromatic-glitch
- handoff_out: element "crt-line": a horizontal line centered at x 960, y 540, width 1920px, height 4px, color #EAF2FF with a soft #3B8EEA glow, scaleX 1, opacity 1, static (no motion) at t=4.0s.

Adapt: keep the typewriter signature (a caret types a line as a human would), drop the brand pop; the payoff is the glitch-out into the next frame instead.

Shared spec: canvas 1920x1080. Fonts via `@font-face` from `assets/fonts/VT323-400.woff2` (DOS screen) and `assets/fonts/JetBrainsMono-500.woff2` (the year label). Ground `#000` behind the CRT.

The DOS screen is an 80x25 text grid recreation of MS-DOS Editor (1991) in VT323, cell size 24x43.2px so the grid fills 1920x1080:
row 1: grey (`#AAAAAA`) menu bar, black text: "  File  Edit  Search  Options                                             Help";
row 2: blue (`#0000AA`) field with a white double-line box border (`╔═╗║╚╝`) spanning rows 2-23, title "Untitled" centered in the top border as black-on-grey;
rows 3-22: document area, white text on blue; right edge a grey scrollbar column (`░` track, `▓` thumb, `↑`/`↓` arrows); row 23 bottom border with a horizontal scrollbar;
row 24: cyan (`#00AAAA`) status bar, black text: " MS-DOS Editor  <F1=Help> Press ALT to activate menus" left and "00001:001" right.
CRT treatment over the whole screen: fine horizontal scanlines (every 3px, ~18% darker), a soft phosphor bloom on the white/grey text, a rounded inner vignette (corners darken), very slight barrel feel via a rounded-rectangle mask with 40px radius.

Scene 1 (0.0–0.5s): hard open on the DOS screen at full frame, the blinking block caret (`_` underline cursor, white) at row 3 col 2. A small label top-right, outside the DOS grid feel (JetBrains Mono 500, uppercase, tracked, `#3B8EEA`): "1991". A slow push-in starts on the whole screen (scale 1.00 → 1.05 across 0–3s) → multi-phase-camera.
Scene 2 (0.5–2.5s): the caret types `REM the editor you grew up with` character by character on row 3 (one key-press accent at 0.5s; typing finishes by 2.25s) → discrete-text-sequence + context-sensitive-cursor. Status bar column counter advances with the caret (00001:002 … 00001:031).
Scene 3 (2.5–3.0s): the caret blinks twice, still; the screen is read.
Scene 4 (3.0–3.75s): on the 3.0s downbeat (glitch-1) the screen tears: RGB split (red/cyan copies offset ±6 → ±24px), horizontal slice displacement on 6–10 bands that jump on every 1/12s step, brightness flicker, the "1991" label scrambles → chromatic-glitch. Intensity peaks at 3.5s.
Scene 5 (3.75–4.0s): CRT power-off: the whole screen squashes vertically to a 4px bright line (scaleY → 0.004) while brightening to near white, then the line holds static at t=4.0 per handoff_out.

narrativeRole: stop the scroll with a screen every developer over 30 recognises, then break it.
keyMessage: this is the editor you remember.

## Frame 2 — Rebuilt

- scene: A CRT power-on line opens into the modern Edit screen flat, then the camera pulls back and tilts it into an isometric plate whose windows explode apart in layers, and settle back down
- voiceover: ""
- duration: 4s
- transition_in: cut
- status: animated
- src: compositions/frames/02-reveal.html
- type: product_intro
- persuasion: Negative contrast (then vs now)
- beat: surprise → awe
- blueprint: zoom-out-workspace-reveal (Adapt)
- asset_candidates: assets/empty.png — default three-window layout, Assistant focused
- focal: assets/empty.png
- roles: empty.png = cutout (the plate and its window layers)
- sfx: impact-bass-1, whoosh-cinematic
- rules: multi-phase-camera, depth-scatter-assemble, kinetic-beat-slam, discrete-text-sequence
- handoff_in: element "crt-line": horizontal line centered at x 960, y 540, width 1920px, height 4px, color #EAF2FF with a soft #3B8EEA glow, scaleX 1, opacity 1, static at t=0.
- handoff_out: element "iso-plate": the empty.png plate in the shared ISO POSE, camera wrapper transform `translate(150px, 20px) scale(1)`, all window layers at translateZ 0, opacity 1, static (velocity 0) at t=4.0s. Headline block on the left removed by t=4.0 (hard cut).

Adapt: keep the signature one continuous decelerating pull-back that re-scopes the detail into the whole workspace; we open full-frame on the real screen instead of a macro detail, and add the exploded-layer beat on top.

Shared spec — ISO POSE (identical in Frames 2, 3, 5): a `.iso-stage` full-frame container with `perspective: 4200px; perspective-origin: 50% 45%`; inside it a `.camera` wrapper (absolute, inset 0, `transform-style: preserve-3d`, the camera moves animate THIS element); inside it the `.iso` plate: absolute, `left: 280px; top: 141px; width: 1360px; height: 798px` (centered in the frame), `transform-style: preserve-3d; transform: rotateX(54deg) rotateZ(-38deg)`. Screens are `<img>` at the plate's size (source 2720x1596 drawn at 1360x798, so source px / 2 = plate px). Window layers are `div`s inside `.iso`, each `overflow: hidden` showing a crop of the same screenshot via an inner `<img>` offset by the crop origin. Crops in source px (x, y, w, h): menubar 0,0,2720,42 · Assistant 0,42,820,1512 · Editor 820,42,1360,1512 · Files 2180,42,540,1512 · statusbar 0,1554,2720,42. Each layer gets a 1px `#484848` edge; the base plate gets a `#141414` side slab 14px thick (a sibling at translateZ -14px) for thickness. Fonts: `assets/fonts/JetBrainsMono-800.woff2` (display) and `-500` (kicker). Ground `#1E1E1E`.

Scene 1 (0.0–0.5s): the crt-line from handoff_in expands vertically (scaleY from the line to full) into empty.png shown flat, full-bleed (camera at `scale(1.42)` with the plate rotation at 0, so the screenshot covers the frame), slight white overexposure fading out in 0.25s. impact-bass-1 at 0.0. Top-right label "2026" (same position/style as Frame 1's "1991": JetBrains Mono 500, uppercase, tracked, `#3B8EEA`) hard-cuts in at 0.0 → discrete-text-sequence.
Scene 2 (0.5–1.5s): ONE continuous decelerating pull-back: the camera scales 1.42 → 1.0 and translates to `translate(150px, 20px)` while the plate rotates from flat into the ISO POSE (rotateX 0 → 54deg, rotateZ 0 → -38deg), expo-style ease that lands softly at 1.5s → multi-phase-camera. whoosh-cinematic swells under this.
Scene 3 (1.5–2.5s): the windows explode upward in layers on beats → depth-scatter-assemble: at 1.5s the menubar and statusbar strips lift to translateZ 40px; at 2.0s the Assistant and Files layers lift to 110px; at 2.0s the Editor (the focal, accent title) lifts to 170px. The base plate under them shows the empty desktop (#1E1E1E fill drawn over the window regions of the base). Hairline leader ticks are not needed.
Scene 4 (1.5–3.5s), left column (x 120–700, vertically centered around y 470): kicker "the editor you grew up with" (uppercase, accent) slams in at 2.5s; display h1 "rebuilt." (lowercase 800, `#D4D4D4`, the period in `#3B8EEA`, ~140px) slams in at 3.0s → kinetic-beat-slam.
Scene 5 (3.5–4.0s): layers drop back to translateZ 0 on the 3.5s beat (fast, power3.in then a tiny settle), plate holds still in the ISO POSE; at 3.75s the left headline block hard-cuts out; t=4.0 matches handoff_out exactly.

narrativeRole: the same editor, reborn; show the whole workspace at once.
keyMessage: edit, rebuilt for 2026.

## Frame 3 — Windows that dock

- scene: In the isometric plate the Files window lifts off, glides over the editor, a blue drop zone lights up under it and it docks below the editor
- voiceover: ""
- duration: 6s
- transition_in: cut
- status: animated
- src: compositions/frames/03-dock.html
- type: feature_showcase
- persuasion: Show-don't-tell proof
- beat: control
- blueprint: camera-journey (Adapt)
- asset_candidates: assets/empty.png — three-window layout; assets/drag-bottom.png — drop zone on the bottom half of the editor; assets/dropped-bottom.png — Files docked under the editor
- focal: the Files window layer (crop of assets/empty.png)
- roles: empty.png = cutout (base plate and the Files layer) · drag-bottom.png = supporting (base during the drag, shows the real drop zone) · dropped-bottom.png = supporting (base after the drop)
- sfx: whoosh-short, click
- rules: coordinate-target-zoom, card-morph-anchor, spring-pop-entrance
- handoff_in: element "iso-plate": empty.png plate in the shared ISO POSE, camera wrapper `translate(150px, 20px) scale(1)`, all window layers at translateZ 0, opacity 1, static at t=0. No headline on screen at t=0.

Adapt: keep the action-roundtrip signature (dive to a panel, the action fires, the camera travels to where the consequence lands), but there is no cursor: the window itself is the actor.

Shared spec — ISO POSE: a `.iso-stage` full-frame container with `perspective: 4200px; perspective-origin: 50% 45%`; inside it a `.camera` wrapper (absolute, inset 0, `transform-style: preserve-3d`, camera moves animate THIS element); inside it the `.iso` plate: absolute, `left: 280px; top: 141px; width: 1360px; height: 798px`, `transform-style: preserve-3d; transform: rotateX(54deg) rotateZ(-38deg)`. Screens are `<img>` at plate size (source 2720x1596 drawn at 1360x798: source px / 2 = plate px). Layers are `overflow: hidden` divs with an inner `<img>` offset by the crop origin. Crops (source px x, y, w, h): Assistant 0,42,820,1512 · Editor 820,42,1360,1512 · Files 2180,42,540,1512 · drop zone in drag-bottom.png 820,798,1360,756 · docked Files in dropped-bottom.png 820,798,1900,756. Layers get a 1px `#484848` edge; base plate gets a `#141414` side slab 14px thick at translateZ -14px. Fonts `assets/fonts/JetBrainsMono-800.woff2`, `-500`, `-400`. Ground `#1E1E1E`. Headline column on the left: x 120–640, top y 300; kicker uppercase accent 26px tracked 0.14em; h2 lowercase 800 ~92px `#D4D4D4`; lead 400 ~30px `#8E8E8E`. The top-right "2026" label is gone.

Scene 1 (0.0–1.0s): plate holds as handed in. Left column: kicker "01 / docking" hard-cuts in at 0.0; h2 "windows that dock" reveals word by word on 0.0 / 0.25 / 0.5.
Scene 2 (1.0–2.0s): on the 1.0s beat the Files layer lifts off the plate (translateZ 0 → 140px, spring-pop feel, its edge turns `#3B8EEA`), whoosh-short; the base under it shows an empty desktop (#1E1E1E) where Files was. The camera begins a slow zoom-to-target toward the editor's bottom half (scale 1 → 1.18 by 3.0s, counter-translating to keep that region centered) → coordinate-target-zoom.
Scene 3 (2.0–3.0s): the lifted Files layer glides in plate space from its slot over the editor's bottom half (power2.inOut), slightly rotating toward landscape by scaling its box toward the drop zone shape; at 2.0s the base hard-cuts to drag-bottom.png (same empty-desktop patch over the old Files slot) so the real blue drop zone (#253A51 fill, accent border) is visible under the gliding window; the drop zone pulses once at 2.5s.
Scene 4 (3.0–3.5s): DROP on the 3.0s downbeat (click): the floating layer morphs into the docked Files region (card-morph-anchor: scale to the docked rect 820,798,1900,756 and translateZ → 0 in 0.25s) while the base hard-cuts to dropped-bottom.png; a single accent border flash on the docked Files window (0.25s) → card-morph-anchor + spring-pop-entrance.
Scene 5 (3.5–6.0s): camera eases back out to `translate(150px, 20px) scale(1)` by 4.5s; lead line "drag a window to any edge. it docks." reveals under the h2 at 4.0s; everything holds still to 6.0s.

narrativeRole: the first feature, demonstrated with real screens.
keyMessage: drag any window, drop it anywhere, it docks.

## Frame 4 — An assistant built in

- scene: The camera dives into the Assistant's composer: a file chip pops in, the prompt types behind a caret, the send button lights, then the mode and model pickers open on the beat
- voiceover: ""
- duration: 6s
- transition_in: zoom-through
- status: animated
- src: compositions/frames/04-assistant.html
- type: feature_showcase
- persuasion: Friction reduction
- beat: ease + power
- blueprint: prompt-type-submit-generate (Adapt)
- asset_candidates: assets/empty.png — empty composer, placeholder; assets/chip.png — composer with dock.rs chip and typed prompt; assets/chip-mode.png — same with mode picker; assets/chip-model.png — same with model picker
- focal: the composer crop (assets/chip.png)
- roles: empty.png = supporting (opening state) · chip.png = cutout (hero composer) · chip-mode.png = supporting (picker state) · chip-model.png = supporting (picker state)
- sfx: pop, typing, click-soft
- rules: discrete-text-sequence, context-sensitive-cursor, spring-pop-entrance

Adapt: keep the signature (a prompt types into the real product input and steers to the confirming control); the clip ends on the pickers, not on a generated answer (the Assistant isn't connected yet, so never show a reply).

Shared spec: fonts `assets/fonts/JetBrainsMono-800.woff2`, `-500`, `-400`. Ground `#1E1E1E`. All composer states are crops of the same rect of their screenshot: COMPOSER CROP source px x 0, y 1100, w 820, h 454 (it includes the space above the composer where the pickers open). Display the crop at scale 1.3 of source (1066x590 px), as a flat card on the right half: left edge x 760, top y 200, with a 1px `#484848` edge, a `#141414` side slab (offset 10px down-right) for thickness, and a slight perspective tilt (rotateY -10deg, rotateX 6deg around its center, perspective 2400px). States swap by hard cut, always on the beat: empty.png crop → chip.png crop → chip-mode.png crop → chip-model.png crop. Typing illusion on the chip.png crop: the prompt text lives in source rows y 1302–1344 (line 1, x 80–660, 29 chars) and y 1344–1386 (line 2, x 80–620, 27 chars); cover the untyped part of each line with a `#383838` rect (the exact card background) whose left edge steps right one character (20 source px) per typed character, with a 2px `#D4D4D4` caret at the frontier; cover the send button (source x 680–740, y 1386–1428) with a `#2D2D2D` rect with a dim "↑" until it lights. Headline column on the left: x 120–680, top y 300; kicker uppercase accent 26px tracked 0.14em; h2 lowercase 800 ~92px `#D4D4D4`; a lead token line 400 ~30px `#8E8E8E`.

Scene 1 (0.0–1.0s): the zoom-through lands on the empty.png composer card (placeholder "Describe a change…", grey send) with a short push-in settling by 0.75s. Left: kicker "02 / assistant" hard-cuts in at 0.0; h2 "an assistant built in" reveals word by word on 0.25 / 0.5 / 0.75.
Scene 2 (1.0–1.5s): hard cut to the chip.png crop with ALL prompt text covered (only the "dock.rs ×" chip visible) and the chip spring-pops from 0.6 → 1 scale at 1.0s (pop) → spring-pop-entrance.
Scene 3 (1.5–3.5s): the prompt types, line 1 then line 2, 56 characters across 1.5–3.25s, caret leading (typing sfx at 1.5s) → discrete-text-sequence + context-sensitive-cursor. At 3.5s the send cover drops and the accent send button press-pops (click-soft): a quick 0.92 squash and spring back.
Scene 4 (3.5–6.0s): lead token line under the h2 swaps on beats → discrete-text-sequence: at 4.0s hard cut to the chip-mode.png crop and the lead reads "agent, ask or plan"; at 5.0s hard cut to chip-model.png and the lead reads "pick your model". Hold still to 6.0s.

narrativeRole: the AI feature, shown as a Cursor-style composer inside the terminal.
keyMessage: ask about your code without leaving the editor.

## Frame 5 — A new look

- scene: The editor full screen on theme.rs lies flat in the isometric pose; the theme's surfaces peel off above it as labelled color plates, with the menu and search shots resting behind
- voiceover: ""
- duration: 4s
- transition_in: crossfade
- status: animated
- src: compositions/frames/05-look.html
- type: benefit_highlight
- persuasion: Value stacking
- beat: aspiration
- blueprint: grid-card-assemble (Adapt)
- asset_candidates: assets/zoom.png — editor zoomed on theme.rs; assets/menu.png — File menu open; assets/search.png — find bar with match
- focal: assets/zoom.png
- roles: zoom.png = cutout (main plate) · menu.png = background (dimmed ~40%, behind) · search.png = background (dimmed ~40%, behind)
- sfx: whoosh, sparkle
- rules: depth-scatter-assemble, dynamic-content-sequencing, sine-wave-loop

Adapt: keep the staggered self-assembling cascade, but the items are stacked isometric color plates (an exploded view of the theme), not a flat grid.

Shared spec — ISO POSE: a `.iso-stage` full-frame container with `perspective: 4200px; perspective-origin: 50% 45%`; inside it a `.camera` wrapper (absolute, inset 0, `transform-style: preserve-3d`); inside it the `.iso` plate: absolute, `left: 280px; top: 141px; width: 1360px; height: 798px`, `transform-style: preserve-3d; transform: rotateX(54deg) rotateZ(-38deg)`. Screens are `<img>` at plate size (source 2720x1596 at 1360x798). Base plate gets a 1px `#484848` edge and a `#141414` side slab 14px thick. Camera for this frame: `translate(180px, 60px) scale(0.92)`. Fonts `assets/fonts/JetBrainsMono-800.woff2`, `-500`, `-400`. Ground `#1E1E1E`. Headline column left: x 120–620, top y 300; kicker uppercase accent 26px tracked 0.14em; h2 lowercase 800 ~92px `#D4D4D4`; lead 400 ~30px `#8E8E8E`.

Scene 1 (0.0–1.0s): crossfade lands on the zoom.png plate in the ISO POSE; menu.png and search.png as two more plates resting further back and lower (translate within the iso plane by about -900px and +700px along x, translateZ -60px, opacity 0.4). Left: kicker "03 / theme" at 0.0; h2 "a new look" words on 0.25 / 0.5.
Scene 2 (1.0–2.5s): six thin color plates (same footprint as the main plate, 6px thick, 1px `#484848` edge) peel off the main plate and stack upward on quarter-beats, each rising to its own height (translateZ 60, 110, 160, 210, 260, 310px) → depth-scatter-assemble, whoosh at 1.0s: surface `#1E1E1E` (1.0s), panel `#252525` (1.25s), chrome `#2D2D2D` (1.5s), elevated `#383838` (1.75s), border `#4B4B4B` (2.0s), accent `#3B8EEA` (2.25s, sparkle). Plates are semi-transparent (opacity 0.85) so the code below reads through the stack. Each plate gets a label to its right in screen space, aligned to the plate's right corner: index + name in mono uppercase (e.g. "01 surface", "06 accent" in the accent color), with a 1px `#484848` leader line, revealed with its plate.
Scene 3 (2.5–4.0s): lead "every color comes from your terminal." reveals at 2.5s; the stack holds with at most a sub-pixel sine-wave-loop jitter; still to 4.0s.

narrativeRole: one breath on craft: everything derives from your terminal palette.
keyMessage: one accent, your colors, every window.

## Frame 6 — Launch

- scene: Floating plates rush into the center and vanish; the lowercase wordmark "edit" slams in with a blinking accent block cursor; a terminal pill types "$ edit"
- voiceover: ""
- duration: 6s
- transition_in: zoom-through
- status: animated
- src: compositions/frames/06-launch.html
- type: cta
- persuasion: Inevitability
- beat: triumph + urgency-to-act
- blueprint: logo-assemble-lockup (Adapt)
- asset_candidates: assets/empty.png — three-window layout (small plates); assets/zoom.png — editor full screen (small plate)
- focal: the wordmark "edit"
- roles: empty.png = supporting (small collapsing plate) · zoom.png = supporting (small collapsing plate)
- sfx: whoosh, impact-bass-2, key-press
- rules: kinetic-beat-slam, discrete-text-sequence

Adapt: keep the signature (the mark comes to exist from parts and resolves into a centered lockup that extends to the CTA): the parts are real window plates that implode into the wordmark.

Shared spec: fonts `assets/fonts/JetBrainsMono-800.woff2`, `-500`, `-400`. Ground `#1E1E1E`. Plates are small tilted screenshots (about 420px wide, `rotateX(54deg) rotateZ(-38deg)` with perspective 4200px), 1px `#484848` edge, `#141414` side slab.

Scene 1 (0.0–1.0s): four small plates (empty.png, zoom.png, and crops of the Assistant 0,42,820,1512 and Files 2180,42,540,1512 from empty.png) arrive from the frame edges and rush into the center, shrinking and speeding up (an inward implosion, accelerating); whoosh at 0.25s. They converge to a point at 1.0s and vanish.
Scene 2 (1.0–2.0s): on the 1.0s downbeat (impact-bass-2) the wordmark "edit" slams in centered (display, lowercase 800, ~300px, `#D4D4D4`, tracking -0.04em) with a solid `#3B8EEA` block cursor right after the "t" (cell sized to the type, blinking on every beat from 2.0s) → kinetic-beat-slam. A single 2px accent rule stub (48px wide) sits above the wordmark.
Scene 3 (2.0–3.5s): kicker above the wordmark "1991 → 2026" (uppercase, accent, tracked) at 2.0s; lead below "now in your terminal." (400, ~40px, `#8E8E8E`) at 2.5s.
Scene 4 (3.5–5.5s): a terminal pill (1px `#484848` border, `#232323` fill, square corners, ~460px wide) appears below the lead at 3.5s and types "$ edit" one key per quarter-beat (key-press at 3.5s) behind a blinking caret → discrete-text-sequence; the "$" in `#8E8E8E`, "edit" in `#D4D4D4`. Then everything holds still.
Scene 5 (5.5–6.0s): final exit: the whole lockup fades to the ground (opacity → 0 by 6.0s) except the accent block cursor, which stays lit until the last frame.

narrativeRole: name it and hand over the action.
keyMessage: Edit. Now in your terminal.
