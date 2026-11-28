#pragma once

#include <Arduino.h>

// ---- Identity / network ----
#define PET_HOSTNAME     "deskpet"
#define PET_OTA_PASSWORD "deskpet"
#define PET_SETUP_AP     "DeskPet-Setup"
#define PET_DEFAULT_NAME "Arpit"

// ---- Screen layout (landscape 320x240) ----
constexpr int SCREEN_W = 320;
constexpr int SCREEN_H = 240;
constexpr int EYES_W   = 320;
constexpr int EYES_H   = 150;   // eye sprite occupies the top
constexpr int PANEL_Y  = EYES_H;
constexpr int PANEL_W  = 320;
constexpr int PANEL_H  = SCREEN_H - EYES_H;  // 90px info panel

// ---- Timing ----
constexpr uint32_t EYE_FRAME_MS      = 33;     // ~30fps target
constexpr uint32_t PANEL_ANIM_MS     = 40;     // marquee / eq / typewriter
constexpr uint32_t HELPER_TIMEOUT_MS = 20000;  // companion considered offline after this
constexpr long     TZ_OFFSET_S       = 19800;  // IST (UTC+5:30)
constexpr int      BIRTH_Y = 2004, BIRTH_M = 12, BIRTH_D = 23;  // life-in-weeks screen
constexpr int      LIFE_YEARS = 90;

// ---- Palette (RGB565) ----
// Monochrome: white face and text on pure black (matches the desktop pet).
constexpr uint16_t COL_MONO       = 0xFFDE;  // (248,248,246)
constexpr uint16_t COL_EYE_ICE = COL_MONO;
constexpr uint16_t COL_EYE_FOCUS = COL_MONO;
constexpr uint16_t COL_EYE_MUSIC = COL_MONO;
constexpr uint16_t COL_EYE_GOLD = COL_MONO;
constexpr uint16_t COL_EYE_ANGRY = COL_MONO;
constexpr uint16_t COL_EYE_LOVE = COL_MONO;
constexpr uint16_t COL_EYE_SLEEPY = COL_MONO;
constexpr uint16_t COL_GLINT      = 0xFFFF;
constexpr uint16_t COL_DIM        = 0x2104;  // dark grey lines
constexpr uint16_t COL_SUBTLE     = 0x8410;  // mid grey text
constexpr uint16_t COL_TEXT       = 0xFFFF;
constexpr uint16_t COL_SAY = COL_MONO;
