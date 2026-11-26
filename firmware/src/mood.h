#pragma once

#include <Arduino.h>

enum class Mood : uint8_t {
  Idle,
  Happy,
  Excited,
  Focus,
  Music,
  Sleepy,
  Sus,
  Sad,
  Angry,
  Surprised,
  Love,
  Count
};

inline const char *moodName(Mood m) {
  static const char *names[] = {"idle", "happy", "excited", "focus", "music", "sleepy",
                                "sus",  "sad",   "angry",   "surprised", "love"};
  const uint8_t i = static_cast<uint8_t>(m);
  return i < static_cast<uint8_t>(Mood::Count) ? names[i] : "idle";
}

// Returns false if the string is not a known mood (out stays untouched).
inline bool moodFromString(const String &s, Mood &out) {
  String v = s;
  v.toLowerCase();
  for (uint8_t i = 0; i < static_cast<uint8_t>(Mood::Count); i++) {
    if (v == moodName(static_cast<Mood>(i))) {
      out = static_cast<Mood>(i);
      return true;
    }
  }
  return false;
}
