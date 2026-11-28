// Desk Pet firmware: Tabbie-style eyes + hype panel, controlled over WiFi by the
// Rust laptop companion (companion/) and the built-in web app.
//
// HTTP API (port 80, JSON bodies with Content-Type: application/json):
//   GET  /                web app (settings, moods, pomodoro)
//   GET  /api/status      live state
//   GET  /api/settings    | POST /api/settings  {flip,theme,eye,accent,brows,quirks,lyrics,hype,name,pomo_focus,pomo_break}
//   POST /api/state       companion update (see StateMsg / companion/src/main.rs)
//   POST /api/say         {"text":"LET'S GO","mood":"excited","ms":6000}
//   POST /api/mood        {"mood":"love","ms":5000}
//   POST /api/anim        {"name":"lookaround|curious|wiggle|yawn|peek"}
//   POST /api/claude      {"state":"working|waiting|done|none","project":"bell"}
//   POST /api/pomodoro    {"action":"start|stop|skip"}
//   POST /api/art         {"data":"<base64 56x56 big-endian RGB565>"}

#include <Arduino.h>
#include <ArduinoJson.h>
#include <ArduinoOTA.h>
#include <ESPmDNS.h>
#include <Preferences.h>
#include <TFT_eSPI.h>
#include <WebServer.h>
#include <WiFi.h>
#include <WiFiManager.h>
#include <mbedtls/base64.h>

#include "brain.h"
#include "config.h"
#include "eyes.h"
#include "music.h"
#include "views.h"
#include "panel.h"
#include "settings.h"
#include "webapp.h"

static TFT_eSPI tft;
static TFT_eSprite top(&tft);  // 320x150 canvas for face / vinyl view
static Eyes eyes;
static NowPlaying music(tft);
static Panel panel(tft, music);
static Brain brain(eyes, panel, music);
static WebServer server(80);
static WiFiManager wm;
static Preferences prefs;

static bool g_netUp = false;
static bool g_otaActive = false;
static bool g_topOk = false;
static uint32_t g_lastFrame = 0;
static View g_lastView = View::Face;
static uint32_t g_viewSwitchAt = 0;

// Face-full layout: 0 = face on top + info panel, 1 = face centred on the whole
// screen. The panel slides in only while it has something to say.
static float g_full = 0.0f;
static int g_topY = 0;
static uint32_t g_layoutAt = 0;
constexpr int kFullTopY = (SCREEN_H - EYES_H) / 2;  // 45
constexpr float kLayoutMs = 380.0f;

constexpr uint16_t kClaudeOrange = 0xD3AA;

// ---------------------------------------------------------------- rendering

static void applyLook() {
  tft.setRotation(g_settings.rotation);
  tft.fillScreen(TFT_BLACK);
  eyes.setBaseColor(COL_MONO);
  panel.markDirty();
}

static void drawClaudeBadge(uint32_t now) {
  const ClaudeState st = brain.claude();
  if (st == ClaudeState::None) return;
  const bool waiting = st == ClaudeState::Waiting;
  const float pulse = waiting ? 0.5f + 0.5f * sinf(now / 200.0f) : 0;
  const int r = waiting ? 9 + static_cast<int>(4 * pulse) : 8;
  const float spin = now / (waiting ? 700.0f : 260.0f);
  const int cx = 298, cy = 20;
  for (int i = 0; i < 4; i++) {
    const float a = i * PI / 4 + spin;
    const float dx = cosf(a) * r, dy = sinf(a) * r;
    top.drawWideLine(cx - dx, cy - dy, cx + dx, cy + dy, 3, kClaudeOrange, TFT_BLACK);
  }
  if (!waiting) {
    // Three "thinking" dots.
    for (int i = 0; i < 3; i++) {
      const bool lit = ((now / 300) % 3) == static_cast<uint32_t>(i);
      top.fillSmoothCircle(270 + i * 8, cy, 2, lit ? kClaudeOrange : COL_DIM, TFT_BLACK);
    }
  }
}

