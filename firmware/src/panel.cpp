#include "panel.h"

#include <time.h>

#include "config.h"
#include "settings.h"

namespace {

constexpr uint16_t kClaude = 0xD3AA;  // Claude orange (217,119,87)

String fmtMinutes(int m) {
  if (m < 60) return String(m) + " min";
  char buf[16];
  snprintf(buf, sizeof(buf), "%dh %02dm", m / 60, m % 60);
  return String(buf);
}

const GFXfont *const kBigFonts[] = {&FreeSansBold12pt7b, &FreeSansBold9pt7b};

}  // namespace

Panel::Panel(TFT_eSPI &tft, NowPlaying &music) : tft_(tft), music_(music), spr_(&tft) {}

bool Panel::begin() {
  spr_.setColorDepth(8);
  ok_ = spr_.createSprite(PANEL_W, PANEL_H) != nullptr;
  dirty_ = true;
  return ok_;
}

void Panel::setActivity(const String &label, const String &category, int minutes) {
  const String l = asciiOnly(label);
  if (l != actLabel_ || category != actCat_ || minutes != actMinutes_) {
    if (l != actLabel_ && l.length()) peek(5000);
    actLabel_ = l;
    actCat_ = category;
    actMinutes_ = minutes;
    dirty_ = true;
  }
}

void Panel::setHelperOnline(bool online) {
  if (online != helperOnline_) {
    helperOnline_ = online;
    dirty_ = true;
  }
}

void Panel::setShowLyrics(bool on) {
  if (on != showLyrics_) {
    showLyrics_ = on;
    dirty_ = true;
  }
}

void Panel::say(const String &text, uint32_t durationMs) {
  const String t = asciiOnly(text);
  if (!t.length()) return;
  wrap(t, kBigFonts, 2, PANEL_W - 24, sayLines_, sayCount_, sayFont_);
  sayStart_ = millis();
  sayUntil_ = sayStart_ + durationMs;
  dirty_ = true;
}

void Panel::setTask(const String &title, int estMin, int left) {
  const String t = asciiOnly(title);
  if (t != taskTitle_ || estMin != taskEst_ || left != tasksLeft_) dirty_ = true;
  if (t != taskTitle_ && t.length()) peek(5000);
  taskTitle_ = t;
  taskEst_ = estMin;
  tasksLeft_ = left;
}

void Panel::drawTaskLine(int y) {
  if (!taskTitle_.length()) return;
  String line = "NEXT  " + taskTitle_;
  if (taskEst_ > 0) line += "  ~" + String(taskEst_) + "m";
  spr_.setTextFont(1);
  while (line.length() > 4 && spr_.textWidth(line) > PANEL_W - 60) line.remove(line.length() - 1);
  spr_.setTextDatum(BL_DATUM);
  spr_.setTextColor(COL_MONO);
  spr_.drawString(line, 12, y);
  if (tasksLeft_ > 1) {
    spr_.setTextDatum(BR_DATUM);
    spr_.setTextColor(COL_SUBTLE);
    spr_.drawString("+" + String(tasksLeft_ - 1) + " more", PANEL_W - 10, y);
  }
}

void Panel::setPomodoro(bool active, bool focus, uint32_t remainingMs, uint32_t totalMs, int round) {
  const uint32_t sec = remainingMs / 1000;
  if (active != pomoActive_ || focus != pomoFocus_ || sec != lastPomoSec_ || round != pomoRound_) dirty_ = true;
  pomoActive_ = active;
  pomoFocus_ = focus;
  pomoRemain_ = remainingMs;
  pomoTotal_ = max<uint32_t>(1, totalMs);
  pomoRound_ = round;
  lastPomoSec_ = sec;
}

void Panel::setClaude(ClaudeState st, const String &project) {
  const String p = asciiOnly(project);
  if (st != claude_ || p != claudeProject_) dirty_ = true;
  claude_ = st;
  claudeProject_ = p;
}

