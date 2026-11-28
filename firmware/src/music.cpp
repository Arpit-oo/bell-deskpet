#include "music.h"

#include "config.h"

namespace {

constexpr uint16_t kVinyl = 0x10A2;   // record body
constexpr uint16_t kGroove1 = 0x18E3;
constexpr uint16_t kGroove2 = 0x2124;
constexpr uint16_t kSheen = 0x4A69;

}  // namespace

String asciiOnly(const String &in) {
  String out;
  out.reserve(in.length());
  bool lastSpace = false;
  for (size_t i = 0; i < in.length(); i++) {
    const uint8_t c = static_cast<uint8_t>(in[i]);
    if (c >= 0x20 && c < 0x7F) {
      const bool sp = (c == ' ');
      if (sp && lastSpace) continue;
      out += static_cast<char>(c);
      lastSpace = sp;
    }
  }
  out.trim();
  return out;
}

String fmtClock(uint32_t ms) {
  const uint32_t s = ms / 1000;
  char buf[12];
  snprintf(buf, sizeof(buf), "%u:%02u", static_cast<unsigned>(s / 60), static_cast<unsigned>(s % 60));
  return String(buf);
}

NowPlaying::NowPlaying(TFT_eSPI &tft) : tft_(tft), art_(&tft), color_(COL_EYE_MUSIC) {}

bool NowPlaying::begin() {
  art_.setColorDepth(16);
  artOk_ = art_.createSprite(ART_SIZE, ART_SIZE) != nullptr;
  if (artOk_) art_.setPivot(ART_SIZE / 2, ART_SIZE / 2);
  return artOk_;
}

void NowPlaying::setTrack(bool playing, const String &track, const String &artist, uint32_t posMs,
                          uint32_t durMs, uint16_t color) {
  const String t = asciiOnly(track);
  if (t != track_) {
    trackSince_ = millis();
    hasArt_ = false;  // new track: wait for its art
    lyricCur_ = lyricNext_ = "";
  }
  playing_ = playing;
  track_ = t;
  artist_ = asciiOnly(artist);
  posMs_ = posMs;
  durMs_ = durMs;
  posAt_ = millis();
  color_ = color;
}

bool NowPlaying::setArt(const uint8_t *data, size_t len) {
  if (!artOk_ || len != static_cast<size_t>(ART_SIZE * ART_SIZE * 2)) return false;
  // Bytes are big-endian RGB565 == sprite's native byte order, so copy straight in.
  art_.pushImage(0, 0, ART_SIZE, ART_SIZE, reinterpret_cast<const uint16_t *>(data));
  hasArt_ = true;
  return true;
}

void NowPlaying::setLyrics(const String &cur, const String &next, uint32_t lineMs) {
  const String c = asciiOnly(cur);
  if (c != lyricCur_) {
    lyricChangedAt_ = millis();
    lyricLineMs_ = lineMs;
  }
  lyricCur_ = c;
  lyricNext_ = asciiOnly(next);
}

void NowPlaying::clear() {
  playing_ = false;
  track_ = artist_ = lyricCur_ = lyricNext_ = "";
  hasArt_ = false;
}

uint32_t NowPlaying::positionMs(uint32_t now) const {
  uint32_t p = posMs_ + (playing_ ? (now - posAt_) : 0);
  if (durMs_ && p > durMs_) p = durMs_;
  return p;
}

