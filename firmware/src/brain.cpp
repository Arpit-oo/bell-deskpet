#include "brain.h"

#include <time.h>

#include "config.h"
#include "settings.h"

namespace {

uint32_t urand(uint32_t lo, uint32_t hi) { return lo + (esp_random() % (hi - lo + 1)); }

const char *const kOfflineLines[] = {
    "Let's go, %s!",        "You got this, %s.",      "Small steps. Big wins.",
    "Focus. Ship. Repeat.", "One more push, %s!",     "Proud of you, fr.",
    "Hydrate. Then dominate.", "Future you says thanks.",
};
constexpr size_t kOfflineLineCount = sizeof(kOfflineLines) / sizeof(kOfflineLines[0]);

// Rough syllable count: vowel groups.
uint16_t syllables(const String &s) {
  uint16_t n = 0;
  bool prevVowel = false;
  for (size_t i = 0; i < s.length(); i++) {
    const char c = tolower(s[i]);
    const bool v = strchr("aeiouy", c) != nullptr && c != 0;
    if (v && !prevVowel) n++;
    prevVowel = v;
  }
  return max<uint16_t>(n, s.length() ? 1 : 0);
}

constexpr uint32_t kClaudeWorkingExpire = 20UL * 60UL * 1000UL;
constexpr uint32_t kClaudeWaitingExpire = 30UL * 60UL * 1000UL;

}  // namespace

void Brain::begin() {
  const uint32_t now = millis();
  nextQuirk_ = now + 40000;
  nextAnim_ = now + 12000;
  nextOfflineHype_ = now + 10UL * 60UL * 1000UL;
  say("Hey " + g_settings.name + "! Let's go!", Mood::Excited, 5000);
}

bool Brain::helperOnline(uint32_t now) const {
  return lastSeen_ != 0 && (now - lastSeen_) < HELPER_TIMEOUT_MS;
}

void Brain::apply(const StateMsg &msg) {
  lastSeen_ = millis();
  if (msg.hasActivity) {
    category_ = msg.category;
    panel_.setActivity(msg.label, msg.category, msg.minutes);
  }
  if (msg.hasMusic) {
    music_.setTrack(msg.playing, msg.track, msg.artist, msg.posMs, msg.durMs,
                    COL_MONO);
  }
  if (msg.hasLyrics) music_.setLyrics(msg.lyricCur, msg.lyricNext, msg.lyricLineMs);
  if (msg.hasTask) panel_.setTask(msg.taskTitle, msg.taskEst, msg.tasksLeft);
  hasSuggested_ = msg.hasMood;
  if (msg.hasMood) suggested_ = msg.mood;
  if (msg.hasSay) say(msg.sayText, msg.sayMood, msg.sayMs);
}

void Brain::say(const String &text, Mood mood, uint32_t ms) {
  ms = constrain(ms, 1500UL, 20000UL);
  panel_.say(text, ms);
  setTempMood(mood, ms);
}

void Brain::setTempMood(Mood m, uint32_t ms) {
  tempMood_ = m;
  tempUntil_ = millis() + ms;
  if (m == Mood::Sus) eyes_.lookAt(-0.9f, 0.1f, ms);
}

void Brain::setClaude(const String &state, const String &project) {
  const uint32_t now = millis();
  if (state == "working") {
    if (claude_ != ClaudeState::Working) claudeSince_ = now;
    claude_ = ClaudeState::Working;
  } else if (state == "waiting") {
    if (claude_ != ClaudeState::Waiting) {
      claudeSince_ = now;
      eyes_.lookAt(0, 0.2f, 3000);
      eyes_.blink();
    }
    claude_ = ClaudeState::Waiting;
  } else if (state == "done") {
    claude_ = ClaudeState::None;
    say("Claude finished! Go check it.", Mood::Happy, 5000);
  } else {
    claude_ = ClaudeState::None;
  }
  panel_.setClaude(claude_, project);
}

bool Brain::pomodoro(const String &action) {
  const uint32_t now = millis();
  if (action == "start") {
    pomo_ = PomoPhase::Focus;
    pomoTotal_ = g_settings.pomoFocus * 60000UL;
    pomoEnd_ = now + pomoTotal_;
    pomoRound_ = 1;
    say("Pomodoro on. Lock in!", Mood::Excited, 3500);
    pushEvent("pomo_start");
    return true;
  }
  if (action == "stop") {
    pomo_ = PomoPhase::Off;
    return true;
  }
  if (action == "skip" && pomo_ != PomoPhase::Off) {
    pomoEnd_ = now;  // tickPomodoro advances the phase
    return true;
  }
  return false;
}

void Brain::tickPomodoro(uint32_t now) {
  if (pomo_ == PomoPhase::Off) {
    panel_.setPomodoro(false, true, 0, 1, 0);
    return;
  }
  if (static_cast<int32_t>(now - pomoEnd_) >= 0) {
    if (pomo_ == PomoPhase::Focus) {
      pomo_ = PomoPhase::Break;
      pomoTotal_ = g_settings.pomoBreak * 60000UL;
      say("Pomodoro done! Break time, " + g_settings.name + ".", Mood::Love, 7000);
      pushEvent("pomo_break");
    } else {
      pomo_ = PomoPhase::Focus;
      pomoTotal_ = g_settings.pomoFocus * 60000UL;
      pomoRound_++;
      say("Break over. Let's go!", Mood::Excited, 5000);
      pushEvent("pomo_focus");
    }
    pomoEnd_ = now + pomoTotal_;
  }
  panel_.setPomodoro(true, pomo_ == PomoPhase::Focus, pomoEnd_ - now, pomoTotal_, pomoRound_);
}