void Panel::wrap(const String &s, const GFXfont *const *fonts, int nFonts, int maxW, String lines[2], int &count,
                 const GFXfont *&font) {
  for (int fi = 0; fi < nFonts; fi++) {
    spr_.setFreeFont(fonts[fi]);
    String out[3];
    int n = 0;
    String cur;
    int start = 0;
    while (start < static_cast<int>(s.length())) {
      int sp = s.indexOf(' ', start);
      if (sp < 0) sp = s.length();
      const String word = s.substring(start, sp);
      const String trial = cur.length() ? cur + " " + word : word;
      if (spr_.textWidth(trial) <= maxW || cur.length() == 0) {
        cur = trial;
      } else {
        if (n < 3) out[n++] = cur;
        cur = word;
      }
      start = sp + 1;
    }
    if (cur.length() && n < 3) out[n++] = cur;

    const bool last = fi == nFonts - 1;
    if (n <= 2 || last) {
      font = fonts[fi];
      count = min(n, 2);
      lines[0] = out[0];
      lines[1] = count > 1 ? out[1] : String();
      if (n > 2) {
        String l2 = lines[1];
        while (l2.length() > 3 && spr_.textWidth(l2 + "...") > maxW) l2.remove(l2.length() - 1);
        lines[1] = l2 + "...";
      }
      return;
    }
  }
}

uint16_t Panel::categoryColor(const String &cat) {
  if (cat == "code") return COL_EYE_ICE;
  if (cat == "video") return COL_EYE_ANGRY;
  if (cat == "social") return COL_EYE_LOVE;
  if (cat == "chat") return COL_TEXT;
  if (cat == "music") return COL_EYE_MUSIC;
  if (cat == "game") return COL_TEXT;
  if (cat == "browse") return COL_TEXT;
  if (cat == "away") return COL_EYE_SLEEPY;
  return COL_SUBTLE;
}

String Panel::activitySubline(const String &cat, int m) {
  if (cat == "away") return "Away. Resting my eyes...";
  if (cat == "music") return "Vibing";
  if (cat == "code") return m < 1 ? String("Just started. Let's cook.") : fmtMinutes(m) + " locked in";
  if (m < 1) return "Just opened";
  if (cat == "video") return fmtMinutes(m) + " watching";
  if (cat == "social") return fmtMinutes(m) + " scrolling";
  if (cat == "chat") return fmtMinutes(m) + " chatting";
  if (cat == "game") return fmtMinutes(m) + " gaming";
  if (cat == "browse") return fmtMinutes(m) + " browsing";
  return fmtMinutes(m);
}

Panel::Mode Panel::currentMode(uint32_t now) const {
  if (claude_ == ClaudeState::Waiting) return Mode::Claude;
  if (now < sayUntil_) return Mode::Say;
  if (pomoActive_) return Mode::Pomodoro;
  if (music_.playing() && music_.hasTrack()) return Mode::Music;
  if (helperOnline_ && actLabel_.length()) return Mode::Activity;
  return Mode::Clock;
}

void Panel::drawEq(int x, int baseY, int bars, int barW, int maxH, uint16_t col) {
  const uint32_t now = millis();
  if (now >= eqNext_) {
    for (float &t : eqT_) t = 0.15f + (esp_random() % 85) / 100.0f;
    eqNext_ = now + 110;
  }
  for (int i = 0; i < bars && i < 12; i++) {
    eq_[i] += (eqT_[i] - eq_[i]) * 0.35f;
    const int h = max(3, static_cast<int>(eq_[i] * maxH));
    spr_.fillRoundRect(x + i * (barW + 4), baseY - h, barW, h, 2, col);
  }
}

