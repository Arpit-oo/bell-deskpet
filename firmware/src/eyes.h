#pragma once

#include <TFT_eSPI.h>

#include "mood.h"

// Short expressive animations played while idle.
enum class IdleAnim : uint8_t { None, LookAround, Curious, Wiggle, Yawn, Peek };

// Procedural Tabbie-style eyes (+ brows). Pure state + drawing: the caller owns
// the sprite, so the same face can be drawn full-size or small (music view).
class Eyes {
 public:
  Eyes();

  void setMood(Mood m);
  Mood mood() const { return mood_; }
  void setBaseColor(uint16_t c);
  void setBrows(bool on) { browsEnabled_ = on; }

  void blink();
  void lookAt(float x, float y, uint32_t holdMs);  // -1..1
  void openFromClosed();
  void play(IdleAnim a);
  // Sing-along: mode 0 off, 1 hum, 2 sing (lip-sync to the current lyric line).
  void setSing(uint8_t mode, uint32_t lineStart, uint16_t syllables, uint32_t lineMs);
  bool singing() const { return singMode_ > 0; }
  bool animating() const { return anim_ != IdleAnim::None; }

  void update(uint32_t now);
  // Draws the face centred on (cx, cy) into `s` (does not clear it).
  void draw(TFT_eSprite &s, int cx, int cy, float scale);

 private:
  struct Shape {
    float w, h, r;
    float slant;    // +: inner corner lowered (angry), -: outer lowered (sad)
    float lid;      // 0..1 covered from top
    float happy;    // 0..1 bottom arch cut
    float yOff;
    float browY;    // brow gap above eye top
    float browAng;  // degrees, + = inner end down
  };
  struct Target {
    Shape l, r;
    uint16_t color;
    float gap, bounce, hearts, sparkles, zzz, brows;
    float saccadeRate, blinkRate;
  };
  struct Mods {  // idle animation modifiers
    float dyL, dyR, browL, browR, lid, hScale, happy, scale;
  };

  Target targetFor(Mood m) const;
  void drawEye(TFT_eSprite &s, int cx, int cy, const Shape &e, float scale, bool isLeft, uint16_t col);
  void drawBrow(TFT_eSprite &s, int cx, int eyeTop, const Shape &e, float scale, bool isLeft, float lift,
                uint16_t col);
  void drawHeart(TFT_eSprite &s, int cx, int cy, float size, uint16_t col);
  void drawSparkles(TFT_eSprite &s, int cx, int cy, float scale);
  void drawZzz(TFT_eSprite &s, int x, int y);
  void drawMouth(TFT_eSprite &s, int cx, int y, float scale, uint16_t col);
  void drawIdleMouth(TFT_eSprite &s, int cx, int y, float scale, uint16_t col);
  void drawNotes(TFT_eSprite &s, int cx, int cy, float scale, uint16_t col);
  float mouthTarget(uint32_t now) const;
  float blinkFactor(uint32_t now);
  void updateAnim(uint32_t now);

  Mood mood_ = Mood::Idle;
  uint16_t baseColor_;
  bool browsEnabled_ = true;
  Target tgt_;
  Shape curL_, curR_;
  float curGap_, curBounce_, curHearts_, curSparkles_, curZzz_, curBrows_;
  float colR_, colG_, colB_;
  Mods mods_{0, 0, 0, 0, 0, 1, 0, 1};

  float lookX_ = 0, lookY_ = 0, lookTX_ = 0, lookTY_ = 0;
  uint32_t nextSaccade_ = 0, lookHoldUntil_ = 0;

  uint32_t nextBlink_ = 0, blinkStart_ = 0;
  bool blinking_ = false, doubleBlink_ = false;

  IdleAnim anim_ = IdleAnim::None;
  uint32_t animStart_ = 0;
  bool animBlinked_ = false;

  uint8_t singMode_ = 0;
  uint32_t lineStart_ = 0, lineMs_ = 0;
  uint16_t syllables_ = 0;
  float singAmt_ = 0, mouthOpen_ = 0;
  float smile_ = 0.55f, oMouth_ = 0;  // resting mouth: smile (-1..1) / "o" (0..1)

  float bootOpen_ = 1.0f;
  uint32_t now_ = 0;
};
