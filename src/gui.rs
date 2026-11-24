//! Drawing the machine screens: textures by the Java screens' blit coordinates, vanilla icons, Placebo's
//! `TickableTextList` (text that types itself out) and the client-side state the screen hooks share.

use crate::util::*;
use mod_api::*;
use std::sync::Mutex;

/// `gfx.blit(BASE, x, y, u, v, w, h, 256, 256)` relative to the screen's corner `o`.
pub(crate) fn blit(draw: &mut dyn DrawContext, texture: &str, o: [f32; 2], x: f32, y: f32, u: f32, v: f32, w: f32, h: f32) {
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    draw.sprite(texture, [u, v, w, h], [o[0] + x, o[1] + y, w, h], None);
}

/// A vanilla 16x16 texture (`textures/item/redstone.png`, a redstone torch) by its atlas name.
pub(crate) fn icon(draw: &mut dyn DrawContext, texture: &str, at: [f32; 2], tint: Option<u32>) {
    let name = texture.trim_start_matches("minecraft:").trim_start_matches("textures/").trim_end_matches(".png");
    draw.sprite(name, [0.0, 0.0, 16.0, 16.0], [at[0], at[1], 16.0, 16.0], tint);
}

pub(crate) const BLUE: u32 = 0xFF62_D8FF; // -10299137, the screens' cyan
pub(crate) const MINT: u32 = 0xFF00_FFC0; // 65472
pub(crate) const RED_TEXT: u32 = 0xFFE0_5050; // 14700624
pub(crate) const ORANGE_TEXT: u32 = 0xFFE0_8080; // 14712960
pub(crate) const GRAY_TEXT: u32 = 0xFF80_8080; // 8421504

/// One piece of a line: its text and colour.
pub(crate) type Segment = (String, u32);

/// Placebo's TickableTextList: lines that type themselves out, each at its own speed (characters a tick).
#[derive(Clone, Default)]
pub(crate) struct Typewriter {
    lines: Vec<(Vec<Segment>, f32)>,
    ticks: f32,
    pub max_width: f32,
}

impl Typewriter {
    pub fn new(max_width: f32) -> Self {
        Self { max_width, ..Self::default() }
    }
    pub fn clear(&mut self) {
        self.lines.clear();
        self.ticks = 0.0;
    }
    pub fn add_line(&mut self, text: String, colour: u32, speed: f32) {
        self.lines.push((vec![(text, colour)], speed));
    }
    pub fn continue_line(&mut self, text: String, colour: u32) {
        if let Some((segs, _)) = self.lines.last_mut() {
            segs.push((text, colour));
        }
    }
    pub fn set_line(&mut self, i: usize, text: String, colour: u32, speed: f32) {
        if i < self.lines.len() {
            self.lines[i] = (vec![(text, colour)], speed);
        }
    }
    pub fn set_ticks(&mut self, ticks: f32) {
        self.ticks = ticks;
    }
    pub fn tick(&mut self) {
        self.ticks += 1.0;
    }
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }
    /// The widest line, as drawn.
    pub fn width(&self, draw: &dyn DrawContext) -> f32 {
        self.lines.iter().map(|(segs, _)| segs.iter().map(|(t, _)| draw.text_width(t)).sum::<f32>()).fold(0.0, f32::max).min(self.max_width)
    }
    /// Draws what has typed out so far at `at`, wrapping each line at `max_width`; the lines drawn.
    pub fn render(&self, draw: &mut dyn DrawContext, at: [f32; 2]) -> usize {
        let mut budget = self.ticks;
        let mut y = at[1];
        let mut drawn = 0;
        for (segs, speed) in &self.lines {
            let total: usize = segs.iter().map(|(t, _)| t.chars().count()).sum();
            let shown = if *speed <= 0.0 { total } else { ((budget * speed) as usize).min(total) };
            budget = (budget - total as f32 / speed.max(0.001)).max(0.0);
            // Wrap the whole line, then show its first `shown` characters.
            let mut rows: Vec<Vec<Segment>> = vec![Vec::new()];
            let mut row_w = 0.0;
            for (text, colour) in segs {
                for word in text.split_inclusive(' ') {
                    let w = draw.text_width(word);
                    if row_w + w > self.max_width && row_w > 0.0 {
                        rows.push(Vec::new());
                        row_w = 0.0;
                    }
                    rows.last_mut().unwrap().push((word.to_owned(), *colour));
                    row_w += w;
                }
            }
            let mut left = shown;
            for row in rows {
                let mut x = at[0];
                for (word, colour) in row {
                    if left == 0 {
                        break;
                    }
                    let part: String = word.chars().take(left).collect();
                    left -= part.chars().count();
                    draw.text(&part, [x, y], colour, true);
                    x += draw.text_width(&part);
                }
                y += 12.0;
                drawn += 1;
            }
            if shown < total {
                break;
            }
        }
        drawn
    }
}