void Panel::drawClockSmall(int rightX, int y) {
  struct tm tmv;
  if (!getLocalTime(&tmv, 0)) return;
  char buf[8];
  strftime(buf, sizeof(buf), "%H:%M", &tmv);
  spr_.setFreeFont(&FreeSansBold9pt7b);
  spr_.setTextColor(COL_SUBTLE);
  spr_.setTextDatum(MR_DATUM);
  spr_.drawString(buf, rightX, y);
}

void Panel::drawClaude(uint32_t now) {
  // Pulsing Claude-orange spark + call to action.
  const float pulse = 0.5f + 0.5f * sinf(now / 220.0f);
  const int r = 9 + static_cast<int>(4 * pulse);
  for (int i = 0; i < 4; i++) {
    const float a = i * PI / 4 + now / 900.0f;
    const int dx = static_cast<int>(cosf(a) * r), dy = static_cast<int>(sinf(a) * r);
    spr_.drawWideLine(30 - dx, 45 - dy, 30 + dx, 45 + dy, 4, kClaude, TFT_BLACK);
  }
  spr_.setFreeFont(&FreeSansBold12pt7b);
  spr_.setTextColor(kClaude);
  spr_.setTextDatum(ML_DATUM);
  spr_.drawString("Claude needs you", 56, 32);
  spr_.setFreeFont(&FreeSans9pt7b);
  spr_.setTextColor(COL_SUBTLE);
  spr_.drawString(claudeProject_.length() ? claudeProject_ + "  -  waiting for input" : String("waiting for input"),
                  56, 62);
}

void Panel::drawSay(uint32_t now) {
  const uint32_t shown = (now - sayStart_) / 26;
  spr_.setFreeFont(sayFont_);
  spr_.setTextColor(COL_MONO);
  spr_.setTextDatum(ML_DATUM);
  static const int ys1[] = {44};
  static const int ys2[] = {28, 62};
  const int *ys = sayCount_ > 1 ? ys2 : ys1;
  uint32_t budget = shown;
  for (int i = 0; i < sayCount_; i++) {
    const String &line = sayLines_[i];
    const int x = (PANEL_W - spr_.textWidth(line)) / 2;
    const uint32_t take = min<uint32_t>(budget, line.length());
    spr_.drawString(line.substring(0, take), x, ys[i]);
    budget -= take;
    if (budget == 0) break;
  }
}

void Panel::drawPomodoro(uint32_t /*now*/) {
  const uint16_t col = pomoFocus_ ? COL_MONO : COL_EYE_MUSIC;
  const float frac = 1.0f - static_cast<float>(pomoRemain_) / pomoTotal_;
  const uint32_t end = 180 + static_cast<uint32_t>(frac * 359.0f);  // TFT_eSPI arcs: 0 deg = 6 o'clock
  spr_.drawSmoothArc(44, 45, 34, 28, 0, 360, COL_DIM, TFT_BLACK, false);
  if (frac > 0.003f) spr_.drawSmoothArc(44, 45, 34, 28, 180, end > 360 ? end - 360 : end, col, TFT_BLACK, true);
  spr_.setTextFont(2);
  spr_.setTextDatum(MC_DATUM);
  spr_.setTextColor(col);
  spr_.drawString(String(pomoRound_), 44, 45);

  const uint32_t s = (pomoRemain_ + 999) / 1000;
  char buf[8];
  snprintf(buf, sizeof(buf), "%02u:%02u", static_cast<unsigned>(s / 60), static_cast<unsigned>(s % 60));
  spr_.setFreeFont(&Orbitron_Light_32);
  spr_.setTextColor(COL_TEXT);
  spr_.setTextDatum(ML_DATUM);
  spr_.drawString(buf, 96, 36);
  spr_.setFreeFont(&FreeSans9pt7b);
  spr_.setTextColor(COL_SUBTLE);
  spr_.drawString(pomoFocus_ ? "FOCUS  -  deep work" : "BREAK  -  stretch, sip water", 98, 70);
}

