#include "settings.h"

#include "config.h"

Settings g_settings;

namespace {

struct Theme {
  const char *id;
  uint16_t eye;
  uint16_t accent;
};

const Theme kThemes[] = {
    {"ice", 0x7F3F, 0xFEAB},     // icy cyan + gold
    {"mint", 0x7FF7, 0xFD4F},    // mint + peach
    {"sunset", 0xFCAB, 0xFAD1},  // orange + hot pink
    {"sakura", 0xFD1A, 0xB4BF},  // pink + lilac
    {"matrix", 0x57EF, 0x57EF},  // terminal green
    {"mono", 0xFFFF, 0xCE59},    // white
};

}  // namespace

bool Settings::themeColors(const String &theme, uint16_t &eye, uint16_t &accent) {
  for (const Theme &t : kThemes) {
    if (theme == t.id) {
      eye = t.eye;
      accent = t.accent;
      return true;
    }
  }
  return false;
}

String Settings::toHex(uint16_t c) {
  const uint8_t r = ((c >> 11) & 0x1F) * 255 / 31;
  const uint8_t g = ((c >> 5) & 0x3F) * 255 / 63;
  const uint8_t b = (c & 0x1F) * 255 / 31;
  char buf[8];
  snprintf(buf, sizeof(buf), "#%02x%02x%02x", r, g, b);
  return String(buf);
}

bool Settings::fromHex(const char *hex, uint16_t &out) {
  if (!hex) return false;
  if (*hex == '#') hex++;
  if (strlen(hex) != 6) return false;
  char *end = nullptr;
  const long v = strtol(hex, &end, 16);
  if (end == nullptr || *end != '\0') return false;
  const uint8_t r = (v >> 16) & 0xFF, g = (v >> 8) & 0xFF, b = v & 0xFF;
  out = ((r >> 3) << 11) | ((g >> 2) << 5) | (b >> 3);
  return true;
}

void Settings::load(Preferences &p) {
  rotation = p.getUChar("rot", 1) == 3 ? 3 : 1;
  theme = p.getString("theme", "ice");
  brows = p.getBool("brows", true);
  quirks = p.getBool("quirks", true);
  lyrics = p.getBool("lyrics", true);
  sing = p.getBool("sing", true);
  hype = min<uint8_t>(p.getUChar("hype", 1), 2);
  name = p.getString("name", PET_DEFAULT_NAME);
  pomoFocus = constrain(p.getUChar("pfocus", 25), 1, 90);
  pomoBreak = constrain(p.getUChar("pbreak", 5), 1, 30);
  if (!themeColors(theme, eyeColor, accent)) {
    eyeColor = p.getUShort("eye", COL_EYE_ICE);
    accent = p.getUShort("accent", COL_SAY);
    theme = "custom";
  }
}

void Settings::save(Preferences &p) const {
  p.putUChar("rot", rotation);
  p.putString("theme", theme);
  p.putUShort("eye", eyeColor);
  p.putUShort("accent", accent);
  p.putBool("brows", brows);
  p.putBool("quirks", quirks);
  p.putBool("lyrics", lyrics);
  p.putBool("sing", sing);
  p.putUChar("hype", hype);
  p.putString("name", name);
  p.putUChar("pfocus", pomoFocus);
  p.putUChar("pbreak", pomoBreak);
}

String Settings::toJson() const {
  JsonDocument d;
  d["flip"] = rotation == 3;
  d["theme"] = theme;
  d["eye"] = toHex(eyeColor);
  d["accent"] = toHex(accent);
  d["brows"] = brows;
  d["quirks"] = quirks;
  d["lyrics"] = lyrics;
  d["sing"] = sing;
  d["hype"] = hype;
  d["name"] = name;
  d["pomo_focus"] = pomoFocus;
  d["pomo_break"] = pomoBreak;
  String out;
  serializeJson(d, out);
  return out;
}

bool Settings::apply(JsonVariantConst j) {
  bool changed = false;
  if (j["flip"].is<bool>()) {
    const uint8_t r = j["flip"].as<bool>() ? 3 : 1;
    changed |= r != rotation;
    rotation = r;
  }
  if (j["theme"].is<const char *>()) {
    const String t = j["theme"].as<const char *>();
    uint16_t e, a;
    if (themeColors(t, e, a)) {
      theme = t;
      eyeColor = e;
      accent = a;
      changed = true;
    }
  }
  uint16_t c;
  if (j["eye"].is<const char *>() && fromHex(j["eye"].as<const char *>(), c)) {
    eyeColor = c;
    theme = "custom";
    changed = true;
  }
  if (j["accent"].is<const char *>() && fromHex(j["accent"].as<const char *>(), c)) {
    accent = c;
    theme = "custom";
    changed = true;
  }
  if (j["brows"].is<bool>()) { brows = j["brows"]; changed = true; }
  if (j["quirks"].is<bool>()) { quirks = j["quirks"]; changed = true; }
  if (j["lyrics"].is<bool>()) { lyrics = j["lyrics"]; changed = true; }
  if (j["sing"].is<bool>()) { sing = j["sing"]; changed = true; }
  if (j["hype"].is<int>()) { hype = constrain(j["hype"].as<int>(), 0, 2); changed = true; }
  if (j["name"].is<const char *>()) {
    String n = j["name"].as<const char *>();
    n.trim();
    if (n.length() && n.length() <= 20) { name = n; changed = true; }
  }
  if (j["pomo_focus"].is<int>()) { pomoFocus = constrain(j["pomo_focus"].as<int>(), 1, 90); changed = true; }
  if (j["pomo_break"].is<int>()) { pomoBreak = constrain(j["pomo_break"].as<int>(), 1, 30); changed = true; }
  return changed;
}