void Brain::tickClaude(uint32_t now) {
  if (claude_ == ClaudeState::Working && now - claudeSince_ > kClaudeWorkingExpire) setClaude("none", "");
  if (claude_ == ClaudeState::Waiting && now - claudeSince_ > kClaudeWaitingExpire) setClaude("none", "");
  if (claude_ == ClaudeState::Working && now > nextThinkGlance_) {
    // "Thinking" glance up and to the side.
    eyes_.lookAt((esp_random() % 2) ? 0.7f : -0.7f, -0.75f, 1800);
    nextThinkGlance_ = now + urand(2500, 5000);
  }
}

bool Brain::showMusicView(uint32_t now) const {
  if (!music_.playing() || !music_.hasTrack()) return false;
  if (claude_ == ClaudeState::Waiting || pomo_ == PomoPhase::Focus) return false;
  const uint32_t t = now - music_.trackChangedAt();
  if (t < 25000) return true;                 // fresh track: show it off
  return ((t - 25000) % 60000) < 15000;       // then 15 s of every minute
}

Mood Brain::resolveMood(uint32_t now) {
  if (now < tempUntil_) return tempMood_;

  if (claude_ == ClaudeState::Waiting) return (now - claudeSince_ < 1500) ? Mood::Surprised : Mood::Happy;
  if (claude_ == ClaudeState::Working) return Mood::Focus;
  if (pomo_ == PomoPhase::Focus) return Mood::Focus;
  if (pomo_ == PomoPhase::Break) return Mood::Happy;

  const bool online = helperOnline(now);
  if (online && category_ == "away") return Mood::Sleepy;
  if (online && music_.playing()) return Mood::Music;
  if (online && hasSuggested_) return suggested_;

  if (!online) {
    struct tm tmv;
    if (getLocalTime(&tmv, 0) && tmv.tm_hour >= 1 && tmv.tm_hour < 6) return Mood::Sleepy;
  }

  if (g_settings.quirks) {
    if (now > nextQuirk_) {
      static const Mood quirks[] = {Mood::Happy, Mood::Love, Mood::Sus, Mood::Surprised, Mood::Excited};
      setTempMood(quirks[esp_random() % 5], 2600);
      nextQuirk_ = now + urand(35000, 80000);
      return tempMood_;
    }
    if (now > nextAnim_ && !eyes_.animating()) {
      static const IdleAnim anims[] = {IdleAnim::LookAround, IdleAnim::Curious, IdleAnim::Wiggle,
                                       IdleAnim::Yawn, IdleAnim::Peek};
      eyes_.play(anims[esp_random() % 5]);
      nextAnim_ = now + urand(9000, 22000);
    }
  }
  return Mood::Idle;
}

void Brain::offlineHype(uint32_t now) {
  if (now < nextOfflineHype_) return;
  nextOfflineHype_ = now + urand(12, 20) * 60UL * 1000UL;
  char buf[64];
  snprintf(buf, sizeof(buf), kOfflineLines[esp_random() % kOfflineLineCount], g_settings.name.c_str());
  say(buf, Mood::Excited, 6000);
}

void Brain::update(uint32_t now) {
  const bool online = helperOnline(now);
  panel_.setHelperOnline(online);
  panel_.setShowLyrics(g_settings.lyrics);

  if (wasOnline_ && !online) {
    hasSuggested_ = false;
    category_ = "";
    music_.clear();
    panel_.setActivity("", "", 0);
  }
  wasOnline_ = online;

  if (!online) offlineHype(now);
  tickPomodoro(now);
  tickClaude(now);
  eyes_.setBrows(g_settings.brows);

  // Sing along: lip-sync when we have a lyric line, hum otherwise.
  uint8_t sing = 0;
  if (g_settings.sing && online && music_.playing() && claude_ != ClaudeState::Waiting &&
      pomo_ != PomoPhase::Focus && now >= tempUntil_) {
    sing = music_.lyricCur().length() ? 2 : 1;
  }
  eyes_.setSing(sing, music_.lyricChangedAt(), syllables(music_.lyricCur()), music_.lyricLineMs());
  eyes_.setMood(resolveMood(now));
}

void Brain::pushEvent(const char *ev) {
  if (eventCount_ < 8) events_[eventCount_++] = ev;
}

String Brain::takeEventsJson() {
  String s = "[";
  for (uint8_t i = 0; i < eventCount_; i++) {
    if (i) s += ",";
    s += "\"" + events_[i] + "\"";
  }
  s += "]";
  eventCount_ = 0;
  return s;
}

String Brain::statusJson(uint32_t now) const {
  static const char *claudeNames[] = {"none", "working", "waiting"};
  static const char *pomoNames[] = {"off", "focus", "break"};
  String s = "{\"ok\":true,\"mood\":\"";
  s += moodName(eyes_.mood());
  s += "\",\"companion\":";
  s += helperOnline(now) ? "true" : "false";
  s += ",\"claude\":\"";
  s += claudeNames[static_cast<uint8_t>(claude_)];
  s += "\",\"pomodoro\":{\"phase\":\"";
  s += pomoNames[static_cast<uint8_t>(pomo_)];
  s += "\",\"remaining_s\":";
  s += pomo_ == PomoPhase::Off ? 0 : (pomoEnd_ - now) / 1000;
  s += ",\"round\":";
  s += pomoRound_;
  s += "},\"music\":{\"playing\":";
  s += music_.playing() ? "true" : "false";
  s += ",\"track\":\"";
  String tr = music_.track();
  tr.replace("\\", "/");
  tr.replace("\"", "'");
  s += tr;
  s += "\"},\"heap\":";
  s += ESP.getFreeHeap();
  s += ",\"uptime_s\":";
  s += now / 1000;
  s += "}";
  return s;
}