static void renderTop(uint32_t now) {
  if (!g_topOk) return;
  const View view = brain.topView(now);
  if (view != g_lastView) {
    g_lastView = view;
    g_viewSwitchAt = now;
  }
  const bool musicView = view == View::Music || view == View::Focus;

  // Layout: slide the face down to centre when the panel has nothing to show.
  const bool wantPanel = panel.wantsVisible(now, musicView);
  const float dt = g_layoutAt ? static_cast<float>(now - g_layoutAt) : 0.0f;
  g_layoutAt = now;
  const float step = dt / kLayoutMs;
  g_full = wantPanel ? max(0.0f, g_full - step) : min(1.0f, g_full + step);
  panel.setHidden(!wantPanel || g_full > 0.0f);
  const float e = g_full * g_full * (3 - 2 * g_full);
  const int topY = static_cast<int>(kFullTopY * e + 0.5f);
  if (topY != g_topY || (g_full > 0.0f && g_full < 1.0f)) {
    if (topY > 0) tft.fillRect(0, 0, SCREEN_W, topY, TFT_BLACK);
    if (topY + EYES_H < SCREEN_H) tft.fillRect(0, topY + EYES_H, SCREEN_W, SCREEN_H - topY - EYES_H, TFT_BLACK);
    g_topY = topY;
  }

  top.fillSprite(TFT_BLACK);
  if (view == View::Music) {
    music.drawTop(top, now);
  } else if (view == View::Clock) {
    drawClockView(top, brain, now);
  } else if (view == View::Focus) {
    drawFocusView(top, brain, now);
  } else {
    eyes.draw(top, EYES_W / 2, static_cast<int>(84 - 6 * e), 1.0f + 0.08f * e);
    drawClaudeBadge(now);
  }

  // Curtain reveal when switching between face and vinyl.
  const uint32_t t = now - g_viewSwitchAt;
  if (g_viewSwitchAt && t < 260) {
    const int cover = static_cast<int>(EYES_H * (1.0f - t / 260.0f));
    top.fillRect(0, EYES_H - cover, EYES_W, cover, TFT_BLACK);
  }
  top.pushSprite(0, g_topY);
}

// ---------------------------------------------------------------- http helpers

static bool parseBody(JsonDocument &doc) {
  if (!server.hasArg("plain")) {
    server.send(400, "application/json", "{\"ok\":false,\"err\":\"no body\"}");
    return false;
  }
  const DeserializationError err = deserializeJson(doc, server.arg("plain"));
  if (err) {
    server.send(400, "application/json", String("{\"ok\":false,\"err\":\"") + err.c_str() + "\"}");
    return false;
  }
  return true;
}

static Mood moodOr(JsonVariantConst v, Mood fallback) {
  Mood m = fallback;
  if (v.is<const char *>()) moodFromString(String(v.as<const char *>()), m);
  return m;
}

static void sendOk() { server.send(200, "application/json", "{\"ok\":true}"); }
static void sendErr(const char *e) { server.send(400, "application/json", String("{\"ok\":false,\"err\":\"") + e + "\"}"); }

// ---------------------------------------------------------------- routes

static void handleState() {
  JsonDocument doc;
  if (!parseBody(doc)) return;

  StateMsg msg;
  JsonVariantConst act = doc["activity"];
  if (!act.isNull()) {
    msg.hasActivity = true;
    msg.label = act["label"] | "";
    msg.category = act["category"] | "other";
    msg.minutes = act["minutes"] | 0;
  }
  JsonVariantConst mus = doc["music"];
  if (!mus.isNull()) {
    msg.hasMusic = true;
    msg.playing = mus["playing"] | false;
    msg.track = mus["track"] | "";
    msg.artist = mus["artist"] | "";
    msg.posMs = mus["position_ms"] | 0;
    msg.durMs = mus["duration_ms"] | 0;
    uint16_t c;
    if (Settings::fromHex(mus["color"] | "", c)) msg.color = c;
  }
  JsonVariantConst lyr = doc["lyrics"];
  if (!lyr.isNull()) {
    msg.hasLyrics = true;
    msg.lyricCur = lyr["cur"] | "";
    msg.lyricNext = lyr["next"] | "";
    msg.lyricLineMs = lyr["line_ms"] | 0;
  }
  JsonVariantConst task = doc["task"];
  if (!task.isNull()) {
    msg.hasTask = true;
    msg.taskTitle = task["title"] | "";
    msg.taskEst = task["est_min"] | 0;
    msg.tasksLeft = task["left"] | 0;
  }
  if (doc["mood"].is<const char *>()) msg.hasMood = moodFromString(String(doc["mood"].as<const char *>()), msg.mood);
  JsonVariantConst say = doc["say"];
  if (say["text"].is<const char *>()) {
    msg.hasSay = true;
    msg.sayText = say["text"].as<const char *>();
    msg.sayMood = moodOr(say["mood"], Mood::Excited);
    msg.sayMs = say["ms"] | 6000;
  }
  brain.apply(msg);
  server.send(200, "application/json", "{\"ok\":true,\"events\":" + brain.takeEventsJson() + "}");
}

