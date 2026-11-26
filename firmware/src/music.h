#pragma once

#include <TFT_eSPI.h>

#include "eyes.h"

constexpr int ART_SIZE = 56;  // album art thumbnail (companion sends this size)

// Now-playing state + the "vinyl" view drawn into the top canvas.
class NowPlaying {
 public:
  explicit NowPlaying(TFT_eSPI &tft);
  bool begin();

  void setTrack(bool playing, const String &track, const String &artist, uint32_t posMs, uint32_t durMs,
                uint16_t color);
  bool setArt(const uint8_t *rgb565be, size_t len);  // ART_SIZE^2 * 2 bytes, big-endian RGB565
  void setLyrics(const String &cur, const String &next, uint32_t lineMs);
  void clear();

  bool playing() const { return playing_; }
  bool hasTrack() const { return track_.length() > 0; }
  const String &track() const { return track_; }
  const String &artist() const { return artist_; }
  const String &lyricCur() const { return lyricCur_; }
  const String &lyricNext() const { return lyricNext_; }
  uint32_t lyricChangedAt() const { return lyricChangedAt_; }
  uint32_t lyricLineMs() const { return lyricLineMs_; }
  uint32_t trackChangedAt() const { return trackSince_; }
  uint16_t color() const { return color_; }
  uint32_t positionMs(uint32_t now) const;
  uint32_t durationMs() const { return durMs_; }

  // Full now-playing layout for the 320x150 top canvas (small eyes included).
  void drawTop(TFT_eSprite &s, Eyes &eyes, uint32_t now);

 private:
  void drawVinyl(TFT_eSprite &s, int cx, int cy, int r, uint32_t now);

  TFT_eSPI &tft_;
  TFT_eSprite art_;
  bool artOk_ = false;
  bool hasArt_ = false;

  bool playing_ = false;
  String track_, artist_;
  uint32_t trackSince_ = 0;
  uint32_t posMs_ = 0, durMs_ = 0, posAt_ = 0;
  uint16_t color_;
  String lyricCur_, lyricNext_;
  uint32_t lyricChangedAt_ = 0;
  uint32_t lyricLineMs_ = 0;

  float angle_ = 0;
  uint32_t lastFrame_ = 0;
};

String asciiOnly(const String &in);
String fmtClock(uint32_t ms);  // 83000 -> "1:23"
