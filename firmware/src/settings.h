#pragma once

#include <Arduino.h>
#include <ArduinoJson.h>
#include <Preferences.h>

// User-tunable look & behaviour, persisted in NVS, edited from the web app.
struct Settings {
  uint8_t rotation = 1;      // 1 = normal landscape, 3 = flipped 180
  String theme = "ice";      // ice|mint|sunset|sakura|matrix|mono|custom
  uint16_t eyeColor = 0;     // resolved from theme (or custom)
  uint16_t accent = 0;
  bool brows = true;
  bool quirks = true;        // random idle expressions / animations
  bool lyrics = true;
  bool sing = true;         // sing / hum along with music
  uint8_t hype = 1;          // 0 chill, 1 normal, 2 max (read by companion)
  String name;
  uint8_t pomoFocus = 25;
  uint8_t pomoBreak = 5;
  bool fullscreen = false;  // never show the bottom strip

  void load(Preferences &p);
  void save(Preferences &p) const;
  String toJson() const;
  // Applies known keys; returns true if anything changed.
  bool apply(JsonVariantConst j);

  static bool themeColors(const String &theme, uint16_t &eye, uint16_t &accent);
  static String toHex(uint16_t c565);
  static bool fromHex(const char *hex, uint16_t &out);
};

extern Settings g_settings;
