#include "eyes.h"

#include <math.h>

#include "config.h"

namespace {

float frand(float lo, float hi) {
  return lo + (hi - lo) * (static_cast<float>(esp_random() % 10000) / 10000.0f);
}

uint32_t urand(uint32_t lo, uint32_t hi) { return lo + (esp_random() % (hi - lo + 1)); }

// Exponential smoothing that is frame-rate independent.
float approach(float cur, float target, float dtMs, float tauMs) {
  return cur + (target - cur) * (1.0f - expf(-dtMs / tauMs));
}

void split565(uint16_t c, float &r, float &g, float &b) {
  r = ((c >> 11) & 0x1F) * 255.0f / 31.0f;
  g = ((c >> 5) & 0x3F) * 255.0f / 63.0f;
  b = (c & 0x1F) * 255.0f / 31.0f;
}

uint16_t join565(float r, float g, float b) {
  const uint16_t R = static_cast<uint16_t>(constrain(r, 0.0f, 255.0f)) >> 3;
  const uint16_t G = static_cast<uint16_t>(constrain(g, 0.0f, 255.0f)) >> 2;
  const uint16_t B = static_cast<uint16_t>(constrain(b, 0.0f, 255.0f)) >> 3;
  return (R << 11) | (G << 5) | B;
}

// 0 -> 1 -> 0 envelope with smooth 250 ms edges.
float envelope(uint32_t t, uint32_t dur) {
  const float edge = 250.0f;
  float e = min(1.0f, min(t / edge, (dur > t ? (dur - t) : 0) / edge));
  e = constrain(e, 0.0f, 1.0f);
  return e * e * (3 - 2 * e);
}

constexpr float kBeatMs = 60000.0f / 116.0f;

uint32_t animDuration(IdleAnim a) {
  switch (a) {
    case IdleAnim::LookAround: return 2600;
    case IdleAnim::Curious: return 2000;
    case IdleAnim::Wiggle: return 1000;
    case IdleAnim::Yawn: return 2400;
    case IdleAnim::Peek: return 1600;
    default: return 0;
  }
}

}  // namespace

Eyes::Eyes() : baseColor_(COL_EYE_ICE) {
  tgt_ = targetFor(Mood::Idle);
  curL_ = tgt_.l;
  curR_ = tgt_.r;
  curGap_ = tgt_.gap;
  curBounce_ = curHearts_ = curSparkles_ = curZzz_ = 0;
  curBrows_ = 1;
  split565(tgt_.color, colR_, colG_, colB_);
}

void Eyes::openFromClosed() {
  bootOpen_ = 0.0f;
  const uint32_t now = millis();
  nextBlink_ = now + 2500;
  nextSaccade_ = now + 1500;
}

void Eyes::setBaseColor(uint16_t c) {
  baseColor_ = c;
  tgt_ = targetFor(mood_);
}

