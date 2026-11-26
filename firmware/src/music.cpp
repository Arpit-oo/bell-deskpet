#include "music.h"

#include "config.h"

namespace {

constexpr uint16_t kArtKey = 0x0001;       // transparent key for the circular label mask
constexpr uint16_t kVinyl = 0x10A2;        // record body
constexpr uint16_t kGroove1 = 0x18E3;
constexpr uint16_t kGroove2 = 0x2124;
constexpr uint16_t kSheen = 0x4A69;
constexpr uint16_t kTrack = 0x2945;        // progress track

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
  const float r = ART_SIZE / 2.0f - 0.5f;
  for (int y = 0; y < ART_SIZE; y++) {
    for (int x = 0; x < ART_SIZE; x++) {
      const float dx = x - r, dy = y - r;
      if (dx * dx + dy * dy > r * r) art_.drawPixel(x, y, kArtKey);
    }
  }
  art_.fillCircle(ART_SIZE / 2, ART_SIZE / 2, 4, TFT_BLACK);  // spindle hole
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

void NowPlaying::drawVinyl(TFT_eSprite &s, int cx, int cy, int r, uint32_t now) {
  s.fillSmoothCircle(cx, cy, r, kVinyl, TFT_BLACK);
  for (int g = r - 6; g > 32; g -= 5) s.drawCircle(cx, cy, g, (g / 5) % 2 ? kGroove1 : kGroove2);

  // Rotating light sheen sells the spin.
  const uint32_t a = static_cast<uint32_t>(angle_) % 360;
  s.drawSmoothArc(cx, cy, r - 3, 34, a, (a + 28) % 360, kSheen, kVinyl, true);
  s.drawSmoothArc(cx, cy, r - 3, 34, (a + 180) % 360, (a + 208) % 360, kSheen, kVinyl, true);

  // Label: rotating album art, or the album colour.
  if (hasArt_) {
    s.setPivot(cx, cy);
    art_.pushRotated(&s, static_cast<int16_t>(angle_), kArtKey);
  } else {
    s.fillSmoothCircle(cx, cy, ART_SIZE / 2, color_, kVinyl);
    // Small mark so the spin is visible even without art.
    const float rad = angle_ * DEG_TO_RAD;
    s.fillCircle(cx + static_cast<int>(cosf(rad) * 18), cy + static_cast<int>(sinf(rad) * 18), 3, TFT_BLACK);
    s.fillCircle(cx, cy, 4, TFT_BLACK);
  }
  s.drawCircle(cx, cy, ART_SIZE / 2 + 1, kGroove2);

  // Tone arm resting on the record.
  const int px = cx + r + 4, py = cy - r + 6;
  const float lift = playing_ ? 0 : 10;  // arm swings off when paused
  s.drawWideLine(px, py, cx + 40 + lift, cy - 4 - lift, 4, COL_SUBTLE, TFT_BLACK);
  s.fillSmoothRoundRect(static_cast<int>(cx + 34 + lift), static_cast<int>(cy - 8 - lift), 12, 8, 3, 0xBDF7,
                        TFT_BLACK);
  s.fillSmoothCircle(px, py, 6, 0x6B4D, TFT_BLACK);
  s.fillSmoothCircle(px, py, 2, COL_GLINT, 0x6B4D);
  (void)now;
}

void NowPlaying::drawTop(TFT_eSprite &s, Eyes &eyes, uint32_t now) {
  const float dt = lastFrame_ ? (now - lastFrame_) : 0;
  lastFrame_ = now;
  if (playing_) angle_ = fmodf(angle_ + dt * 0.2f, 360.0f);  // 33 1/3 rpm

  drawVinyl(s, 78, 76, 66, now);

  // Tiny vibing face top-right.
  eyes.draw(s, 240, 34, 0.38f);

  // Title (scrolls inside a clipped viewport).
  const int x0 = 166, w = 148;
  s.setViewport(x0, 70, w, 26);
  s.setFreeFont(&FreeSansBold12pt7b);
  s.setTextColor(COL_TEXT);
  s.setTextDatum(ML_DATUM);
  const int tw = s.textWidth(track_);
  if (tw <= w) {
    s.drawString(track_, 0, 13);
  } else {
    const int period = tw + 40;
    const uint32_t t = now - trackSince_;
    const int off = t < 1800 ? 0 : static_cast<int>(((t - 1800) / 30) % period);
    s.drawString(track_, -off, 13);
    s.drawString(track_, -off + period, 13);
  }
  s.resetViewport();

  String a = artist_;
  s.setFreeFont(&FreeSans9pt7b);
  while (a.length() > 3 && s.textWidth(a) > w) a.remove(a.length() - 1);
  if (a != artist_) a += "...";
  s.setTextColor(COL_SUBTLE);
  s.drawString(a, x0, 106);

  // Progress bar.
  const uint32_t pos = positionMs(now);
  const int barY = 124;
  s.fillSmoothRoundRect(x0, barY, w, 4, 2, kTrack, TFT_BLACK);
  if (durMs_ > 0) {
    const int fw = max(4, static_cast<int>(static_cast<uint64_t>(w) * pos / durMs_));
    s.fillSmoothRoundRect(x0, barY, fw, 4, 2, color_, TFT_BLACK);
    s.fillSmoothCircle(x0 + fw, barY + 2, 4, COL_GLINT, TFT_BLACK);
    s.setTextFont(1);
    s.setTextColor(COL_SUBTLE);
    s.setTextDatum(TL_DATUM);
    s.drawString(fmtClock(pos), x0, barY + 10);
    s.setTextDatum(TR_DATUM);
    s.drawString("-" + fmtClock(durMs_ - pos), x0 + w, barY + 10);
  }
}