static void handleView() {
  JsonDocument doc;
  if (!parseBody(doc)) return;
  const String v = doc["view"] | "next";
  const uint32_t now = millis();
  if (v == "next") brain.cycleView(now);
  else if (!brain.setView(v, now)) return sendErr("bad view");
  server.send(200, "application/json", String("{\"ok\":true,\"view\":\"") + viewName(brain.topView(now)) + "\"}");
}

static void handleTimer() {
  JsonDocument doc;
  if (!parseBody(doc)) return;
  const String action = doc["action"] | "";
  const uint32_t secs = doc["seconds"] | 0;
  if (!brain.timer(action, secs)) return sendErr("bad timer action");
  if (action != "stop") brain.setView("focus", millis());
  sendOk();
}

static void handleSay() {
  JsonDocument doc;
  if (!parseBody(doc)) return;
  const char *text = doc["text"] | "";
  if (!*text) return sendErr("text required");
  brain.say(text, moodOr(doc["mood"], Mood::Excited), doc["ms"] | 6000);
  sendOk();
}

static void handleMood() {
  JsonDocument doc;
  if (!parseBody(doc)) return;
  Mood m;
  if (!doc["mood"].is<const char *>() || !moodFromString(String(doc["mood"].as<const char *>()), m)) {
    return sendErr("unknown mood");
  }
  brain.setTempMood(m, constrain(static_cast<uint32_t>(doc["ms"] | 5000), 500UL, 60000UL));
  sendOk();
}

static void handleAnim() {
  JsonDocument doc;
  if (!parseBody(doc)) return;
  const String n = doc["name"] | "";
  IdleAnim a = IdleAnim::None;
  if (n == "lookaround") a = IdleAnim::LookAround;
  else if (n == "curious") a = IdleAnim::Curious;
  else if (n == "wiggle") a = IdleAnim::Wiggle;
  else if (n == "yawn") a = IdleAnim::Yawn;
  else if (n == "peek") a = IdleAnim::Peek;
  if (a == IdleAnim::None) return sendErr("unknown anim");
  eyes.play(a);
  sendOk();
}

static void handleClaude() {
  JsonDocument doc;
  if (!parseBody(doc)) return;
  brain.setClaude(doc["state"] | "none", doc["project"] | "");
  sendOk();
}

static void handlePomodoro() {
  JsonDocument doc;
  if (!parseBody(doc)) return;
  if (!brain.pomodoro(doc["action"] | "")) return sendErr("bad action");
  sendOk();
}

static void handleArt() {
  JsonDocument doc;
  if (!parseBody(doc)) return;
  const char *b64 = doc["data"] | "";
  const size_t need = ART_SIZE * ART_SIZE * 2;
  static uint8_t buf[ART_SIZE * ART_SIZE * 2 + 4];
  size_t olen = 0;
  const int rc = mbedtls_base64_decode(buf, sizeof(buf), &olen, reinterpret_cast<const unsigned char *>(b64),
                                       strlen(b64));
  if (rc != 0 || olen != need || !music.setArt(buf, olen)) return sendErr("bad art");
  sendOk();
}

static void handleSettingsGet() { server.send(200, "application/json", g_settings.toJson()); }

static void handleSettingsPost() {
  JsonDocument doc;
  if (!parseBody(doc)) return;
  const uint8_t oldRot = g_settings.rotation;
  if (g_settings.apply(doc.as<JsonVariantConst>())) {
    g_settings.save(prefs);
    if (g_settings.rotation != oldRot) applyLook();
    eyes.setBaseColor(COL_MONO);
    panel.markDirty();
  }
  server.send(200, "application/json", g_settings.toJson());
}

// Legacy v1 endpoint: {"rotation":1|3,"name":"..."}
static void handleConfig() {
  JsonDocument doc;
  if (!parseBody(doc)) return;
  JsonDocument mapped;
  if (doc["rotation"].is<int>()) mapped["flip"] = doc["rotation"].as<int>() == 3;
  if (doc["name"].is<const char *>()) mapped["name"] = doc["name"];
  const uint8_t oldRot = g_settings.rotation;
  if (g_settings.apply(mapped.as<JsonVariantConst>())) {
    g_settings.save(prefs);
    if (g_settings.rotation != oldRot) applyLook();
  }
  sendOk();
}

// ---------------------------------------------------------------- network