Eyes::Target Eyes::targetFor(Mood m) const {
  //            w   h   r  slant lid happy yOff browY browAng
  const Shape base{78, 92, 24, 0, 0, 0, 0, 16, 0};
  Target t{base, base, baseColor_, 54, 0, 0, 0, 0, 1, 1.0f, 1.0f};

  switch (m) {
    case Mood::Idle:
      break;
    case Mood::Happy:
      t.l = t.r = Shape{78, 92, 24, 0, 0, 1.0f, 0, 22, -6};
      break;
    case Mood::Excited:
      t.l = t.r = Shape{86, 98, 26, 0, 0, 1.0f, -4, 22, -8};
      t.color = COL_EYE_GOLD;
      t.bounce = 1;
      t.sparkles = 1;
      t.saccadeRate = 1.4f;
      break;
    case Mood::Focus:
      t.l = t.r = Shape{86, 56, 18, 7, 0, 0, 6, 10, 8};
      t.color = COL_EYE_FOCUS;
      t.saccadeRate = 0.35f;
      t.blinkRate = 0.8f;
      break;
    case Mood::Music:
      t.l = t.r = Shape{80, 88, 24, 0, 0, 0.8f, 0, 20, -4};
      t.color = COL_EYE_MUSIC;
      t.bounce = 1;
      t.saccadeRate = 0.6f;
      break;
    case Mood::Sleepy:
      t.l = t.r = Shape{80, 78, 22, -5, 0.52f, 0, 8, 6, -4};
      t.color = COL_EYE_SLEEPY;
      t.zzz = 1;
      t.saccadeRate = 0.2f;
      t.blinkRate = 0.45f;
      break;
    case Mood::Sus:
      t.l = Shape{80, 84, 22, 4, 0.48f, 0, 4, 6, 12};
      t.r = Shape{80, 84, 22, 4, 0.30f, 0, 2, 22, -10};
      t.saccadeRate = 0.25f;
      break;
    case Mood::Sad:
      t.l = t.r = Shape{76, 80, 24, -18, 0, 0, 8, 14, -20};
      t.color = COL_EYE_SLEEPY;
      t.saccadeRate = 0.4f;
      break;
    case Mood::Angry:
      t.l = t.r = Shape{82, 76, 18, 20, 0, 0, 4, 4, 26};
      t.color = COL_EYE_ANGRY;
      t.saccadeRate = 0.6f;
      break;
    case Mood::Surprised:
      t.l = t.r = Shape{90, 104, 40, 0, 0, 0, -6, 18, -4};
      t.gap = 48;
      t.saccadeRate = 1.6f;
      t.blinkRate = 0.5f;
      break;
    case Mood::Love:
      t.color = COL_EYE_LOVE;
      t.hearts = 1;
      t.brows = 0;
      t.bounce = 0.4f;
      t.saccadeRate = 0.5f;
      break;
    default:
      break;
  }
  return t;
}

void Eyes::setMood(Mood m) {
  if (m == mood_) return;
  mood_ = m;
  tgt_ = targetFor(m);
  if (!blinking_ && m != Mood::Sleepy) blink();
}

void Eyes::blink() {
  blinking_ = true;
  blinkStart_ = millis();
  doubleBlink_ = (esp_random() % 100) < 15;
}

void Eyes::lookAt(float x, float y, uint32_t holdMs) {
  lookTX_ = constrain(x, -1.0f, 1.0f);
  lookTY_ = constrain(y, -1.0f, 1.0f);
  lookHoldUntil_ = millis() + holdMs;
}

void Eyes::play(IdleAnim a) {
  anim_ = a;
  animStart_ = millis();
  animBlinked_ = false;
}

void Eyes::setSing(uint8_t mode, uint32_t lineStart, uint16_t syllables, uint32_t lineMs) {
  singMode_ = mode;
  lineStart_ = lineStart;
  syllables_ = syllables;
  lineMs_ = lineMs;
}

float Eyes::mouthTarget(uint32_t now) const {
  if (singMode_ == 1) return 0.14f + 0.06f * sinf(now / 320.0f);  // hum: lips barely moving
  if (singMode_ != 2 || syllables_ == 0) return 0.05f;
  const uint32_t t = now - lineStart_;
  const float active = lineMs_ ? lineMs_ * 0.85f : syllables_ * 230.0f;
  if (t >= active) return 0.05f;
  const float ph = (t / active) * syllables_;
  const int syl = static_cast<int>(ph);
  // Per-syllable loudness variation so it doesn't look robotic.
  const float loud = 0.55f + 0.45f * (((syl * 2654435761u) >> 24) & 0xFF) / 255.0f;
  return powf(fabsf(sinf(PI * ph)), 0.7f) * loud;
}

float Eyes::blinkFactor(uint32_t now) {
  if (!blinking_) return 1.0f;
  const int32_t t = static_cast<int32_t>(now - blinkStart_);
  if (t < 0) return 1.0f;
  if (t < 70) return 1.0f - 0.95f * (t / 70.0f);
  if (t < 100) return 0.05f;
  if (t < 210) return 0.05f + 0.95f * ((t - 100) / 110.0f);
  if (doubleBlink_) {
    doubleBlink_ = false;
    blinkStart_ = now + 60;
  } else {
    blinking_ = false;
  }
  return 1.0f;
}

