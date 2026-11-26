#pragma once

#include <Arduino.h>

#include "eyes.h"
#include "mood.h"
#include "music.h"
#include "panel.h"

// One update from the laptop companion (all fields optional).
struct StateMsg {
  String name;
  bool hasActivity = false;
  String label, category;
  int minutes = 0;
  bool hasMusic = false;
  bool playing = false;
  String track, artist;
  uint32_t posMs = 0, durMs = 0;
  uint16_t color = 0;
  bool hasLyrics = false;
  String lyricCur, lyricNext;
  uint32_t lyricLineMs = 0;
  bool hasTask = false;
  String taskTitle;
  int taskEst = 0, tasksLeft = 0;
  bool hasMood = false;
  Mood mood = Mood::Idle;
  bool hasSay = false;
  String sayText;
  Mood sayMood = Mood::Excited;
  uint32_t sayMs = 6000;
};

enum class PomoPhase : uint8_t { Off, Focus, Break };

// Decides what the pet feels and shows.
class Brain {
 public:
  Brain(Eyes &eyes, Panel &panel, NowPlaying &music) : eyes_(eyes), panel_(panel), music_(music) {}

  void begin();
  void apply(const StateMsg &msg);
  void say(const String &text, Mood mood, uint32_t ms);
  void setTempMood(Mood m, uint32_t ms);
  void setClaude(const String &state, const String &project);
  bool pomodoro(const String &action);  // start|stop|skip
  void update(uint32_t now);

  bool helperOnline(uint32_t now) const;
  bool showMusicView(uint32_t now) const;
  ClaudeState claude() const { return claude_; }
  uint32_t claudeSince() const { return claudeSince_; }
  String statusJson(uint32_t now) const;
  // Events for the laptop (sounds), drained by each /api/state response.
  void pushEvent(const char *ev);
  String takeEventsJson();

 private:
  Mood resolveMood(uint32_t now);
  void offlineHype(uint32_t now);
  void tickPomodoro(uint32_t now);
  void tickClaude(uint32_t now);

  Eyes &eyes_;
  Panel &panel_;
  NowPlaying &music_;

  uint32_t lastSeen_ = 0;
  bool wasOnline_ = false;
  String category_;
  bool hasSuggested_ = false;
  Mood suggested_ = Mood::Idle;

  Mood tempMood_ = Mood::Idle;
  uint32_t tempUntil_ = 0;
  uint32_t nextQuirk_ = 0;
  uint32_t nextAnim_ = 0;
  uint32_t nextOfflineHype_ = 0;

  ClaudeState claude_ = ClaudeState::None;
  uint32_t claudeSince_ = 0;
  uint32_t nextThinkGlance_ = 0;

  PomoPhase pomo_ = PomoPhase::Off;
  uint32_t pomoEnd_ = 0, pomoTotal_ = 0;
  int pomoRound_ = 1;

  String events_[8];
  uint8_t eventCount_ = 0;
};