/// The open machine screen as the client last drew it: its block, menu data, corner and the pointer.
#[derive(Default)]
pub(crate) struct Shown {
    pub block: String,
    pub data: Vec<i32>,
    pub origin: [f32; 2],
    pub mouse: [f32; 2],
    /// The game time the screen last ticked its text at.
    pub last_tick: i64,
}

pub(crate) static SHOWN: Mutex<Option<Shown>> = Mutex::new(None);

pub(crate) fn with_shown<T>(f: impl FnOnce(&mut Shown) -> T) -> T {
    let mut g = SHOWN.lock().unwrap_or_else(|e| e.into_inner());
    f(g.get_or_insert_with(Shown::default))
}

/// The open menu's slot `slot` on this client.
pub(crate) fn menu_slot(host: &dyn ClientHost, slot: usize) -> Option<ItemStack> {
    let menu = host.local_player()?.container_menu(host)?;
    menu.slot(host, slot).flatten()
}

/// Presses menu button `id` and plays the click (`handleInventoryButtonClick` and `UI_BUTTON_CLICK`), for input the
/// screen_click hook doesn't see.
pub(crate) fn press(host: &mut dyn ClientHost, id: i32) {
    if let Some(menu) = host.local_player().and_then(|p| p.container_menu(&*host)) {
        menu.click_button(host, id);
    }
}

/// A line of a tooltip with a § colour code nearest `argb`.
pub(crate) fn tip(text: impl AsRef<str>, argb: u32) -> String {
    format!("{}{}", code_for(argb & 0xFF_FFFF), text.as_ref())
}

/// Draws `lines` as a vanilla tooltip whose box ends left of `right`, its top at `top` (Placebo's DrawsOnLeft).
pub(crate) fn tooltip_on_left(draw: &mut dyn DrawContext, lines: &[String], top: f32, right: f32) {
    // DrawsOnLeft: the lines wrap to the room left of the screen, the box's right edge at `right`.
    let room = (right - 12.0).max(60.0);
    let mut wrapped: Vec<String> = Vec::new();
    for l in lines {
        let code: String = if l.starts_with('§') { l.chars().take(2).collect() } else { String::new() };
        for part in crate::util::wrap(&strip_codes(l), room, |t| draw.text_width(t)) {
            wrapped.push(format!("{code}{part}"));
        }
    }
    let width = wrapped.iter().map(|l| draw.text_width(&strip_codes(l))).fold(0.0, f32::max);
    let texts: Vec<Text> = wrapped.iter().map(|l| legacy_text(l)).collect();
    // The vanilla tooltip draws its box 12 right of and 12 above the point it's given, 4 pixels of padding a side.
    let x = (right - width - 4.0 - 12.0).max(-8.0);
    let _ = draw.gui_tooltip(&texts, [x, top + 12.0]);
}

pub(crate) fn strip_codes(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '§' {
            chars.next();
        } else {
            out.push(c);
        }
    }
    out
}