void Eyes::updateAnim(uint32_t now) {
  mods_ = Mods{0, 0, 0, 0, 0, 1, 0, 1};
  if (anim_ == IdleAnim::None) return;
  const uint32_t dur = animDuration(anim_);
  const uint32_t t = now - animStart_;
  if (t >= dur) {
    anim_ = IdleAnim::None;
    return;
  }
  const float e = envelope(t, dur);
  switch (anim_) {
    case IdleAnim::LookAround:
      if (t < 700) lookAt(-0.95f, -0.15f, 400);
      else if (t < 1400) lookAt(0.95f, -0.15f, 400);
      else if (t < 2000) lookAt(0.0f, 0.5f, 400);
      else lookAt(0, 0, 400);
      break;
    case IdleAnim::Curious:
      mods_.dyL = -8 * e;
      mods_.dyR = 8 * e;
      mods_.browR = 10 * e;
      mods_.browL = -3 * e;
      lookAt(0.35f, -0.2f, 300);
      break;
    case IdleAnim::Wiggle:
      lookAt(0.55f * sinf(t / 55.0f) * e, 0, 200);
      break;
    case IdleAnim::Yawn:
      if (t < 1000) {
        mods_.lid = 0.55f * min(1.0f, t / 700.0f);
      } else if (t < 1750) {
        mods_.hScale = 1.0f + 0.18f * e;
        mods_.browL = mods_.browR = 9 * e;
      } else if (!animBlinked_) {
        animBlinked_ = true;
        blink();
      }
      break;
    case IdleAnim::Peek:
      mods_.scale = 1.0f + 0.08f * e;
      mods_.happy = 0.6f * e;
      lookAt(0, 0, 300);
      break;
    default:
      break;
  }
}

void Eyes::update(uint32_t now) {
  const float dt = (now_ == 0) ? 16.0f : static_cast<float>(now - now_);
  now_ = now;
  if (dt <= 0) return;

  auto shapeApproach = [&](Shape &c, const Shape &t) {
    c.w = approach(c.w, t.w, dt, 90);
    c.h = approach(c.h, t.h, dt, 90);
    c.r = approach(c.r, t.r, dt, 90);
    c.slant = approach(c.slant, t.slant, dt, 110);
    c.lid = approach(c.lid, t.lid, dt, 140);
    c.happy = approach(c.happy, t.happy, dt, 120);
    c.yOff = approach(c.yOff, t.yOff, dt, 120);
    c.browY = approach(c.browY, t.browY, dt, 110);
    c.browAng = approach(c.browAng, t.browAng, dt, 110);
  };
  shapeApproach(curL_, tgt_.l);
  shapeApproach(curR_, tgt_.r);
  curGap_ = approach(curGap_, tgt_.gap, dt, 100);
  curBounce_ = approach(curBounce_, tgt_.bounce, dt, 200);
  curHearts_ = approach(curHearts_, tgt_.hearts, dt, 120);
  curSparkles_ = approach(curSparkles_, tgt_.sparkles, dt, 200);
  curZzz_ = approach(curZzz_, tgt_.zzz, dt, 300);
  curBrows_ = approach(curBrows_, (browsEnabled_ ? tgt_.brows : 0.0f), dt, 120);

  float tr, tg, tb;
  split565(tgt_.color, tr, tg, tb);
  colR_ = approach(colR_, tr, dt, 150);
  colG_ = approach(colG_, tg, dt, 150);
  colB_ = approach(colB_, tb, dt, 150);

  bootOpen_ = approach(bootOpen_, 1.0f, dt, 260);
  singAmt_ = approach(singAmt_, singMode_ ? 1.0f : 0.0f, dt, 220);
  mouthOpen_ = approach(mouthOpen_, mouthTarget(now), dt, 40);

  updateAnim(now);

  if (now > lookHoldUntil_ && now > nextSaccade_) {
    if ((esp_random() % 100) < 30) {
      lookTX_ = lookTY_ = 0;
    } else {
      lookTX_ = frand(-0.9f, 0.9f);
      lookTY_ = frand(-0.6f, 0.6f);
    }
    nextSaccade_ = now + static_cast<uint32_t>(urand(1200, 4200) / tgt_.saccadeRate);
  }
  lookX_ = approach(lookX_, lookTX_, dt, 45);
  lookY_ = approach(lookY_, lookTY_, dt, 45);

  if (!blinking_ && now > nextBlink_) {
    blink();
    nextBlink_ = now + static_cast<uint32_t>(urand(2200, 6000) / tgt_.blinkRate);
  }
}