void Panel::drawLyrics(uint32_t now) {
  if (music_.lyricCur() != lyricShown_) {
    lyricShown_ = music_.lyricCur();
    wrap(lyricShown_, kBigFonts, 2, PANEL_W - 24, lyricLines_, lyricCount_, lyricFont_);
  }
  // Header: what's playing.
  spr_.setTextFont(1);
  spr_.setTextDatum(TL_DATUM);
  spr_.setTextColor(music_.color());
  String head = String("> ") + music_.track();
  if (music_.artist().length()) head += "  -  " + music_.artist();
  if (head.length() > 50) head = head.substring(0, 49) + "~";
  spr_.drawString(head, 12, 5);

  // Slide-up transition when the line changes.
  const uint32_t t = now - music_.lyricChangedAt();
  const int slide = t < 220 ? static_cast<int>(14 * (1.0f - t / 220.0f)) : 0;
  spr_.setFreeFont(lyricFont_);
  spr_.setTextColor(COL_TEXT);
  spr_.setTextDatum(MC_DATUM);
  if (lyricCount_ <= 1) {
    spr_.drawString(lyricLines_[0], PANEL_W / 2, 38 + slide);
  } else {
    spr_.drawString(lyricLines_[0], PANEL_W / 2, 28 + slide);
    spr_.drawString(lyricLines_[1], PANEL_W / 2, 52 + slide);
  }
  if (lyricCount_ <= 1 && music_.lyricNext().length()) {
    String nx = music_.lyricNext();
    spr_.setFreeFont(&FreeSans9pt7b);
    while (nx.length() > 3 && spr_.textWidth(nx) > PANEL_W - 30) nx.remove(nx.length() - 1);
    spr_.setTextColor(0x6B4D);
    spr_.drawString(nx, PANEL_W / 2, 72 + slide);
  }
}

void Panel::drawMusic(uint32_t now) {
  if (showLyrics_ && music_.lyricCur().length()) {
    drawLyrics(now);
    return;
  }
  // Compact now-playing: title, artist, activity, EQ visualiser.
  const String &title = music_.track();
  spr_.setViewport(14, 8, 236, 34);
  spr_.setFreeFont(&FreeSansBold12pt7b);
  spr_.setTextColor(COL_TEXT);
  spr_.setTextDatum(ML_DATUM);
  const int tw = spr_.textWidth(title);
  if (tw <= 236) {
    spr_.drawString(title, 0, 16);
  } else {
    const int period = tw + 48;
    const uint32_t t = now - music_.trackChangedAt();
    const int off = t < 1800 ? 0 : static_cast<int>(((t - 1800) / 28) % period);
    spr_.drawString(title, -off, 16);
    spr_.drawString(title, -off + period, 16);
  }
  spr_.resetViewport();

  String a = music_.artist();
  spr_.setFreeFont(&FreeSans9pt7b);
  while (a.length() > 3 && spr_.textWidth(a) > 230) a.remove(a.length() - 1);
  if (a != music_.artist()) a += "...";
  spr_.setTextColor(COL_SUBTLE);
  spr_.setTextDatum(ML_DATUM);
  spr_.drawString(a, 14, 54);

  if (helperOnline_ && actLabel_.length() && actCat_ != "music") {
    spr_.setTextFont(1);
    spr_.setTextColor(categoryColor(actCat_));
    spr_.drawString(actLabel_ + "  -  " + activitySubline(actCat_, actMinutes_), 14, 77);
  }
  drawEq(262, 74, 5, 6, 40, music_.color());
}

void Panel::drawActivity(uint32_t /*now*/) {
  spr_.fillCircle(20, 30, 6, categoryColor(actCat_));
  String label = actLabel_;
  spr_.setFreeFont(&FreeSansBold12pt7b);
  while (label.length() > 3 && spr_.textWidth(label) > 210) label.remove(label.length() - 1);
  spr_.setTextColor(COL_TEXT);
  spr_.setTextDatum(ML_DATUM);
  spr_.drawString(label, 34, 30);
  spr_.setFreeFont(&FreeSans9pt7b);
  spr_.setTextColor(COL_SUBTLE);
  spr_.drawString(activitySubline(actCat_, actMinutes_), 34, 56);
  drawClockSmall(310, 30);
  drawTaskLine(PANEL_H - 4);
}

