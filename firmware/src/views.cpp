#include "views.h"

#include <time.h>

#include "brain.h"
#include "config.h"

namespace {

constexpr uint16_t kDotOff = 0x1082;   // unlit LED
constexpr uint16_t kDotLow = 0x39E7;   // half-lit (labels, past rounds)
constexpr int W = 320;

// 5x7 glyphs, one byte per row, bit 4 = leftmost column.
struct Glyph {
  char c;
  uint8_t rows[7];
};
const Glyph kFont[] = {
    {'0', {0x0E, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0E}}, {'1', {0x04, 0x0C, 0x04, 0x04, 0x04, 0x04, 0x0E}},
    {'2', {0x0E, 0x11, 0x01, 0x02, 0x04, 0x08, 0x1F}}, {'3', {0x1F, 0x02, 0x04, 0x02, 0x01, 0x11, 0x0E}},
    {'4', {0x02, 0x06, 0x0A, 0x12, 0x1F, 0x02, 0x02}}, {'5', {0x1F, 0x10, 0x1E, 0x01, 0x01, 0x11, 0x0E}},
    {'6', {0x06, 0x08, 0x10, 0x1E, 0x11, 0x11, 0x0E}}, {'7', {0x1F, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08}},
    {'8', {0x0E, 0x11, 0x11, 0x0E, 0x11, 0x11, 0x0E}}, {'9', {0x0E, 0x11, 0x11, 0x0F, 0x01, 0x02, 0x0C}},
    {'A', {0x0E, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11}}, {'B', {0x1E, 0x11, 0x11, 0x1E, 0x11, 0x11, 0x1E}},
    {'C', {0x0E, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0E}}, {'D', {0x1C, 0x12, 0x11, 0x11, 0x11, 0x12, 0x1C}},
    {'E', {0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x1F}}, {'F', {0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x10}},
    {'G', {0x0E, 0x11, 0x10, 0x17, 0x11, 0x11, 0x0F}}, {'H', {0x11, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11}},
    {'I', {0x0E, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0E}}, {'J', {0x07, 0x02, 0x02, 0x02, 0x02, 0x12, 0x0C}},
    {'K', {0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11}}, {'L', {0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1F}},
    {'M', {0x11, 0x1B, 0x15, 0x15, 0x11, 0x11, 0x11}}, {'N', {0x11, 0x11, 0x19, 0x15, 0x13, 0x11, 0x11}},
    {'O', {0x0E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E}}, {'P', {0x1E, 0x11, 0x11, 0x1E, 0x10, 0x10, 0x10}},
    {'R', {0x1E, 0x11, 0x11, 0x1E, 0x14, 0x12, 0x11}}, {'S', {0x0F, 0x10, 0x10, 0x0E, 0x01, 0x01, 0x1E}},
    {'T', {0x1F, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04}}, {'U', {0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E}},
    {'V', {0x11, 0x11, 0x11, 0x11, 0x11, 0x0A, 0x04}}, {'W', {0x11, 0x11, 0x11, 0x15, 0x15, 0x15, 0x0A}},
    {'Y', {0x11, 0x11, 0x0A, 0x04, 0x04, 0x04, 0x04}}, {'-', {0x00, 0x00, 0x00, 0x1F, 0x00, 0x00, 0x00}},
};

const uint8_t *glyph(char c) {
  for (const Glyph &g : kFont)
    if (g.c == c) return g.rows;
  return nullptr;
}

int cols(char c) { return c == ':' ? 1 : (c == ' ' ? 3 : 5); }

void dot(TFT_eSprite &s, int x, int y, int d, uint16_t col) {
  if (d <= 2) s.fillRect(x, y, d, d, col);
  else s.fillRoundRect(x, y, d, d, d / 3, col);
}

// Row of LED dots acting as a progress bar.
void dotBar(TFT_eSprite &s, int x, int y, int w, int pitch, float frac, uint16_t on) {
  const int n = w / pitch;
  const int lit = static_cast<int>(n * constrain(frac, 0.0f, 1.0f) + 0.5f);
  const int d = max(2, pitch * 3 / 5);
  for (int i = 0; i < n; i++) dot(s, x + i * pitch, y, d, i < lit ? on : kDotOff);
}

void fmtMMSS(char *buf, size_t n, uint32_t ms) {
  uint32_t secs = (ms + 999) / 1000;
  if (secs >= 6000) snprintf(buf, n, "%u:%02u", static_cast<unsigned>(secs / 3600), static_cast<unsigned>(secs / 60 % 60));
  else snprintf(buf, n, "%02u:%02u", static_cast<unsigned>(secs / 60), static_cast<unsigned>(secs % 60));
}

// Big centred dot-matrix readout.
void bigDots(TFT_eSprite &s, const char *txt, int y, bool colonOn) {
  constexpr int kPitch = 11;
  const int w = dotTextWidth(txt, kPitch);
  dotText(s, txt, (W - w) / 2, y, kPitch, COL_MONO, kDotOff, colonOn);
}

}  // namespace

int dotTextWidth(const char *txt, int pitch) {
  int c = 0;
  for (const char *p = txt; *p; p++) c += cols(*p) + (p[1] ? 1 : 0);
  return c * pitch;
}