void Eyes::drawEye(TFT_eSprite &s, int cx, int cy, const Shape &e, float scale, bool isLeft, uint16_t col) {
  const float w = e.w * scale;
  const float h = e.h * scale;
  if (w < 2 || h < 1) return;

  const int x = static_cast<int>(cx - w / 2);
  const int y = static_cast<int>(cy - h / 2 + e.yOff * scale);
  const int iw = static_cast<int>(w);
  const int ih = max(2, static_cast<int>(h));
  const int r = min(static_cast<int>(e.r * scale), min(iw / 2, ih / 2));

  s.fillSmoothRoundRect(x, y, iw, ih, r, col, TFT_BLACK);

  if (ih > 28 * scale) {
    const int gw = max(4, static_cast<int>(w * 0.17f));
    const int gh = max(4, static_cast<int>(h * 0.15f));
    s.fillSmoothRoundRect(x + static_cast<int>(w * 0.20f), y + static_cast<int>(h * 0.16f), gw, gh,
                          min(gw, gh) / 2, COL_GLINT, col);
  }

  int top = y;
  if (e.lid > 0.02f) {
    const int lh = static_cast<int>(h * min(e.lid, 0.95f));
    s.fillRect(x - 2, y - 2, iw + 4, lh + 2, TFT_BLACK);
    top = y + lh;
  }

  const int sl = static_cast<int>(e.slant * scale);
  if (sl > 1) {
    if (isLeft) s.fillTriangle(x - 2, top - 2, x + iw + 2, top - 2, x + iw + 2, top + sl, TFT_BLACK);
    else        s.fillTriangle(x - 2, top - 2, x + iw + 2, top - 2, x - 2, top + sl, TFT_BLACK);
  } else if (sl < -1) {
    const int d = -sl;
    if (isLeft) s.fillTriangle(x - 2, top - 2, x + iw + 2, top - 2, x - 2, top + d, TFT_BLACK);
    else        s.fillTriangle(x - 2, top - 2, x + iw + 2, top - 2, x + iw + 2, top + d, TFT_BLACK);
  }

  if (e.happy > 0.02f) {
    s.fillEllipse(cx, static_cast<int>(y + h + h * 0.15f), static_cast<int>(w * 0.72f),
                  static_cast<int>(h * 0.58f * min(e.happy, 1.0f)), TFT_BLACK);
  }
}

void Eyes::drawBrow(TFT_eSprite &s, int cx, int eyeTop, const Shape &e, float scale, bool isLeft,
                    float lift, uint16_t col) {
  const float halfW = e.w * scale * 0.36f;
  const float by = eyeTop - (e.browY + lift) * scale;
  const float dy = tanf(e.browAng * DEG_TO_RAD) * halfW;
  const float th = max(2.0f, 7.0f * scale * curBrows_);
  // Inner end is toward the face centre.
  const float outerX = isLeft ? cx - halfW : cx + halfW;
  const float innerX = isLeft ? cx + halfW : cx - halfW;
  s.drawWideLine(outerX, by - dy, innerX, by + dy, th, col, TFT_BLACK);
}

