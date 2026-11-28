#pragma once

#include <TFT_eSPI.h>

class Brain;

// Full-canvas (320x150) screens drawn into the top sprite.
void drawClockView(TFT_eSprite &s, const Brain &brain, uint32_t now);
void drawFocusView(TFT_eSprite &s, const Brain &brain, uint32_t now);  // pomodoro / timer

// Dot-matrix (5x7 LED) text. `pitch` = px per dot cell. Returns width in px.
int dotTextWidth(const char *txt, int pitch);
void dotText(TFT_eSprite &s, const char *txt, int x, int y, int pitch, uint16_t on, uint16_t off, bool colonOn = true);