void dotText(TFT_eSprite &s, const char *txt, int x, int y, int pitch, uint16_t on, uint16_t off, bool colonOn) {
  const int d = max(2, pitch * 4 / 5);
  for (const char *p = txt; *p; p++) {
    const char ch = toupper(*p);
    if (ch == ':') {
      for (int r = 0; r < 7; r++) {
        const bool lit = colonOn && (r == 2 || r == 4);
        if (lit || off != TFT_BLACK) dot(s, x, y + r * pitch, d, lit ? on : off);
      }
    } else if (ch != ' ') {
      const uint8_t *g = glyph(ch);
      for (int r = 0; r < 7; r++)
        for (int c = 0; c < 5; c++) {
          const bool lit = g && (g[r] >> (4 - c)) & 1;
          if (lit || off != TFT_BLACK) dot(s, x + c * pitch, y + r * pitch, d, lit ? on : off);
        }
    }
    x += (cols(ch) + 1) * pitch;
  }
}

// ---------------------------------------------------------------- clock

void drawClockView(TFT_eSprite &s, const Brain &brain, uint32_t now) {
  struct tm t;
  const bool ok = getLocalTime(&t, 0);
  char buf[24];

  // Header: AM/PM (left), day + date (right), tiny dots.
  if (ok) {
    dotText(s, t.tm_hour < 12 ? "AM" : "PM", 18, 10, 3, COL_MONO, TFT_BLACK);
    strftime(buf, sizeof(buf), "%a %d %b", &t);
    for (char *p = buf; *p; p++) *p = toupper(*p);
    dotText(s, buf, W - 18 - dotTextWidth(buf, 3), 10, 3, kDotLow, TFT_BLACK);
  }

  // 12h time, blinking colon.
  if (ok) snprintf(buf, sizeof(buf), "%02d:%02d", (t.tm_hour % 12) ? t.tm_hour % 12 : 12, t.tm_min);
  else strcpy(buf, "--:--");
  bigDots(s, buf, 36, !ok || (now / 500) % 2 == 0);

  // Seconds ticker.
  if (ok) dotBar(s, 18, 124, W - 36 - 92, 5, t.tm_sec / 59.0f, kDotLow);

  // Mood pill, bottom right.
  String m = moodName(brain.currentMood());
  if (m.length()) m.setCharAt(0, toupper(m[0]));
  s.setFreeFont(&FreeSansBold9pt7b);
  const int pw = s.textWidth(m) + 20, px = W - 16 - pw, py = 116;
  s.fillSmoothRoundRect(px, py, pw, 22, 11, COL_MONO, TFT_BLACK);
  s.setTextColor(TFT_BLACK);
  s.setTextDatum(MC_DATUM);
  s.drawString(m, px + pw / 2, py + 11);
}

// ---------------------------------------------------------------- pomodoro / timer

void drawFocusView(TFT_eSprite &s, const Brain &brain, uint32_t now) {
  char buf[16];
  const bool timerShown = brain.timerOn() || brain.timerDoneRecently(now);
  const bool blink = (now / 500) % 2 == 0;

  if (timerShown) {
    const bool done = !brain.timerOn();
    dotText(s, done ? "DONE" : "TIMER", 18, 10, 3, COL_MONO, TFT_BLACK);
    fmtMMSS(buf, sizeof(buf), done ? 0 : brain.timerRemainMs(now));
    if (!done || blink) bigDots(s, buf, 36, true);
    else bigDots(s, "     ", 36, false);
    const float frac = done ? 1.0f : 1.0f - static_cast<float>(brain.timerRemainMs(now)) / max<uint32_t>(1, brain.timerTotalMs());
    dotBar(s, 18, 128, W - 36, 6, frac, COL_MONO);
    return;
  }

  const PomoPhase ph = brain.pomoPhase();
  const bool active = ph != PomoPhase::Off;
  dotText(s, !active ? "READY" : ph == PomoPhase::Focus ? "FOCUS" : "BREAK", 18, 10, 3, COL_MONO, TFT_BLACK);

  // Round markers: four per set, filled = done.
  const int round = active ? brain.pomoRound() : 1;
  const int doneInSet = (round - 1) % 4;
  for (int i = 0; i < 4; i++) {
    const int x = W - 18 - 9 - (3 - i) * 14;
    const bool filled = i < doneInSet || (ph == PomoPhase::Break && i == doneInSet);
    if (filled) s.fillSmoothCircle(x + 4, 20, 4, COL_MONO, TFT_BLACK);
    else s.drawCircle(x + 4, 20, 4, kDotLow);
  }

  const uint32_t remain = active ? brain.pomoRemainMs(now) : brain.pomoFocusMs();
  fmtMMSS(buf, sizeof(buf), remain);
  bigDots(s, buf, 36, !active || ph == PomoPhase::Break || blink);
  const float frac = active ? 1.0f - static_cast<float>(remain) / max<uint32_t>(1, brain.pomoTotalMs()) : 0.0f;
  dotBar(s, 18, 128, W - 36, 6, frac, ph == PomoPhase::Break ? kDotLow : COL_MONO);
}