static void onNetworkUp() {
  g_netUp = true;
  configTime(TZ_OFFSET_S, 0, "pool.ntp.org", "time.google.com");

  MDNS.begin(PET_HOSTNAME);
  MDNS.addService("http", "tcp", 80);

  ArduinoOTA.setHostname(PET_HOSTNAME);
  ArduinoOTA.setPassword(PET_OTA_PASSWORD);
  ArduinoOTA.onStart([]() {
    g_otaActive = true;
    tft.fillScreen(TFT_BLACK);
    tft.setTextDatum(MC_DATUM);
    tft.setTextColor(COL_MONO, TFT_BLACK);
    tft.drawString("Updating...", SCREEN_W / 2, SCREEN_H / 2 - 18, 4);
    tft.drawRoundRect(40, SCREEN_H / 2 + 8, SCREEN_W - 80, 12, 6, COL_DIM);
  });
  ArduinoOTA.onProgress([](unsigned int done, unsigned int total) {
    const int w = static_cast<int>((static_cast<uint64_t>(SCREEN_W - 84) * done) / total);
    tft.fillRoundRect(42, SCREEN_H / 2 + 10, max(8, w), 8, 4, COL_MONO);
  });
  ArduinoOTA.onError([](ota_error_t e) {
    Serial.printf("[ota] error %u\n", e);
    g_otaActive = false;
    tft.fillScreen(TFT_BLACK);
    panel.markDirty();
  });
  ArduinoOTA.begin();

  server.on("/", HTTP_GET, []() { server.send_P(200, "text/html", kWebApp); });
  server.on("/api/status", HTTP_GET, []() { server.send(200, "application/json", brain.statusJson(millis())); });
  server.on("/api/settings", HTTP_GET, handleSettingsGet);
  server.on("/api/settings", HTTP_POST, handleSettingsPost);
  server.on("/api/state", HTTP_POST, handleState);
  server.on("/api/say", HTTP_POST, handleSay);
  server.on("/api/view", HTTP_POST, handleView);
  server.on("/api/timer", HTTP_POST, handleTimer);
  server.on("/api/mood", HTTP_POST, handleMood);
  server.on("/api/anim", HTTP_POST, handleAnim);
  server.on("/api/claude", HTTP_POST, handleClaude);
  server.on("/api/pomodoro", HTTP_POST, handlePomodoro);
  server.on("/api/art", HTTP_POST, handleArt);
  server.on("/api/config", HTTP_POST, handleConfig);
  server.onNotFound([]() { server.send(404, "application/json", "{\"ok\":false}"); });
  server.begin();

  Serial.printf("[net] up: http://%s/  (%s.local)\n", WiFi.localIP().toString().c_str(), PET_HOSTNAME);
}

// ---------------------------------------------------------------- arduino

void setup() {
  Serial.begin(115200);
  delay(100);

  prefs.begin("deskpet", false);
  g_settings.load(prefs);

  tft.init();
  tft.invertDisplay(true);  // this clone panel is inverted (see DEAD_ZONE_FIX_HISTORY.md)
  applyLook();

  // Allocate sprites before WiFi so the big buffers get contiguous RAM.
  top.setColorDepth(16);
  g_topOk = top.createSprite(EYES_W, EYES_H) != nullptr;
  if (!g_topOk) {
    top.setColorDepth(8);
    g_topOk = top.createSprite(EYES_W, EYES_H) != nullptr;
  }
  if (!panel.begin()) Serial.println("[panel] sprite alloc FAILED");
  if (!music.begin()) Serial.println("[music] art sprite alloc FAILED");
  Serial.printf("[mem] top=%d free heap: %u\n", g_topOk, ESP.getFreeHeap());

  eyes.openFromClosed();
  panel.peek(8000);  // greet with the info panel once, then the face takes over
  brain.begin();
  eyes.update(millis());
  renderTop(millis());
  panel.update(millis());

  WiFi.mode(WIFI_STA);
  WiFi.setHostname(PET_HOSTNAME);
  wm.setConnectTimeout(12);
  wm.setConnectRetries(2);
  wm.setConfigPortalBlocking(false);
  wm.setConfigPortalTimeout(0);
  wm.setTitle("Desk Pet");
  if (wm.autoConnect(PET_SETUP_AP)) {
    onNetworkUp();
  } else {
    Serial.println("[wifi] setup portal: join '" PET_SETUP_AP "' -> 192.168.4.1");
    brain.say("Join WiFi " PET_SETUP_AP " to set me up", Mood::Surprised, 15000);
  }
}

void loop() {
  if (g_netUp) {
    ArduinoOTA.handle();
    if (g_otaActive) return;
    server.handleClient();
  } else {
    wm.process();
    if (WiFi.status() == WL_CONNECTED) onNetworkUp();
  }

  const uint32_t now = millis();
  brain.update(now);
  eyes.update(now);
  if (now - g_lastFrame >= EYE_FRAME_MS) {
    g_lastFrame = now;
    renderTop(now);
  }
  panel.update(now);
}
