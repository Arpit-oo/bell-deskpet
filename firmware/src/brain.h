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

// What the top canvas shows. Auto = face, music card when a track starts.
enum class View : uint8_t { Auto, Face, Music, Clock, Focus };
const char *viewName(View v);

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
  bool timer(const String &action, uint32_t seconds);  // start|add|stop

  Mood currentMood() const { return eyes_.mood(); }
  PomoPhase pomoPhase() const { return pomo_; }
  int pomoRound() const { return pomoRound_; }
  uint32_t pomoRemainMs(uint32_t now) const { return static_cast<int32_t>(pomoEnd_ - now) > 0 ? pomoEnd_ - now : 0; }
  uint32_t pomoTotalMs() const { return pomoTotal_; }
  uint32_t pomoFocusMs() const;
  bool timerOn() const { return timerOn_; }
  uint32_t timerRemainMs(uint32_t now) const { return static_cast<int32_t>(timerEnd_ - now) > 0 ? timerEnd_ - now : 0; }
  uint32_t timerTotalMs() const { return timerTotal_; }
  bool timerDoneRecently(uint32_t now) const { return timerDoneAt_ && now - timerDoneAt_ < 10000; }
  void update(uint32_t now);

  bool helperOnline(uint32_t now) const;
  bool showMusicView(uint32_t now) const;
  View topView(uint32_t now) const;          // resolved view (never Auto)
  View cycleView(uint32_t now);              // next view, pinned for a while
  bool setView(const String &name, uint32_t now);
  const String &taskTitle() const { return taskTitle_; }
  int tasksLeft() const { return tasksLeft_; }
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
  void tickTimer(uint32_t now);
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

  bool timerOn_ = false;
  uint32_t timerEnd_ = 0, timerTotal_ = 0, timerDoneAt_ = 0;

  View pinned_ = View::Auto;
  uint32_t pinnedUntil_ = 0;
  String taskTitle_;
  int tasksLeft_ = 0;

  String events_[8];
  uint8_t eventCount_ = 0;
};
