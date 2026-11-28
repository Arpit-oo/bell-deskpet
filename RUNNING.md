# Running Bell

Bell = ESP32 robot (`firmware/`) + Windows companion app (`companion/`, `bell.exe`).
The companion watches what you do on the PC and pushes mood/state (relax, happy,
focus, music, Claude status...) to the robot over WiFi. **No WiFi link = robot
shows the default face only.**

## 1. Turn on the hotspot (every boot)

The robot joins the Windows Mobile Hotspot (SSID `BELLA 7347`, subnet `192.168.137.x`).

```powershell
powershell -ExecutionPolicy Bypass -File C:\code\bell\tools\hotspot.ps1
```

Or: Settings → Network → Mobile hotspot → On. Then power the robot; give it ~10 s to join.

Check it joined:

```powershell
arp -a | findstr 192.168.137
```

You should see one `192.168.137.x` entry (ESP32 MAC `70-4b-ca-...`).

## 2. Start the companion

```powershell
Start-Process C:\code\bell\companion\target\release\bell.exe
```

Options: `--host <ip>` (default `192.168.137.243`, env `DESKPET_HOST`), `--name Arpit`, `--no-overlay`.

If the robot isn't at the default IP, the companion finds it on its own: it scans
the ARP table for `192.168.137.x` neighbours every 15 s while offline.

- Desktop pet notch shows up at the top of the screen.
- **Double-click the notch** (or right-click → Open Bell app) → native Bell window
  (`bell.exe --app`, frameless WebView2, same black/white look). Browser fallback: http://localhost:7777
- **Click the notch** → robot screen cycles: Face → Spotify → Clock → Focus → Life in weeks. Manual pick sticks 2 min, then auto.
- **Right-click the notch** → Timer (1m…1h, +1m, stop), Pomodoro (start/skip/stop), Robot screen (pick any, Full screen toggle).
- App Pet page: same screens, timer card, Full screen switch (hides the robot's bottom strip; saved on the robot).
- Life in weeks uses birthday `BIRTH_Y/M/D` in `firmware/src/config.h`.

Robot screen API: `POST /api/view {"view":"next"|"face"|"music"|"clock"|"focus"|"life"|"auto"}`, `POST /api/timer {"action":"start"|"add"|"stop","seconds":N}`, `POST /api/settings {"fullscreen":true}`.

## 3. Check it's connected

```powershell
Get-Content "$env:TEMP\deskpet-companion.log" -Tail 5
```

Good: `pet connected at http://192.168.137.x`.
Bad: `pet unreachable ...` → hotspot is off or the robot isn't powered/joined (go back to step 1).

## Build

```powershell
cd C:\code\bell\companion
taskkill /F /IM bell.exe      # running exe locks the file
cargo build --release
```

## Firmware

```powershell
cd C:\code\bell\firmware
pio run -e deskpet -t upload          # USB, COM10 @ 57600
pio run -e deskpet_ota -t upload      # OTA over the hotspot (password deskpet)
```

OTA targets `192.168.137.243` by default. If the robot got another IP, run with
`--upload-port 192.168.137.x`.

First boot / new WiFi: the robot opens the WiFiManager portal AP. Join it from
your phone and enter the hotspot SSID and password.

## Claude Code hooks

See README: hooks call `bell.exe --claude working|waiting|done`.

## Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| Face shows, never changes mood | Robot offline | Hotspot on (step 1), check the log |
| `pet unreachable` forever | Robot not on the hotspot | `arp -a`, power-cycle the robot |
| `cargo build` "Access is denied" | bell.exe still running | `taskkill /F /IM bell.exe` |
| OTA fails | IP changed | Pass `--upload-port` with the IP from `arp -a` |
