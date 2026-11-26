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

// ---- Palette (RGB565) ----
constexpr uint16_t COL_EYE_ICE    = 0x7F3F;  // (120,230,255)
constexpr uint16_t COL_EYE_FOCUS  = 0xB6FF;  // (180,220,255)
constexpr uint16_t COL_EYE_MUSIC  = 0x6F94;  // (110,240,160)
constexpr uint16_t COL_EYE_GOLD   = 0xFEAB;  // (255,214,90)
constexpr uint16_t COL_EYE_ANGRY  = 0xFA88;  // (255,80,70)
constexpr uint16_t COL_EYE_LOVE   = 0xFB56;  // (255,105,180)
constexpr uint16_t COL_EYE_SLEEPY = 0x5919;  // (90,160,200)
constexpr uint16_t COL_GLINT      = 0xFFFF;
constexpr uint16_t COL_DIM        = 0x2104;  // dark grey lines
constexpr uint16_t COL_SUBTLE     = 0x8410;  // mid grey text
constexpr uint16_t COL_TEXT       = 0xFFFF;
constexpr uint16_t COL_SAY        = 0xFEAB;  // warm gold for hype lines