void Panel::drawClock(uint32_t /*now*/) {
  struct tm tmv;
  const bool synced = getLocalTime(&tmv, 0);
  char timeBuf[8] = "--:--";
  char dateBuf[24] = "";
  if (synced) {
    strftime(timeBuf, sizeof(timeBuf), "%H:%M", &tmv);
    strftime(dateBuf, sizeof(dateBuf), "%a, %d %b", &tmv);
  }
  spr_.setFreeFont(&Orbitron_Light_32);
  spr_.setTextColor(COL_TEXT);
  spr_.setTextDatum(MC_DATUM);
  spr_.drawString(timeBuf, PANEL_W / 2, 36);
  spr_.setFreeFont(&FreeSans9pt7b);
  spr_.setTextColor(COL_SUBTLE);
  spr_.drawString(dateBuf, PANEL_W / 2, taskTitle_.length() ? 64 : 70);
  if (helperOnline_) drawTaskLine(PANEL_H - 4);
  if (!helperOnline_) {
    spr_.setTextFont(1);
    spr_.setTextDatum(BR_DATUM);
    spr_.setTextColor(COL_DIM);
    spr_.drawString("companion offline", PANEL_W - 4, PANEL_H - 2);
  }
}

void Panel::peek(uint32_t ms) {
  const uint32_t until = millis() + ms;
  if (until > peekUntil_) peekUntil_ = until;
}

void Panel::setHidden(bool h) {
  if (h != hidden_) {
    hidden_ = h;
    dirty_ = true;
  }
}

bool Panel::wantsVisible(uint32_t now, bool musicView) const {
  if (claude_ == ClaudeState::Waiting) return true;
  if (now < sayUntil_) return true;
  if (musicView) return false;  // music / focus screens own the display
  if (pomoActive_) return true;
  return now < peekUntil_;
}

void Panel::update(uint32_t now) {
  if (!ok_ || hidden_) return;

  const Mode mode = currentMode(now);
  if (mode != lastMode_) dirty_ = true;

  struct tm tmv;
  if (getLocalTime(&tmv, 0) && tmv.tm_min != lastMinute_) {
    lastMinute_ = tmv.tm_min;
    dirty_ = true;
  }

  bool animating = false;
  switch (mode) {
    case Mode::Claude:
      animating = true;
      break;
    case Mode::Say: {
      size_t total = 0;
      for (int i = 0; i < sayCount_; i++) total += sayLines_[i].length();
      animating = (now - sayStart_) / 26 <= total;
      break;
    }
    case Mode::Music:
      animating = !(showLyrics_ && music_.lyricCur().length()) || (now - music_.lyricChangedAt() < 260) ||
                  music_.lyricCur() != lyricShown_;
      break;
    default:
      break;
  }

  if (!dirty_ && !(animating && now - lastDraw_ >= PANEL_ANIM_MS)) return;

  spr_.fillSprite(TFT_BLACK);
  spr_.drawFastHLine(24, 0, PANEL_W - 48, COL_DIM);

  switch (mode) {
    case Mode::Claude:   drawClaude(now); break;
    case Mode::Say:      drawSay(now); break;
    case Mode::Pomodoro: drawPomodoro(now); break;
    case Mode::Music:    drawMusic(now); break;
    case Mode::Activity: drawActivity(now); break;
    case Mode::Clock:    drawClock(now); break;
  }

  spr_.pushSprite(0, PANEL_Y);
  lastMode_ = mode;
  dirty_ = false;
  lastDraw_ = now;
}
