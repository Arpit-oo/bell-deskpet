//! Procedural eyes for the desktop pet. Port of the robot's `eyes.cpp`, drawn with
//! tiny-skia (anti-aliased, transparent). Coordinates are in "robot units" (the
//! face is ~210 x 100 units) and mapped to pixels with `k`.

use tiny_skia::{BlendMode, Color, FillRule, LineCap, Paint, PathBuilder, Pixmap, Rect, Stroke, Transform};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mood {
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
}

impl Mood {
    pub fn parse(s: &str) -> Mood {
        match s.to_ascii_lowercase().as_str() {
            "happy" => Mood::Happy,
            "excited" => Mood::Excited,
            "focus" => Mood::Focus,
            "music" => Mood::Music,
            "sleepy" => Mood::Sleepy,
            "sus" => Mood::Sus,
            "sad" => Mood::Sad,
            "angry" => Mood::Angry,
            "surprised" => Mood::Surprised,
            "love" => Mood::Love,
            _ => Mood::Idle,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Anim {
    None,
    LookAround,
    Curious,
    Wiggle,
    Yawn,
    Peek,
}

pub const ICE: [f32; 3] = [120.0, 230.0, 255.0];
const FOCUS: [f32; 3] = [180.0, 220.0, 255.0];
const MUSIC: [f32; 3] = [110.0, 240.0, 160.0];
pub const GOLD: [f32; 3] = [255.0, 214.0, 90.0];
const ANGRY: [f32; 3] = [255.0, 80.0, 70.0];
pub const LOVE: [f32; 3] = [255.0, 105.0, 180.0];
const SLEEPY: [f32; 3] = [90.0, 160.0, 200.0];

#[derive(Clone, Copy)]
struct Shape {
    w: f32,
    h: f32,
    r: f32,
    slant: f32,
    lid: f32,
    happy: f32,
    y_off: f32,
    brow_y: f32,
    brow_ang: f32,
}

const fn sh(w: f32, h: f32, r: f32, slant: f32, lid: f32, happy: f32, y_off: f32, brow_y: f32, brow_ang: f32) -> Shape {
    Shape { w, h, r, slant, lid, happy, y_off, brow_y, brow_ang }
}

#[derive(Clone, Copy)]
struct Target {
    l: Shape,
    r: Shape,
    color: [f32; 3],
    gap: f32,
    bounce: f32,
    hearts: f32,
    sparkles: f32,
    zzz: f32,
    brows: f32,
    saccade_rate: f32,
    blink_rate: f32,
}

fn target_for(m: Mood, base_color: [f32; 3]) -> Target {
    let base = sh(78.0, 92.0, 24.0, 0.0, 0.0, 0.0, 0.0, 16.0, 0.0);
    let mut t = Target {
        l: base,
        r: base,
        color: base_color,
        gap: 54.0,
        bounce: 0.0,
        hearts: 0.0,
        sparkles: 0.0,
        zzz: 0.0,
        brows: 1.0,
        saccade_rate: 1.0,
        blink_rate: 1.0,
    };
    match m {
        Mood::Idle => {}
        Mood::Happy => {
            t.l = sh(78.0, 92.0, 24.0, 0.0, 0.0, 1.0, 0.0, 22.0, -6.0);
            t.r = t.l;
        }
        Mood::Excited => {
            t.l = sh(86.0, 98.0, 26.0, 0.0, 0.0, 1.0, -4.0, 22.0, -8.0);
            t.r = t.l;
            t.color = GOLD;
            t.bounce = 1.0;
            t.sparkles = 1.0;
            t.saccade_rate = 1.4;
        }
        Mood::Focus => {
            t.l = sh(86.0, 56.0, 18.0, 7.0, 0.0, 0.0, 6.0, 10.0, 8.0);
            t.r = t.l;
            t.color = FOCUS;
            t.saccade_rate = 0.35;
            t.blink_rate = 0.8;
        }
        Mood::Music => {
            t.l = sh(80.0, 88.0, 24.0, 0.0, 0.0, 0.8, 0.0, 20.0, -4.0);
            t.r = t.l;
            t.color = MUSIC;
            t.bounce = 1.0;
            t.saccade_rate = 0.6;
        }
        Mood::Sleepy => {
            t.l = sh(80.0, 78.0, 22.0, -5.0, 0.52, 0.0, 8.0, 6.0, -4.0);
            t.r = t.l;
            t.color = SLEEPY;
            t.zzz = 1.0;
            t.saccade_rate = 0.2;
            t.blink_rate = 0.45;
        }
        Mood::Sus => {
            t.l = sh(80.0, 84.0, 22.0, 4.0, 0.48, 0.0, 4.0, 6.0, 12.0);
            t.r = sh(80.0, 84.0, 22.0, 4.0, 0.30, 0.0, 2.0, 22.0, -10.0);
            t.saccade_rate = 0.25;
        }
        Mood::Sad => {
            t.l = sh(76.0, 80.0, 24.0, -18.0, 0.0, 0.0, 8.0, 14.0, -20.0);
            t.r = t.l;
            t.color = SLEEPY;
            t.saccade_rate = 0.4;
        }
        Mood::Angry => {
            t.l = sh(82.0, 76.0, 18.0, 20.0, 0.0, 0.0, 4.0, 4.0, 26.0);
            t.r = t.l;
            t.color = ANGRY;
            t.saccade_rate = 0.6;
        }
        Mood::Surprised => {
            t.l = sh(90.0, 104.0, 40.0, 0.0, 0.0, 0.0, -6.0, 18.0, -4.0);
            t.r = t.l;
            t.gap = 48.0;
            t.saccade_rate = 1.6;
            t.blink_rate = 0.5;
        }
        Mood::Love => {
            t.color = LOVE;
            t.hearts = 1.0;
            t.brows = 0.0;
            t.bounce = 0.4;
            t.saccade_rate = 0.5;
        }
    }
    t
}

/// Frame-rate independent exponential smoothing.
fn approach(cur: f32, target: f32, dt: f32, tau: f32) -> f32 {
    cur + (target - cur) * (1.0 - (-dt / tau).exp())
}

fn envelope(t: f32, dur: f32) -> f32 {
    let edge = 250.0;
    let e = (t / edge).min((dur - t).max(0.0) / edge).clamp(0.0, 1.0);
    e * e * (3.0 - 2.0 * e)
}

const BEAT_MS: f32 = 60000.0 / 116.0;

pub struct Rng(u64);

impl Rng {
    pub fn new() -> Self {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x9E37_79B9_7F4A_7C15);
        Rng(seed | 1)
    }
    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    pub fn f(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * ((self.next() % 10_000) as f32 / 10_000.0)
    }
    pub fn chance(&mut self, pct: u64) -> bool {
        self.next() % 100 < pct
    }
}

#[derive(Clone, Copy)]
struct Mods {
    dy_l: f32,
    dy_r: f32,
    brow_l: f32,
    brow_r: f32,
    lid: f32,
    h_scale: f32,
    happy: f32,
    scale: f32,
}

const NO_MODS: Mods = Mods { dy_l: 0.0, dy_r: 0.0, brow_l: 0.0, brow_r: 0.0, lid: 0.0, h_scale: 1.0, happy: 0.0, scale: 1.0 };

pub struct Face {
    mood: Mood,
    base_color: [f32; 3],
    tgt: Target,
    cur_l: Shape,
    cur_r: Shape,
    gap: f32,
    bounce: f32,
    hearts: f32,
    sparkles: f32,
    zzz: f32,
    brows: f32,
    col: [f32; 3],
    mods: Mods,

    look_x: f32,
    look_y: f32,
    look_tx: f32,
    look_ty: f32,
    next_saccade: f32,
    look_hold_until: f32,
    /// External gaze (mouse cursor) overrides saccades while set.
    gaze: Option<(f32, f32)>,

    next_blink: f32,
    blink_start: f32,
    blinking: bool,
    double_blink: bool,

    anim: Anim,
    anim_start: f32,
    anim_blinked: bool,
    next_idle_anim: f32,

    sing_mode: u8,
    line_start: f32,
    line_ms: f32,
    syllables: u32,
    sing_amt: f32,
    mouth_open: f32,

    boot_open: f32,
    now: f32,
    rng: Rng,
}

impl Face {
    pub fn new() -> Self {
        let tgt = target_for(Mood::Idle, ICE);
        Face {
            mood: Mood::Idle,
            base_color: ICE,
            tgt,
            cur_l: tgt.l,
            cur_r: tgt.r,
            gap: tgt.gap,
            bounce: 0.0,
            hearts: 0.0,
            sparkles: 0.0,
            zzz: 0.0,
            brows: 1.0,
            col: tgt.color,
            mods: NO_MODS,
            look_x: 0.0,
            look_y: 0.0,
            look_tx: 0.0,
            look_ty: 0.0,
            next_saccade: 1500.0,
            look_hold_until: 0.0,
            gaze: None,
            next_blink: 2500.0,
            blink_start: 0.0,
            blinking: false,
            double_blink: false,
            anim: Anim::None,
            anim_start: 0.0,
            anim_blinked: false,
            next_idle_anim: 9000.0,
            sing_mode: 0,
            line_start: 0.0,
            line_ms: 0.0,
            syllables: 0,
            sing_amt: 0.0,
            mouth_open: 0.0,
            boot_open: 0.0,
            now: 0.0,
            rng: Rng::new(),
        }
    }

    pub fn set_mood(&mut self, m: Mood) {
        if m == self.mood {
            return;
        }
        self.mood = m;
        self.tgt = target_for(m, self.base_color);
        if !self.blinking && m != Mood::Sleepy {
            self.blink();
        }
    }

    pub fn blink(&mut self) {
        self.blinking = true;
        self.blink_start = self.now;
        self.double_blink = self.rng.chance(15);
    }

    pub fn set_gaze(&mut self, g: Option<(f32, f32)>) {
        self.gaze = g.map(|(x, y)| (x.clamp(-1.0, 1.0), y.clamp(-1.0, 1.0)));
    }

    fn look_at(&mut self, x: f32, y: f32, hold: f32) {
        self.look_tx = x.clamp(-1.0, 1.0);
        self.look_ty = y.clamp(-1.0, 1.0);
        self.look_hold_until = self.now + hold;
    }

    pub fn play(&mut self, a: Anim) {
        self.anim = a;
        self.anim_start = self.now;
        self.anim_blinked = false;
    }

    /// mode 0 off, 1 hum, 2 sing (lip-sync to the current lyric line).
    pub fn set_sing(&mut self, mode: u8, line_start: f32, syllables: u32, line_ms: f32) {
        self.sing_mode = mode;
        self.line_start = line_start;
        self.syllables = syllables;
        self.line_ms = line_ms;
    }

    fn mouth_target(&self) -> f32 {
        let now = self.now;
        if self.sing_mode == 1 {
            return 0.14 + 0.06 * (now / 320.0).sin();
        }
        if self.sing_mode != 2 || self.syllables == 0 {
            return 0.05;
        }
        let t = now - self.line_start;
        let active = if self.line_ms > 0.0 { self.line_ms * 0.85 } else { self.syllables as f32 * 230.0 };
        if t < 0.0 || t >= active {
            return 0.05;
        }
        let ph = (t / active) * self.syllables as f32;
        let syl = ph as u32;
        let loud = 0.55 + 0.45 * ((syl.wrapping_mul(2_654_435_761) >> 24) & 0xFF) as f32 / 255.0;
        (std::f32::consts::PI * ph).sin().abs().powf(0.7) * loud
    }

    fn blink_factor(&mut self) -> f32 {
        if !self.blinking {
            return 1.0;
        }
        let t = self.now - self.blink_start;
        if t < 0.0 {
            return 1.0;
        }
        if t < 70.0 {
            return 1.0 - 0.95 * (t / 70.0);
        }
        if t < 100.0 {
            return 0.05;
        }
        if t < 210.0 {
            return 0.05 + 0.95 * ((t - 100.0) / 110.0);
        }
        if self.double_blink {
            self.double_blink = false;
            self.blink_start = self.now + 60.0;
        } else {
            self.blinking = false;
        }
        1.0
    }

    fn update_anim(&mut self) {
        self.mods = NO_MODS;
        if self.anim == Anim::None {
            return;
        }
        let dur = match self.anim {
            Anim::LookAround => 2600.0,
            Anim::Curious => 2000.0,
            Anim::Wiggle => 1000.0,
            Anim::Yawn => 2400.0,
            Anim::Peek => 1600.0,
            Anim::None => 0.0,
        };
        let t = self.now - self.anim_start;
        if t >= dur {
            self.anim = Anim::None;
            return;
        }
        let e = envelope(t, dur);
        match self.anim {
            Anim::LookAround => {
                if t < 700.0 {
                    self.look_at(-0.95, -0.15, 400.0)
                } else if t < 1400.0 {
                    self.look_at(0.95, -0.15, 400.0)
                } else if t < 2000.0 {
                    self.look_at(0.0, 0.5, 400.0)
                } else {
                    self.look_at(0.0, 0.0, 400.0)
                }
            }
            Anim::Curious => {
                self.mods.dy_l = -8.0 * e;
                self.mods.dy_r = 8.0 * e;
                self.mods.brow_r = 10.0 * e;
                self.mods.brow_l = -3.0 * e;
                self.look_at(0.35, -0.2, 300.0);
            }
            Anim::Wiggle => self.look_at(0.55 * (t / 55.0).sin() * e, 0.0, 200.0),
            Anim::Yawn => {
                if t < 1000.0 {
                    self.mods.lid = 0.55 * (t / 700.0).min(1.0);
                } else if t < 1750.0 {
                    self.mods.h_scale = 1.0 + 0.18 * e;
                    self.mods.brow_l = 9.0 * e;
                    self.mods.brow_r = 9.0 * e;
                } else if !self.anim_blinked {
                    self.anim_blinked = true;
                    self.blink();
                }
            }
            Anim::Peek => {
                self.mods.scale = 1.0 + 0.08 * e;
                self.mods.happy = 0.6 * e;
                self.look_at(0.0, 0.0, 300.0);
            }
            Anim::None => {}
        }
    }

    pub fn update(&mut self, now: f32) {
        let dt = if self.now == 0.0 { 16.0 } else { (now - self.now).clamp(0.0, 100.0) };
        self.now = now;
        if dt <= 0.0 {
            return;
        }
        let ap = |c: &mut Shape, t: &Shape| {
            c.w = approach(c.w, t.w, dt, 90.0);
            c.h = approach(c.h, t.h, dt, 90.0);
            c.r = approach(c.r, t.r, dt, 90.0);
            c.slant = approach(c.slant, t.slant, dt, 110.0);
            c.lid = approach(c.lid, t.lid, dt, 140.0);
            c.happy = approach(c.happy, t.happy, dt, 120.0);
            c.y_off = approach(c.y_off, t.y_off, dt, 120.0);
            c.brow_y = approach(c.brow_y, t.brow_y, dt, 110.0);
            c.brow_ang = approach(c.brow_ang, t.brow_ang, dt, 110.0);
        };
        let (tl, tr) = (self.tgt.l, self.tgt.r);
        ap(&mut self.cur_l, &tl);
        ap(&mut self.cur_r, &tr);
        self.gap = approach(self.gap, self.tgt.gap, dt, 100.0);
        self.bounce = approach(self.bounce, self.tgt.bounce, dt, 200.0);
        self.hearts = approach(self.hearts, self.tgt.hearts, dt, 120.0);
        self.sparkles = approach(self.sparkles, self.tgt.sparkles, dt, 200.0);
        self.zzz = approach(self.zzz, self.tgt.zzz, dt, 300.0);
        self.brows = approach(self.brows, self.tgt.brows, dt, 120.0);
        for i in 0..3 {
            self.col[i] = approach(self.col[i], self.tgt.color[i], dt, 150.0);
        }
        self.boot_open = approach(self.boot_open, 1.0, dt, 260.0);
        self.sing_amt = approach(self.sing_amt, if self.sing_mode > 0 { 1.0 } else { 0.0 }, dt, 220.0);
        self.mouth_open = approach(self.mouth_open, self.mouth_target(), dt, 40.0);

        self.update_anim();

        if let Some((gx, gy)) = self.gaze {
            if self.anim == Anim::None {
                self.look_tx = gx;
                self.look_ty = gy;
            }
        } else if now > self.look_hold_until && now > self.next_saccade {
            if self.rng.chance(30) {
                self.look_tx = 0.0;
                self.look_ty = 0.0;
            } else {
                self.look_tx = self.rng.f(-0.9, 0.9);
                self.look_ty = self.rng.f(-0.6, 0.6);
            }
            self.next_saccade = now + self.rng.f(1200.0, 4200.0) / self.tgt.saccade_rate;
        }
        self.look_x = approach(self.look_x, self.look_tx, dt, 45.0);
        self.look_y = approach(self.look_y, self.look_ty, dt, 45.0);

        if !self.blinking && now > self.next_blink {
            self.blink();
            self.next_blink = now + self.rng.f(2200.0, 6000.0) / self.tgt.blink_rate;
        }

        // Little personality while idle.
        if now > self.next_idle_anim {
            self.next_idle_anim = now + self.rng.f(14_000.0, 32_000.0);
            if self.anim == Anim::None && self.gaze.is_none() && self.sing_mode == 0 {
                let pick = match self.mood {
                    Mood::Sleepy => Anim::Yawn,
                    Mood::Focus | Mood::Angry => Anim::None,
                    _ => match self.rng.next() % 4 {
                        0 => Anim::LookAround,
                        1 => Anim::Curious,
                        2 => Anim::Peek,
                        _ => Anim::Wiggle,
                    },
                };
                if pick != Anim::None {
                    self.play(pick);
                }
            }
        }
    }

    pub fn color(&self) -> [f32; 3] {
        self.col
    }

    /// Draws the face centred on (cx, cy) px. Eye shapes go into `layer` (cut-outs
    /// use Clear so the layer stays transparent); floating effects (notes,
    /// sparkles) go into `fx`, which is not clipped to the pet's screen.
    pub fn draw(&mut self, layer: &mut Pixmap, fx: &mut Pixmap, cx: f32, cy: f32, k: f32) {
        let now = self.now;
        let k = k * self.mods.scale;
        let b = (self.blink_factor() * self.boot_open).max(0.04);
        let phase = (now % BEAT_MS) / BEAT_MS;
        let beat = (phase * std::f32::consts::PI).sin().abs() * self.bounce;
        let breathe = (now / 1500.0).sin() * 2.0;

        let cy = cy + (breathe - 7.0 * beat) * k;
        let base_cx = cx;
        let cx = cx + 7.0 * (2.0 * std::f32::consts::PI * now / (2.0 * BEAT_MS)).sin() * self.sing_amt * k;
        let lpx = self.look_x * 34.0 * k;
        let lpy = self.look_y * 14.0 * k;
        let l_scale = k * (1.0 - 0.07 * self.look_x);
        let r_scale = k * (1.0 + 0.07 * self.look_x);
        let col = rgba(self.col, 255);

        let l_cx = cx - (self.gap / 2.0 + self.cur_l.w / 2.0) * k + lpx;
        let r_cx = cx + (self.gap / 2.0 + self.cur_r.w / 2.0) * k + lpx;
        let eye_y = cy + lpy;

        if self.hearts > 0.5 {
            let pulse = 1.0 + 0.07 * (now / 170.0).sin();
            let sz = 80.0 * k * pulse * ((self.hearts - 0.5) * 2.0) * b;
            heart(layer, l_cx, eye_y, sz, col);
            heart(layer, r_cx, eye_y, sz, col);
        } else {
            let (mut l, mut r) = (self.cur_l, self.cur_r);
            let squash = (1.0 - 0.06 * beat) * self.mods.h_scale;
            l.h *= b * squash;
            r.h *= b * squash;
            l.w *= 1.0 + 0.04 * beat;
            r.w *= 1.0 + 0.04 * beat;
            l.lid = (l.lid + self.mods.lid).min(0.95);
            r.lid = (r.lid + self.mods.lid).min(0.95);
            l.happy = (l.happy + self.mods.happy).min(1.0);
            r.happy = (r.happy + self.mods.happy).min(1.0);
            l.y_off += self.mods.dy_l;
            r.y_off += self.mods.dy_r;

            eye(layer, l_cx, eye_y, &l, l_scale, true, col);
            eye(layer, r_cx, eye_y, &r, r_scale, false, col);

            if self.brows > 0.15 {
                let l_top = eye_y - self.cur_l.h * l_scale * self.mods.h_scale / 2.0 + l.y_off * l_scale;
                let r_top = eye_y - self.cur_r.h * r_scale * self.mods.h_scale / 2.0 + r.y_off * r_scale;
                self.brow(layer, l_cx, l_top, &l, l_scale, true, self.mods.brow_l, col);
                self.brow(layer, r_cx, r_top, &r, r_scale, false, self.mods.brow_r, col);
            }
        }

        if self.sing_amt > 0.05 && self.hearts <= 0.5 {
            self.mouth(layer, cx, eye_y + 50.0 * k, k, col);
            self.notes(fx, base_cx, cy, k, col);
        }
        if self.sparkles > 0.1 {
            self.sparkle(fx, cx, cy, k);
        }
    }

    /// 0..1 how sleepy the face looks (overlay draws the floating z's).
    pub fn zzz(&self) -> f32 {
        self.zzz
    }

    #[allow(clippy::too_many_arguments)]
    fn brow(&self, s: &mut Pixmap, cx: f32, eye_top: f32, e: &Shape, k: f32, is_left: bool, lift: f32, col: Color) {
        let half_w = e.w * k * 0.36;
        let by = eye_top - (e.brow_y + lift) * k;
        let dy = e.brow_ang.to_radians().tan() * half_w;
        let th = (7.0 * k * self.brows).max(1.5);
        let (outer, inner) = if is_left { (cx - half_w, cx + half_w) } else { (cx + half_w, cx - half_w) };
        line(s, outer, by - dy, inner, by + dy, th, col);
    }

    fn mouth(&self, s: &mut Pixmap, cx: f32, y: f32, k: f32, col: Color) {
        let a = self.sing_amt;
        let o = self.mouth_open.clamp(0.0, 1.0);
        let w = (20.0 + 12.0 * o) * k * a;
        let h = (5.0 + 24.0 * o) * k * a;
        if w < 3.0 || h < 2.0 {
            return;
        }
        rrect(s, cx - w / 2.0, y - h / 2.0, w, h, w.min(h) / 2.0, col, BlendMode::SourceOver);
        if h > 9.0 * k.max(0.5) {
            let t = (3.0 * k).max(2.0);
            let (iw, ih) = (w - 2.0 * t, h - 2.0 * t);
            rrect(s, cx - iw / 2.0, y - ih / 2.0, iw, ih, iw.min(ih) / 2.0, Color::TRANSPARENT, BlendMode::Clear);
            if o > 0.5 {
                oval(s, cx, y + ih / 2.0 - 3.0 * k, iw / 3.0, (ih / 5.0).max(2.0), rgba(LOVE, 255), BlendMode::SourceOver);
            }
        }
    }

    fn notes(&self, s: &mut Pixmap, cx: f32, cy: f32, k: f32, col: Color) {
        let speed = if self.sing_mode == 2 { 1900.0 } else { 2800.0 };
        for i in 0..3 {
            let ph = (self.now / speed + i as f32 / 3.0) % 1.0;
            if ph > 0.85 {
                continue;
            }
            let fade = ((0.85 - ph) / 0.25).min(1.0) * self.sing_amt;
            let mut c = col;
            c.apply_opacity(fade);
            let side = if i % 2 == 1 { 1.0 } else { -1.0 };
            let x = cx + side * (128.0 + 10.0 * (ph * 9.0 + i as f32).sin()) * k;
            let y = cy + (30.0 - ph * 110.0) * k;
            let r = (5.0 * k).max(2.0);
            oval(s, x, y, r * 1.15, r, c, BlendMode::SourceOver);
            let th = (2.4 * k).max(1.2);
            line(s, x + r, y, x + r, y - 4.2 * r, th, c);
            line(s, x + r, y - 4.2 * r, x + 3.2 * r, y - 3.2 * r, th, c);
        }
    }

    fn sparkle(&self, s: &mut Pixmap, cx: f32, cy: f32, k: f32) {
        const POS: [(f32, f32); 4] = [(-132.0, -45.0), (136.0, -35.0), (-118.0, 45.0), (124.0, 40.0)];
        for (i, (px, py)) in POS.iter().enumerate() {
            let tw = (self.now / 210.0 + i as f32 * 1.7).sin().abs();
            let sz = (2.0 + 6.0 * tw) * self.sparkles * k;
            if sz < 1.0 {
                continue;
            }
            let x = cx + px * k;
            let y = cy + py * k;
            let c = rgba(GOLD, (255.0 * self.sparkles) as u8);
            let th = (2.0 * k).max(1.0);
            line(s, x - sz, y, x + sz, y, th, c);
            line(s, x, y - sz, x, y + sz, th, c);
        }
    }
}

// ---------------------------------------------------------------- drawing helpers

pub fn rgba(c: [f32; 3], a: u8) -> Color {
    Color::from_rgba8(c[0].clamp(0.0, 255.0) as u8, c[1].clamp(0.0, 255.0) as u8, c[2].clamp(0.0, 255.0) as u8, a)
}

fn paint(c: Color, mode: BlendMode) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color(c);
    p.anti_alias = true;
    p.blend_mode = mode;
    p
}

pub fn rrect_path(x: f32, y: f32, w: f32, h: f32, r: f32) -> Option<tiny_skia::Path> {
    if w <= 0.0 || h <= 0.0 {
        return None;
    }
    let r = r.min(w / 2.0).min(h / 2.0).max(0.0);
    let c = r * 0.552_284_8;
    let mut pb = PathBuilder::new();
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.cubic_to(x + w - r + c, y, x + w, y + r - c, x + w, y + r);
    pb.line_to(x + w, y + h - r);
    pb.cubic_to(x + w, y + h - r + c, x + w - r + c, y + h, x + w - r, y + h);
    pb.line_to(x + r, y + h);
    pb.cubic_to(x + r - c, y + h, x, y + h - r + c, x, y + h - r);
    pb.line_to(x, y + r);
    pb.cubic_to(x, y + r - c, x + r - c, y, x + r, y);
    pb.close();
    pb.finish()
}

#[allow(clippy::too_many_arguments)]
pub fn rrect(s: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, r: f32, c: Color, mode: BlendMode) {
    if let Some(p) = rrect_path(x, y, w, h, r) {
        s.fill_path(&p, &paint(c, mode), FillRule::Winding, Transform::identity(), None);
    }
}

pub fn oval(s: &mut Pixmap, cx: f32, cy: f32, rx: f32, ry: f32, c: Color, mode: BlendMode) {
    if let Some(r) = Rect::from_xywh(cx - rx, cy - ry, 2.0 * rx, 2.0 * ry) {
        if let Some(p) = PathBuilder::from_oval(r) {
            s.fill_path(&p, &paint(c, mode), FillRule::Winding, Transform::identity(), None);
        }
    }
}

pub fn line(s: &mut Pixmap, x0: f32, y0: f32, x1: f32, y1: f32, w: f32, c: Color) {
    let mut pb = PathBuilder::new();
    pb.move_to(x0, y0);
    pb.line_to(x1, y1);
    if let Some(p) = pb.finish() {
        let st = Stroke { width: w, line_cap: LineCap::Round, ..Default::default() };
        s.stroke_path(&p, &paint(c, BlendMode::SourceOver), &st, Transform::identity(), None);
    }
}

fn poly(s: &mut Pixmap, pts: &[(f32, f32)], c: Color, mode: BlendMode) {
    let mut pb = PathBuilder::new();
    pb.move_to(pts[0].0, pts[0].1);
    for p in &pts[1..] {
        pb.line_to(p.0, p.1);
    }
    pb.close();
    if let Some(p) = pb.finish() {
        s.fill_path(&p, &paint(c, mode), FillRule::Winding, Transform::identity(), None);
    }
}

fn eye(s: &mut Pixmap, cx: f32, cy: f32, e: &Shape, k: f32, is_left: bool, col: Color) {
    let w = e.w * k;
    let h = e.h * k;
    if w < 2.0 || h < 1.0 {
        return;
    }
    let x = cx - w / 2.0;
    let y = cy - h / 2.0 + e.y_off * k;
    let h = h.max(2.0);
    let r = (e.r * k).min(w / 2.0).min(h / 2.0);
    rrect(s, x, y, w, h, r, col, BlendMode::SourceOver);

    if h > 28.0 * k {
        let gw = (w * 0.17).max(3.0);
        let gh = (h * 0.15).max(3.0);
        rrect(s, x + w * 0.20, y + h * 0.16, gw, gh, gw.min(gh) / 2.0, Color::from_rgba8(255, 255, 255, 235), BlendMode::SourceOver);
    }

    let clear = Color::TRANSPARENT;
    let pad = 2.0 * k.max(0.5);
    let mut top = y;
    if e.lid > 0.02 {
        let lh = h * e.lid.min(0.95);
        if let Some(rc) = Rect::from_xywh(x - pad, y - pad, w + 2.0 * pad, lh + pad) {
            s.fill_rect(rc, &paint(clear, BlendMode::Clear), Transform::identity(), None);
        }
        top = y + lh;
    }

    let sl = e.slant * k;
    let (xl, xr) = (x - pad, x + w + pad);
    if sl > 1.0 {
        let tri = if is_left { [(xl, top - pad), (xr, top - pad), (xr, top + sl)] } else { [(xl, top - pad), (xr, top - pad), (xl, top + sl)] };
        poly(s, &tri, clear, BlendMode::Clear);
    } else if sl < -1.0 {
        let d = -sl;
        let tri = if is_left { [(xl, top - pad), (xr, top - pad), (xl, top + d)] } else { [(xl, top - pad), (xr, top - pad), (xr, top + d)] };
        poly(s, &tri, clear, BlendMode::Clear);
    }

    if e.happy > 0.02 {
        oval(s, cx, y + h + h * 0.15, w * 0.72, h * 0.58 * e.happy.min(1.0), clear, BlendMode::Clear);
    }
}

fn heart(s: &mut Pixmap, cx: f32, cy: f32, size: f32, col: Color) {
    let r = size / 4.2;
    if r < 1.5 {
        return;
    }
    oval(s, cx - r * 0.95, cy - r * 0.5, r, r, col, BlendMode::SourceOver);
    oval(s, cx + r * 0.95, cy - r * 0.5, r, r, col, BlendMode::SourceOver);
    poly(s, &[(cx - r * 1.93, cy - r * 0.15), (cx + r * 1.93, cy - r * 0.15), (cx, cy + r * 2.2)], col, BlendMode::SourceOver);
    let g = (r * 0.28).max(1.5);
    oval(s, cx - r * 1.2, cy - r * 0.85, g, g, Color::from_rgba8(255, 255, 255, 235), BlendMode::SourceOver);
}
