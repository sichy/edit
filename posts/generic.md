# Bringing Turbo Vision windows back: docking, an AI Assistant and syntax highlighting in a Rust terminal editor

For a while now I've been working on my fork of [Microsoft Edit](https://github.com/microsoft/edit), the small, fast terminal text editor written in Rust. The latest round of changes is about how the editor feels to work in. It brings back the window management of the 90s Borland IDEs, adds a chat-style AI Assistant window, and moves syntax highlighting onto the upstream engine with a modern dark theme.

Code: https://github.com/sichy/edit

## Windows and docking, Turbo Vision style

If you used Borland C++ or Turbo Pascal, this will look familiar. The Editor, the File Browser and the AI Assistant are now real windows:

- **Classic frames.** The active window has a double-line border, inactive ones a single line. The title bar has a `[■]` close button and a `[↑]` zoom button. Floating windows get a `◢` resize grip.
- **Drag and drop docking.** Drag a window by its title bar:
  - drop it on a screen edge to dock it along that whole edge;
  - drop it near the side of another window to split that window;
  - drop it on the middle of another window to swap the two.

  A live outline shows exactly where the window will land.
- **Resizing.** Drag the line between two docked windows to resize both. Floating windows resize from their edges or from the grip.
- **Floating windows.** Any window can float above the docked layout (View > Float Window) and dock again later (View > Dock Window).
- **Zoom.** Double-click a title bar or press F5 to make a window fill the screen.
- **Keyboard first.** The shortcuts are the classic ones:
  - F6 and Shift+F6 cycle through windows;
  - Ctrl+F5 enters move/size mode, where the arrow keys move a window and Shift+arrows resize it;
  - Alt+F3 closes the active window.
- **Persistent layout.** The arrangement is saved when you change it and restored when the editor starts.

Under the hood, this is a small window manager built into Edit's immediate-mode terminal UI. Docked windows form a tree of horizontal and vertical splits, each storing its share of the space, so layouts scale cleanly when the terminal is resized. Floating windows sit above the tree in their own z-order.

## An AI Assistant window

The AI Assistant is now a chat-style window:

- a header with the chat title and a **New** button;
- a scrollable message history, with word-wrapped prompts and replies;
- a composer with a multi-line prompt that grows up to six lines. Enter sends, Alt+Enter adds a new line.

Like every other window, it can be docked anywhere, floated or zoomed. To be clear about where it stands: the interface is done, but it isn't connected to an AI model yet, so replies are placeholders for now. Wiring up a model is the next step.

## Syntax highlighting

My fork had its own syntax highlighting before upstream Edit shipped one. Highlighting now runs on Edit's LSH engine, merged from upstream. It has definitions for around 30 languages, including:

- **Systems:** Rust, C/C++, Zig, Odin
- **.NET and JVM:** C#, F#, Java
- **Scripting:** Go, Python, Ruby, Lua, JavaScript, PowerShell, shell scripts
- **Data and config:** SQL, JSON, YAML, TOML, XML, Markdown

On top of the engine sits my own modern dark colour theme.

## Try it

```sh
git clone https://github.com/sichy/edit
cd edit
cargo run --release -- path/to/file.rs
```

Building it needs a recent stable Rust toolchain. Thanks to the Microsoft Edit team for a great foundation to build on.

---

## Notes for the publisher

- Suggested images:
  - the default layout with all three windows;
  - a window being dragged, with the docking outline visible;
  - a floating window over the docked ones;
  - the AI Assistant chat.
- For Reddit (e.g. r/rust or r/commandline), use the title above and trim the "Under the hood" paragraph, or keep it for r/rust.
- For Mastodon, the single X post from `x.md` works as-is (500-character limit).
