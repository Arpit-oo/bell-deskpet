# Bell: desk pet

Bell is a little robot face that lives in two places:

- **On your desktop.** A small floating window that stays on top of everything. It has animated eyes, speech bubbles, sings along to Spotify, and shows a badge while Claude Code is working. It's a native Rust app with no Electron, idles at about 50 MB, and draws in sync with the compositor so the animation stays smooth.
- **On your desk.** An ESP32 with a 2.8" ILI9341 screen running the same face, updated over WiFi.

The desktop app also runs the robot's companion: it watches what you're doing (focused app, Spotify via the Windows media session, idle time) and streams that to the robot. It also serves an everyday planner at `http://localhost:7777`.

```
companion/   Rust app: desktop overlay + companion + planner (bell.exe)
firmware/    ESP32 firmware (PlatformIO)
tools/       helper scripts
```

## Desktop pet

```
cd companion
cargo build --release
target\release\bell.exe              # pet on screen + robot companion
target\release\bell.exe --install    # start at login
target\release\bell.exe --no-overlay # robot only, no desktop pet
```

| Do this | Bell does |
|---|---|
| Drag | moves it (position is remembered) |
| Click | gets petted (hearts / happy / excited) |
| Click a bubble | dismisses it |
| Double-click | opens the planner app |
| Right-click | menu: size, lyrics, eyes-follow-mouse, move to corner, hide 30 min, quit |

- The eyes follow your mouse, blink, glance around, and play small idle animations (look around, curious, wiggle, peek, yawn).
- The mood follows what you're doing: focus while coding, music, sleepy when you're away, sus after 20 min of YouTube, and so on.
- Hype lines, reminders and habit nudges appear as speech bubbles with a typewriter effect.
- With Spotify playing, it lip-syncs to synced lyrics (LRCLIB) with floating notes, shows the lyric line, and hums when a song has no lyrics.
- With Claude Code hooks set up, an orange spinner shows while Claude works and "Claude needs you" appears when it's waiting.
- It fades out when a fullscreen app (game, video) is in front.
- Transparent pixels are click-through, so it never blocks what's behind it.

Settings: `%APPDATA%\deskpet\overlay.json`. Planner data: `%APPDATA%\deskpet\data.json`. Log: `%TEMP%\deskpet-companion.log`.

### Claude Code hooks

In `~/.claude/settings.json`, point `UserPromptSubmit`/`PostToolUse` at `bell.exe --claude working`, `Notification` at `--claude waiting` and `Stop` at `--claude done`. Each hook returns instantly; the HTTP call runs in a detached child process.

## Robot

### Hardware

ESP32-D0WD + 2.8" ILI9341 SPI TFT (no touch).

Wiring: VCC+LED → 3V3, GND, CS → 5, RST → 33, DC → 27, MOSI → 23, SCK → 18, MISO → 19.
Display: `ILI9341_2_DRIVER`, SPI 20 MHz, BGR, `invertDisplay(true)`, landscape.

### Flash

```
cd firmware
pio run -e deskpet_ota -t upload    # over WiFi
pio run -e deskpet -t upload        # USB: hold BOOT, tap EN; 57600 baud
```

The OTA host is set in `platformio.ini`. The OTA password is `PET_OTA_PASSWORD` in `src/config.h`; change it.

### First run

With no saved WiFi, the robot opens a `DeskPet-Setup` access point. Join it and enter your WiFi at 192.168.4.1. If your network isolates clients (common on campus networks), use a Windows Mobile Hotspot instead.

On boot the info panel shows once. After that the face takes the whole screen, and the panel slides back in only when there's something to show: a hype line, Claude waiting, a pomodoro, lyrics, or a new app or task.

### HTTP API (port 80, JSON with `Content-Type: application/json`)

- `GET /` phone control page (themes, colours, moods, pomodoro)
- `GET /api/status`, `GET|POST /api/settings`
- `POST /api/state` companion update (activity, music, lyrics, task, mood, say)
- `POST /api/say` `{"text":"LET'S GO","mood":"excited","ms":6000}`
- `POST /api/mood` `{"mood":"love","ms":5000}`: idle happy excited focus music sleepy sus sad angry surprised love
- `POST /api/anim` `{"name":"lookaround|curious|wiggle|yawn|peek"}`
- `POST /api/claude` `{"state":"working|waiting|done|none","project":"bell"}`
- `POST /api/pomodoro` `{"action":"start|stop|skip"}`
- `POST /api/art` base64 56x56 RGB565 album art