void NowPlaying::drawVinyl(TFT_eSprite &s, int cx, int cy, int r) {
  s.fillSmoothCircle(cx, cy, r, kVinyl, TFT_BLACK);
  for (int g = r - 5; g > 24; g -= 4) s.drawCircle(cx, cy, g, (g / 4) % 2 ? kGroove1 : kGroove2);

  // Two rotating light sheens sell the spin.
  const uint32_t a = static_cast<uint32_t>(angle_) % 360;
  s.drawSmoothArc(cx, cy, r - 4, 26, a, (a + 24) % 360, kSheen, kVinyl, true);
  s.drawSmoothArc(cx, cy, r - 4, 26, (a + 180) % 360, (a + 204) % 360, kSheen, kVinyl, true);

  // Plain white label with a rotating mark + spindle.
  s.fillSmoothCircle(cx, cy, 20, COL_MONO, kVinyl);
  const float rad = angle_ * DEG_TO_RAD;
  s.fillSmoothCircle(cx + static_cast<int>(cosf(rad) * 12), cy + static_cast<int>(sinf(rad) * 12), 2, TFT_BLACK, COL_MONO);
  s.fillSmoothCircle(cx, cy, 3, TFT_BLACK, COL_MONO);

  // Tone arm: rests on the record while playing, swings off when paused.
  const int px = cx + r + 6, py = cy - r + 8;
  const float lift = playing_ ? 0 : 14;
  const int hx = static_cast<int>(cx + 30 + lift), hy = static_cast<int>(cy - 10 - lift);
  s.drawWideLine(px, py, hx, hy, 3, COL_SUBTLE, TFT_BLACK);
  s.fillSmoothRoundRect(hx - 5, hy - 3, 11, 7, 2, COL_MONO, TFT_BLACK);
  s.fillSmoothCircle(px, py, 5, 0x4208, TFT_BLACK);
  s.fillSmoothCircle(px, py, 2, COL_MONO, 0x4208);
}

void NowPlaying::drawTop(TFT_eSprite &s, uint32_t now) {
  const float dt = lastFrame_ ? static_cast<float>(now - lastFrame_) : 0.0f;
  lastFrame_ = now;
  if (playing_) angle_ = fmodf(angle_ + dt * 0.2f, 360.0f);  // 33 1/3 rpm

  drawVinyl(s, 72, 75, 62);

  const int x0 = 160, w = 320 - x0 - 14;

  s.setTextFont(1);
  s.setTextColor(COL_SUBTLE);
  s.setTextDatum(TL_DATUM);
  s.drawString(playing_ ? "NOW PLAYING" : "PAUSED", x0, 22);

  // Title (scrolls inside a clipped viewport).
  s.setViewport(x0, 36, w, 28);
  s.setFreeFont(&FreeSansBold12pt7b);
  s.setTextColor(COL_TEXT);
  s.setTextDatum(ML_DATUM);
  const int tw = s.textWidth(track_);
  if (tw <= w) {
    s.drawString(track_, 0, 14);
  } else {
    const int period = tw + 40;
    const uint32_t t = now - trackSince_;
    const int off = t < 2200 ? 0 : static_cast<int>(((t - 2200) / 28) % period);
    s.drawString(track_, -off, 14);
    s.drawString(track_, -off + period, 14);
  }
  s.resetViewport();

  String a = artist_;
  s.setFreeFont(&FreeSans9pt7b);
  while (a.length() > 3 && s.textWidth(a) > w) a.remove(a.length() - 1);
  if (a != artist_) a += "...";
  s.setTextColor(COL_SUBTLE);
  s.setTextDatum(ML_DATUM);
  s.drawString(a, x0, 76);

  // Line visualizer: a calm waveform tapered at both ends; flat when paused.
  const int vy = 106;
  const float tt = now / 1000.0f;
  int lx = x0, ly = vy;
  for (int x = x0; x <= x0 + w; x += 3) {
    const float u = static_cast<float>(x - x0) / w;
    const float env = sinf(u * PI);
    float v = 0;
    if (playing_) {
      v = sinf(u * 19.0f + tt * 7.0f) * 0.55f + sinf(u * 37.0f - tt * 11.0f) * 0.3f +
          sinf(u * 7.0f + tt * 3.0f) * 0.35f;
      v *= 0.75f + 0.25f * sinf(tt * 2.3f);
    }
    const int y = vy + static_cast<int>(v * env * 12.0f);
    if (x > x0) s.drawWideLine(lx, ly, x, y, 2, COL_MONO, TFT_BLACK);
    lx = x;
    ly = y;
  }

  if (durMs_ > 0) {
    const uint32_t pos = positionMs(now);
    s.setTextFont(1);
    s.setTextColor(COL_SUBTLE);
    s.setTextDatum(TL_DATUM);
    s.drawString(fmtClock(pos), x0, 126);
    s.setTextDatum(TR_DATUM);
    s.drawString(fmtClock(durMs_), x0 + w, 126);
  }
}