void Eyes::drawHeart(TFT_eSprite &s, int cx, int cy, float size, uint16_t col) {
  const float r = size / 4.2f;
  if (r < 2) return;
  s.fillSmoothCircle(static_cast<int>(cx - r * 0.95f), static_cast<int>(cy - r * 0.5f), static_cast<int>(r), col,
                     TFT_BLACK);
  s.fillSmoothCircle(static_cast<int>(cx + r * 0.95f), static_cast<int>(cy - r * 0.5f), static_cast<int>(r), col,
                     TFT_BLACK);
  s.fillTriangle(static_cast<int>(cx - r * 1.93f), static_cast<int>(cy - r * 0.15f),
                 static_cast<int>(cx + r * 1.93f), static_cast<int>(cy - r * 0.15f), cx,
                 static_cast<int>(cy + r * 2.2f), col);
  s.fillSmoothCircle(static_cast<int>(cx - r * 1.2f), static_cast<int>(cy - r * 0.85f),
                     max(2, static_cast<int>(r * 0.28f)), COL_GLINT, col);
}

void Eyes::drawSparkles(TFT_eSprite &s, int cx, int cy, float scale) {
  static const int8_t pos[4][2] = {{-120, -45}, {125, -35}, {-105, 45}, {112, 40}};
  for (int i = 0; i < 4; i++) {
    const float tw = fabsf(sinf(now_ / 210.0f + i * 1.7f));
    const int sz = static_cast<int>((2 + 5 * tw) * curSparkles_ * scale);
    if (sz < 2) continue;
    const int x = cx + static_cast<int>(pos[i][0] * scale);
    const int y = cy + static_cast<int>(pos[i][1] * scale);
    s.drawFastHLine(x - sz, y, 2 * sz + 1, COL_EYE_GOLD);
    s.drawFastVLine(x, y - sz, 2 * sz + 1, COL_EYE_GOLD);
    s.fillCircle(x, y, 1, COL_GLINT);
  }
}

void Eyes::drawZzz(TFT_eSprite &s, int x, int y) {
  s.setTextColor(COL_EYE_SLEEPY);
  s.setTextDatum(MC_DATUM);
  for (int i = 0; i < 3; i++) {
    const float ph = fmodf(now_ / 1000.0f + i * 0.7f, 2.1f) / 2.1f;
    if (ph > 0.85f) continue;
    s.drawString("z", x + i * 12 + static_cast<int>(sinf(ph * 6.0f) * 3), y - static_cast<int>(ph * 42),
                 i == 0 ? 2 : 4);
  }
}

void Eyes::drawMouth(TFT_eSprite &s, int cx, int y, float scale, uint16_t col) {
  const float a = singAmt_;
  const float o = constrain(mouthOpen_, 0.0f, 1.0f);
  const int w = static_cast<int>((20 + 12 * o) * scale * a);
  const int h = static_cast<int>((5 + 24 * o) * scale * a);
  if (w < 3 || h < 2) return;
  s.fillSmoothRoundRect(cx - w / 2, y - h / 2, w, h, min(w, h) / 2, col, TFT_BLACK);
  if (h > 9) {
    const int iw = w - 6, ih = h - 6;
    s.fillSmoothRoundRect(cx - iw / 2, y - ih / 2, iw, ih, min(iw, ih) / 2, TFT_BLACK, col);
    if (o > 0.5f) s.fillEllipse(cx, y + ih / 2 - 3, iw / 3, max(2, ih / 5), COL_EYE_LOVE);  // tongue
  }
}

void Eyes::drawNotes(TFT_eSprite &s, int cx, int cy, float scale, uint16_t col) {
  const float speed = singMode_ == 2 ? 1900.0f : 2800.0f;
  for (int i = 0; i < 3; i++) {
    const float ph = fmodf(now_ / speed + i / 3.0f, 1.0f);
    if (ph > 0.85f) continue;
    const int side = (i % 2) ? 1 : -1;
    const int x = cx + side * static_cast<int>((118 + 10 * sinf(ph * 9 + i)) * scale);
    const int y = cy + static_cast<int>((40 - ph * 95) * scale);
    const int r = max(2, static_cast<int>(4 * scale));
    s.fillSmoothCircle(x, y, r, col, TFT_BLACK);
    s.drawWideLine(x + r - 1, y, x + r - 1, y - 4 * r, max(1.5f, 2 * scale), col, TFT_BLACK);
    s.drawWideLine(x + r - 1, y - 4 * r, x + r + 2 * r, y - 3 * r, max(1.5f, 2 * scale), col, TFT_BLACK);
  }
}

