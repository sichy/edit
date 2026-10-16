"""Writes audio_meta.json: the host music bed plus every sound effect at its on-beat offset.

All offsets are frame-local seconds on the 120 BPM grid (one beat = 0.5s).
Run from the project root after assets/bgm/track.wav exists.
"""

import json
import shutil
from pathlib import Path

LIBRARY = Path.home() / ".agents/skills/media-use/audio/assets/sfx"
MANIFEST = json.loads((LIBRARY / "manifest.json").read_text())

# (frame, cue, frame-local offset in seconds, volume)
CUES = [
    (1, "key-press", 0.5, 0.35),
    (1, "typing", 0.6, 0.25),
    (1, "glitch-1", 3.0, 0.5),
    (2, "impact-bass-1", 0.0, 0.6),
    (2, "whoosh", 0.5, 0.35),
    (2, "whoosh-short", 1.5, 0.3),
    (2, "click", 3.0, 0.3),
    (3, "whoosh-short", 1.0, 0.35),
    (3, "click", 3.0, 0.45),
    (4, "pop", 1.0, 0.35),
    (4, "typing", 1.5, 0.3),
    (4, "click-soft", 3.5, 0.4),
    (4, "click", 4.0, 0.3),
    (4, "click", 5.0, 0.3),
    (5, "whoosh", 1.0, 0.3),
    (5, "sparkle", 2.25, 0.3),
    (6, "whoosh", 0.25, 0.35),
    (6, "impact-bass-1", 1.0, 0.6),
    *[(6, "key-press", 3.5 + 0.25 * i, 0.3) for i in range(6)],
]

sfx_dir = Path("assets/sfx")
sfx_dir.mkdir(parents=True, exist_ok=True)
sfx = []
for frame, cue, offset, volume in CUES:
    entry = MANIFEST[cue]
    shutil.copyfile(LIBRARY / entry["file"], sfx_dir / entry["file"])
    sfx.append({
        "frame": frame,
        "file": f"assets/sfx/{entry['file']}",
        "offset_s": offset,
        "duration_s": entry["duration"],
        "volume": volume,
        "source": "host",
    })

meta = {
    "bgm": {"path": "assets/bgm/track.wav", "volume": 0.85, "source": "host"},
    "bgm_pending": False,
    "voices": [],
    "sfx": sfx,
}
Path("audio_meta.json").write_text(json.dumps(meta, indent=2) + "\n")
print(f"audio_meta.json: bgm + {len(sfx)} sfx cues")
