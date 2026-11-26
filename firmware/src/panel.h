#pragma once

#include <TFT_eSPI.h>

#include "music.h"

enum class ClaudeState : uint8_t { None, Working, Waiting };

// Bottom info strip. Priority: Claude waiting > hype line > pomodoro > music > activity > clock.
class Panel {
 public:
  Panel(TFT_eSPI &tft, NowPlaying &music);

  bool begin();
  void markDirty() { dirty_ = true; }

  void setActivity(const String &label, const String &category, int minutes);
  void setHelperOnline(bool online);
  void say(const String &text, uint32_t durationMs);
  void setPomodoro(bool active, bool focus, uint32_t remainingMs, uint32_t totalMs, int round);
  void setClaude(ClaudeState st, const String &project);
  void setShowLyrics(bool on);
  void setTask(const String &title, int estMin, int left);

  // Panel only shows while it has something to say; otherwise the face takes the
  // whole screen. `musicView` = vinyl view is on (lyrics live in the panel then).
  bool wantsVisible(uint32_t now, bool musicView) const;
  void peek(uint32_t ms);  // show the current info for a moment
  void setHidden(bool h);

  void update(uint32_t now);

 private:
  enum class Mode : uint8_t { Claude, Say, Pomodoro, Music, Activity, Clock };

  Mode currentMode(uint32_t now) const;
  void drawClaude(uint32_t now);
  void drawSay(uint32_t now);
  void drawPomodoro(uint32_t now);
  void drawMusic(uint32_t now);
  void drawLyrics(uint32_t now);
  void drawActivity(uint32_t now);
  void drawClock(uint32_t now);
  void drawClockSmall(int rightX, int y);
  void drawEq(int x, int baseY, int bars, int barW, int maxH, uint16_t col);
  void wrap(const String &text, const GFXfont *const *fonts, int nFonts, int maxW, String lines[2], int &count,
            const GFXfont *&font);

  static uint16_t categoryColor(const String &cat);
  static String activitySubline(const String &cat, int minutes);

  TFT_eSPI &tft_;
  NowPlaying &music_;
  TFT_eSprite spr_;
  bool ok_ = false;
  bool dirty_ = true;
  bool hidden_ = false;
  uint32_t peekUntil_ = 0;
  uint32_t lastDraw_ = 0;
  int lastMinute_ = -1;
  Mode lastMode_ = Mode::Clock;

  String actLabel_, actCat_;
  int actMinutes_ = 0;
  bool helperOnline_ = false;
  bool showLyrics_ = true;

  String sayLines_[2];
  int sayCount_ = 0;
  const GFXfont *sayFont_ = nullptr;
  uint32_t sayStart_ = 0, sayUntil_ = 0;

  bool pomoActive_ = false, pomoFocus_ = true;
  uint32_t pomoRemain_ = 0, pomoTotal_ = 1;
  int pomoRound_ = 1;
  uint32_t lastPomoSec_ = 0;

  ClaudeState claude_ = ClaudeState::None;
  String claudeProject_;

  String taskTitle_;
  int taskEst_ = 0, tasksLeft_ = 0;
  void drawTaskLine(int y);

  String lyricShown_;
  String lyricLines_[2];
  int lyricCount_ = 0;
  const GFXfont *lyricFont_ = nullptr;

  float eq_[12] = {0};
  float eqT_[12] = {0};
  uint32_t eqNext_ = 0;
};