void Eyes::draw(TFT_eSprite &s, int cx, int cy, float scale) {
  const uint32_t now = now_ ? now_ : millis();
  scale *= mods_.scale;

  const float b = max(0.04f, blinkFactor(now) * bootOpen_);
  const float phase = fmodf(static_cast<float>(now), kBeatMs) / kBeatMs;
  const float beat = fabsf(sinf(phase * PI)) * curBounce_;
  const float breathe = sinf(now / 1500.0f) * 2.0f;

  cy += static_cast<int>((breathe - 7.0f * beat) * scale);
  const int baseCx = cx;
  cx += static_cast<int>(7.0f * sinf(2.0f * PI * now / (2.0f * kBeatMs)) * singAmt_ * scale);  // sway
  const float lpx = lookX_ * 34.0f * scale;
  const float lpy = lookY_ * 14.0f * scale;
  const float lScale = scale * (1.0f - 0.07f * lookX_);
  const float rScale = scale * (1.0f + 0.07f * lookX_);
  const uint16_t col = join565(colR_, colG_, colB_);

  const int lCx = static_cast<int>(cx - (curGap_ / 2 + curL_.w / 2) * scale + lpx);
  const int rCx = static_cast<int>(cx + (curGap_ / 2 + curR_.w / 2) * scale + lpx);
  const int eyeY = static_cast<int>(cy + lpy);

  if (curHearts_ > 0.5f) {
    const float pulse = 1.0f + 0.07f * sinf(now / 170.0f);
    const float sz = 80.0f * scale * pulse * ((curHearts_ - 0.5f) * 2.0f) * b;
    drawHeart(s, lCx, eyeY, sz, col);
    drawHeart(s, rCx, eyeY, sz, col);
  } else {
    Shape l = curL_, r = curR_;
    const float squash = (1.0f - 0.06f * beat) * mods_.hScale;
    l.h *= b * squash;
    r.h *= b * squash;
    l.w *= 1.0f + 0.04f * beat;
    r.w *= 1.0f + 0.04f * beat;
    l.lid = min(0.95f, l.lid + mods_.lid);
    r.lid = min(0.95f, r.lid + mods_.lid);
    l.happy = min(1.0f, l.happy + mods_.happy);
    r.happy = min(1.0f, r.happy + mods_.happy);
    l.yOff += mods_.dyL;
    r.yOff += mods_.dyR;

    drawEye(s, lCx, eyeY, l, lScale, true, col);
    drawEye(s, rCx, eyeY, r, rScale, false, col);

    if (curBrows_ > 0.15f) {
      // Brows sit above the *unblinked* eye top so they stay put during blinks.
      const int lTop = static_cast<int>(eyeY - curL_.h * lScale * mods_.hScale / 2 + l.yOff * lScale);
      const int rTop = static_cast<int>(eyeY - curR_.h * rScale * mods_.hScale / 2 + r.yOff * rScale);
      drawBrow(s, lCx, lTop, l, lScale, true, mods_.browL, col);
      drawBrow(s, rCx, rTop, r, rScale, false, mods_.browR, col);
    }
  }

  if (singAmt_ > 0.05f && curHearts_ <= 0.5f) {
    drawMouth(s, cx, eyeY + static_cast<int>(50 * scale), scale, col);
    drawNotes(s, baseCx, cy, scale, col);
  }
  if (curSparkles_ > 0.1f) drawSparkles(s, cx, cy, scale);
  if (curZzz_ > 0.3f) drawZzz(s, rCx + static_cast<int>(40 * scale), eyeY - static_cast<int>(40 * scale));
}
