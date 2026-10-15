# Posts: brief for the publishing agent

Social posts announcing recent work on Pavel Sich's fork of Microsoft Edit,
the terminal text editor written in Rust.

| File | Channel | Notes |
|---|---|---|
| `linkedin.md` | LinkedIn | One post, roughly 1,300 characters |
| `x.md` | X (Twitter) | A single post and a 5-post thread. Each post is under 280 characters |
| `generic.md` | Blog, Mastodon, Reddit, dev.to, newsletters | Longer write-up with headings |

## Facts the posts rely on

- Repository: https://github.com/sichy/edit, a fork of https://github.com/microsoft/edit
- Language: Rust. The UI is a terminal UI and runs in any terminal.
- **Window layout and docking** (commits `5255087`, `a5b18e5`, `aefd143`):
  - The Editor, AI Assistant and File Browser are framed windows in the style of Turbo Vision and Borland C++.
  - The active window has a double-line frame. Title bars have `[■]` close and `[↑]` zoom buttons. Floating windows have a `◢` resize grip.
  - Drag a title bar to move a window. Drop it on a screen edge to dock it along that edge. Drop it near the side of another window to split that window. Drop it on the middle of another window to swap the two.
  - A live outline previews where the window will land.
  - Drag the line between docked windows to resize them. Floating windows resize from their edges and corner.
  - Windows can float above the layout. Use View > Float Window or Dock Window.
  - Keyboard: F6 and Shift+F6 cycle windows, F5 zooms, Ctrl+F5 moves and resizes with the arrow keys, Alt+F3 closes the active window.
  - The layout is saved and restored on the next start.
- **AI Assistant** (commit `964999c`):
  - Chat-style window: a header with a "New" chat button, a scrollable message history with word-wrapped messages, and a composer.
  - The composer is multi-line and grows up to 6 lines. Enter sends, Alt+Enter inserts a newline, and it shows placeholder text.
  - It's a dockable window like the others; it defaults to the left edge.
  - **It is NOT connected to an AI model yet.** Replies are placeholders. The posts present it as "the UI is in, the model integration is next". Do not claim it answers questions, writes code or uses any specific model.
- **Syntax highlighting:**
  - Uses Edit's LSH highlighting engine, merged from upstream Microsoft Edit. It has definitions for about 30 languages, including Rust, C/C++, C#, F#, Go, Java, JavaScript, Python, Ruby, Lua, Zig, Odin, SQL, PowerShell, shell scripts, JSON, YAML, TOML, XML and Markdown.
  - The fork adds its own modern dark colour theme on top, e.g. blue strings and green-yellow functions and methods.
  - The fork had its own highlighting before upstream shipped theirs. It now builds on the upstream engine.

## Rules for publishing

- Don't overstate the AI Assistant (see above).
- Credit Microsoft Edit as the upstream project.
- Use screenshots or a short screen recording if possible. The docking drag-and-drop is the most visual feature.
  - Take them in a dark terminal at about 140×40 or larger.
  - Suggested shots: the default layout; dragging a window with the outline preview visible; a floating window over the docked ones; the AI Assistant chat.
  - The existing hero image is `assets/edit_hero_image.png`. It is older and doesn't show the new windows.
- The `[■]`, `[↑]` and `◢` characters may not render on every platform. Remove them if the preview looks broken.
