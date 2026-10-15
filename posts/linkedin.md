# LinkedIn post

---

Remember dragging windows around in Borland C++ or Turbo Pascal? I brought that back, in a modern Rust terminal editor.

For a while now I've been working on my fork of Microsoft Edit, the small terminal text editor written in Rust. This round of changes is all about how the editor feels to work in:

🪟 Turbo Vision style windows and docking
The Editor, the File Browser and the new AI Assistant are now real windows, with double-line frames, close and zoom buttons on the title bar, and a resize grip. Drag a title bar and drop the window on a screen edge to dock it, next to another window to split it, or on top of one to swap the two. A live outline shows where it will land. Drag the lines between windows to resize them, float any window above the layout, or zoom it to full screen. The classic keys work too: F6 to cycle windows, F5 to zoom, Ctrl+F5 to move and size with the keyboard. Your layout is saved between sessions.

💬 An AI Assistant window
A chat-style panel with message history, a multi-line prompt and a "New chat" button, dockable anywhere like every other window. The UI is in place; connecting it to a model is the next step.

🎨 Syntax highlighting
The editor now uses the upstream LSH highlighting engine, which covers about 30 languages (Rust, C#, Go, Python, JavaScript, SQL, YAML and more), with my own modern dark theme on top.

It's been a fun mix of nostalgia and modern engineering: a tiling and floating window manager built into an immediate-mode terminal UI.

Code is on GitHub: https://github.com/sichy/edit
Built on top of Microsoft Edit: https://github.com/microsoft/edit

Which old-school developer tool do you miss the most?

#Rust #OpenSource #TerminalUI #DeveloperTools #TurboVision #RetroComputing

---

## Notes for the publisher

- Attach a screenshot or a short GIF of dragging a window, with the docking outline visible.
- If emojis are unwanted, drop them; the section titles still work as plain lines.
