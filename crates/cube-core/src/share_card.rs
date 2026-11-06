//! Result cards: a single or an average drawn as an image to share.
//!
//! A [`ShareCard`] holds what the card says and [`ShareCard::svg`] lays it out,
//! so the layout can be tested without a window. [`render_png`] turns the SVG
//! into an image with the rasterizer GPUI already brings along. Everything
//! happens on this machine: nothing is uploaded.
//!
//! The card's own colors come from the four in [`CardColors`], and the attempts
//! of an average are the stickers of a cube face. The scramble preview keeps
//! the colors it has everywhere else in the app.
//!
//! The rasterizer always lays out a line left to right and ignores SVG's
//! `direction`, so right-to-left cards mirror their own coordinates and mark
//! the text's direction with Unicode controls.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use anyhow::{Context as _, Result, anyhow};
use resvg::{tiny_skia, usvg};

use crate::event::WcaEvent;
use crate::persistence::write_atomic;
use crate::scramble::{self, Preview};
use crate::solve::Solve;
use crate::stats::{Average, AverageDetail};
use crate::theme::ThemeColor;

/// The longest average a card can list, attempt by attempt.
pub const MAX_ATTEMPTS: usize = 12;

/// The card's size in the SVG's units, which suits most social previews.
pub const WIDTH: u32 = 1200;
pub const HEIGHT: u32 = 675;

/// How many pixels the PNG has per unit, so it stays sharp when enlarged.
const PNG_SCALE: u32 = 2;

/// The bundled typeface, in the weights the card and the app use.
pub const CUBE_MONO_REGULAR: &[u8] = include_bytes!("../../../fonts/CubeMono-Regular.ttf");
pub const CUBE_MONO_MEDIUM: &[u8] = include_bytes!("../../../fonts/CubeMono-Medium.ttf");

const W: f32 = WIDTH as f32;
const H: f32 = HEIGHT as f32;
const MARGIN: f32 = 56.;
const CONTENT_WIDTH: f32 = W - 2. * MARGIN;
/// The width of the cube face on the far side of the card.
const FACE_WIDTH: f32 = 480.;
const FACE_PADDING: f32 = 14.;
const STICKER_GAP: f32 = 10.;
/// The room for words and the result, beside the face.
const COLUMN: f32 = CONTENT_WIDTH - FACE_WIDTH - 40.;
const HERO_MAX_SIZE: f32 = 168.;
/// How much tighter than its natural spacing the result is set.
const HERO_TRACKING: f32 = -0.04;
const CHIP_HEIGHT: f32 = 46.;
/// The box of the small preview an average has beside its date.
const SMALL_PREVIEW_WIDTH: f32 = 226.;
const SMALL_PREVIEW_HEIGHT: f32 = 170.;
/// The tallest the scramble text of a single may get.
const SCRAMBLE_MAX_HEIGHT: f32 = 162.;
const SCRAMBLE_SIZES: [f32; 7] = [26., 23., 20., 18., 16., 14., 13.];
const SCRAMBLE_LINE_HEIGHT: f32 = 1.42;
/// How much of a font size one Latin character takes in a monospace font.
/// Proportional system fonts are narrower, so nothing built on this overflows.
const NARROW_ADVANCE: f32 = 0.6;
const WIDE_ADVANCE: f32 = 1.;

/// The colors of a card: the current theme's, and nothing else.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CardColors {
    pub background: ThemeColor,
    pub text: ThemeColor,
    pub muted: ThemeColor,
    pub accent: ThemeColor,
}

impl CardColors {
    /// The cube's body, just off the background.
    fn body(self) -> ThemeColor {
        mix(self.background, self.text, 0.045)
    }

    /// A sticker on that body.
    fn sticker(self) -> ThemeColor {
        mix(self.background, self.text, 0.11)
    }

    /// Whichever of the theme's two ends reads best on the accent.
    fn on_accent(self) -> ThemeColor {
        let accent = luminance(self.accent);
        let contrast = |color| {
            let other = luminance(color);
            (accent.max(other) + 0.05) / (accent.min(other) + 0.05)
        };
        if contrast(self.background) >= contrast(self.text) {
            self.background
        } else {
            self.text
        }
    }
}

/// The typeface of a card.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CardFont {
    /// Cube Mono throughout.
    #[default]
    Bundled,
    /// A clean system sans for words, and a system monospace for times.
    System,
}

/// How a card looks, as opposed to what it says.
#[derive(Clone, Copy, Debug)]
pub struct CardStyle<'a> {
    pub colors: CardColors,
    pub font: CardFont,
    /// The interface's language code, such as `en` or `ar`.
    pub language: &'a str,
}

/// The words of a card, in the interface's language.
#[derive(Clone, Copy, Debug, Default)]
pub struct CardLabels<'a> {
    /// What the result is: `Single` or `Average of 5`.
    pub title: &'a str,
    /// When it was set; blank leaves it out.
    pub date: &'a str,
    /// A line of praise such as `Session best`.
    pub badge: Option<&'a str>,
}

/// One attempt of an average.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Attempt {
    text: String,
    trimmed: bool,
    /// Whether this is the fastest, which gets the accent.
    fastest: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Body {
    Single { scramble: String },
    Average(Vec<Attempt>),
}

/// What one card says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShareCard {
    event: WcaEvent,
    title: String,
    result: String,
    date: String,
    badge: Option<String>,
    body: Body,
    preview: Option<Preview>,
}

impl ShareCard {
    /// The card of one solve.
    pub fn single(solve: &Solve, labels: CardLabels<'_>) -> Self {
        Self {
            event: solve.event(),
            title: labels.title.to_owned(),
            result: solve.to_string(),
            date: labels.date.to_owned(),
            badge: labels.badge.map(str::to_owned),
            body: Body::Single {
                scramble: scramble::display_text(solve.event(), solve.scramble()),
            },
            preview: draw_preview(solve),
        }
    }

    /// The card of an average, or `None` when it has more attempts than a card
    /// can list.
    pub fn average(detail: &AverageDetail<'_>, labels: CardLabels<'_>) -> Option<Self> {
        let attempts = detail.attempts();
        if attempts.len() > MAX_ATTEMPTS {
            return None;
        }
        let fastest = (0..attempts.len())
            .filter(|&position| attempts[position].effective_ms().is_some())
            .min_by_key(|&position| attempts[position].effective_ms());
        let rows = (0..attempts.len())
            .map(|position| Attempt {
                text: detail.attempt_text(position),
                trimmed: detail.trim(position).is_some(),
                fastest: fastest == Some(position),
            })
            .collect();
        Some(Self {
            event: attempts.last()?.event(),
            title: labels.title.to_owned(),
            result: match detail.average() {
                Average::Time(ms) => crate::format::format_ms(ms),
                Average::Dnf => "DNF".to_owned(),
            },
            date: labels.date.to_owned(),
            badge: labels.badge.map(str::to_owned),
            body: Body::Average(rows),
            preview: featured_attempt(detail).and_then(draw_preview),
        })
    }

    /// Lays the card out as an SVG document.
    pub fn svg(&self, style: CardStyle<'_>) -> String {
        let mut canvas = Canvas::new(style);
        let bottom = H - MARGIN;
        let hero_bottom = match &self.body {
            Body::Average(attempts) => {
                canvas.face(attempts);
                let top = bottom - SMALL_PREVIEW_HEIGHT;
                let mut footer_start = MARGIN;
                if let Some(preview) = &self.preview {
                    canvas.small_preview(preview, top);
                    footer_start += SMALL_PREVIEW_WIDTH + 24.;
                }
                canvas.footer(&self.date, footer_start, bottom);
                top
            }
            Body::Single { scramble } => {
                let lines = fit_scramble(scramble);
                let top = bottom - lines.height();
                canvas.scramble(&lines, top);
                let room = top - 26.;
                if let Some(preview) = &self.preview {
                    canvas.large_preview(preview, room);
                }
                canvas.footer(&self.date, MARGIN, room);
                room - 30.
            }
        };
        canvas.heading(self.event.name(), &self.title, self.badge.as_deref());
        canvas.hero(&self.result, hero_bottom);
        canvas.finish()
    }

    /// Returns a file name for the card, with `label` saying what it shows.
    pub fn file_name(&self, label: &str) -> String {
        let name = format!("cube-{}-{label}-{}", self.event.name(), self.result);
        let safe: String = name
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') {
                    c
                } else {
                    '-'
                }
            })
            .collect();
        format!("{}.png", safe.trim_end_matches('-'))
    }
}

/// The attempt an average shows and links to: its fastest with a scramble.
pub fn featured_attempt<'a>(detail: &AverageDetail<'a>) -> Option<&'a Solve> {
    detail
        .attempts()
        .iter()
        .filter(|solve| !solve.scramble().trim().is_empty())
        .min_by_key(|solve| solve.effective_ms().unwrap_or(u64::MAX))
}

/// Draws the puzzle as `solve`'s scramble leaves it, if it can be drawn.
fn draw_preview(solve: &Solve) -> Option<Preview> {
    let text = solve.scramble().trim();
    if text.is_empty() {
        return None;
    }
    scramble::preview(solve.event(), text)
}

/// Splits a color into its red, green and blue, from `0.0` to `1.0`.
fn channels(color: ThemeColor) -> [f32; 3] {
    let rgb = color.rgb();
    [rgb >> 16, rgb >> 8, rgb].map(|channel| (channel & 0xff) as f32 / 255.)
}

fn from_channels([r, g, b]: [f32; 3]) -> ThemeColor {
    let byte = |channel: f32| (channel.clamp(0., 1.) * 255.).round() as u8;
    ThemeColor::new(byte(r), byte(g), byte(b))
}

/// Blends `over` onto `base` by `amount`.
fn mix(base: ThemeColor, over: ThemeColor, amount: f32) -> ThemeColor {
    let (base, over) = (channels(base), channels(over));
    from_channels([0, 1, 2].map(|index| base[index] + (over[index] - base[index]) * amount))
}

/// Returns the relative luminance WCAG contrast is measured with.
fn luminance(color: ThemeColor) -> f32 {
    let linear = channels(color).map(|channel| {
        if channel <= 0.040_45 {
            channel / 12.92
        } else {
            ((channel + 0.055) / 1.055).powf(2.4)
        }
    });
    0.2126 * linear[0] + 0.7152 * linear[1] + 0.0722 * linear[2]
}

/// Returns how many columns and rows of stickers hold `count` attempts: a
/// cube's face fits 12 as four rows of three.
const fn sticker_grid(count: usize) -> (usize, usize) {
    let columns = match count {
        0..=3 => 1,
        4..=6 => 2,
        _ => 3,
    };
    (columns, count.div_ceil(columns))
}

/// Estimates how wide `text` is at `size`.
fn text_width(text: &str, size: f32) -> f32 {
    text.chars()
        .map(|c| {
            if is_wide(c) {
                WIDE_ADVANCE
            } else {
                NARROW_ADVANCE
            }
        })
        .sum::<f32>()
        * size
}

/// Returns whether `c` is a full-width CJK or Hangul character.
const fn is_wide(c: char) -> bool {
    matches!(
        c as u32,
        0x1100..=0x11ff | 0x2e80..=0xa4cf | 0xac00..=0xd7a3 | 0xf900..=0xfaff | 0xff00..=0xff60
    )
}

/// Returns the largest size up to `max` at which `text` fits in `width`.
fn fit_size(text: &str, max: f32, width: f32) -> f32 {
    let natural = text_width(text, 1.);
    if natural <= 0. {
        max
    } else {
        (width / natural).min(max)
    }
}

/// Folds `text` at spaces so no line passes `columns` characters. Line breaks
/// stay where they are, which keeps a megaminx scramble in its rows.
fn wrap(text: &str, columns: usize) -> Vec<String> {
    let mut lines = Vec::new();
    for row in text.lines() {
        let mut line = String::new();
        for word in row.split(' ').filter(|word| !word.is_empty()) {
            let width = line.chars().count() + 1 + word.chars().count();
            if !line.is_empty() && width > columns {
                lines.push(std::mem::take(&mut line));
            }
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(word);
        }
        if !line.is_empty() {
            lines.push(line);
        }
    }
    lines
}

/// A scramble folded to fit the card.
#[derive(Clone, Debug, PartialEq)]
struct ScrambleLines {
    lines: Vec<String>,
    size: f32,
}

impl ScrambleLines {
    fn line_height(&self) -> f32 {
        self.size * SCRAMBLE_LINE_HEIGHT
    }

    fn height(&self) -> f32 {
        self.lines.len() as f32 * self.line_height()
    }
}

/// Finds the largest size that fits the whole scramble, and cuts it short only
/// when even the smallest does not.
fn fit_scramble(text: &str) -> ScrambleLines {
    let columns = |size: f32| (CONTENT_WIDTH / (size * NARROW_ADVANCE)) as usize;
    for size in SCRAMBLE_SIZES {
        let lines = wrap(text, columns(size));
        let fitted = ScrambleLines { lines, size };
        if fitted.height() <= SCRAMBLE_MAX_HEIGHT {
            return fitted;
        }
    }
    let size = SCRAMBLE_SIZES[SCRAMBLE_SIZES.len() - 1];
    let room = (SCRAMBLE_MAX_HEIGHT / (size * SCRAMBLE_LINE_HEIGHT)) as usize;
    let mut lines = wrap(text, columns(size));
    lines.truncate(room);
    if let Some(last) = lines.last_mut() {
        last.push_str(" …");
    }
    ScrambleLines { lines, size }
}

/// Escapes text for XML, dropping the control characters XML cannot hold.
fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            c if c.is_control() => {}
            c => escaped.push(c),
        }
    }
    escaped
}

/// Returns the fonts of the words of a card.
///
/// Scripts Cube Mono lacks come first, in the face that writes them properly
/// on each platform: the same character can differ between Chinese and
/// Japanese, so leaving the choice to fallback could pick the wrong one. Cube
/// Mono closes every list because it is always there.
fn word_fonts(font: CardFont, language: &str) -> String {
    let script = match language {
        "ar" => "'Segoe UI','Geeza Pro','Noto Sans Arabic','Noto Naskh Arabic',",
        "ja" => "'Yu Gothic UI','Yu Gothic','Hiragino Sans','Noto Sans CJK JP',",
        "ko" => "'Malgun Gothic','Apple SD Gothic Neo','Noto Sans CJK KR',",
        "zh" => "'Microsoft YaHei UI','Microsoft YaHei','PingFang SC','Noto Sans CJK SC',",
        _ => "",
    };
    let base = match font {
        CardFont::Bundled => "'Cube Mono'",
        CardFont::System => "'Segoe UI','Helvetica Neue','Noto Sans','DejaVu Sans','Cube Mono'",
    };
    format!("{script}{base}")
}

/// Returns the fonts of the times of a card.
const fn time_fonts(font: CardFont) -> &'static str {
    match font {
        CardFont::Bundled => "'Cube Mono'",
        CardFont::System => "'Cascadia Mono',Consolas,Menlo,'DejaVu Sans Mono','Cube Mono'",
    }
}

/// Which side of its position a piece of text grows from, before mirroring.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Align {
    Start,
    Middle,
}

/// One piece of text.
struct Label<'a> {
    x: f32,
    y: f32,
    size: f32,
    fill: ThemeColor,
    align: Align,
    /// Times and dates read left to right in every language.
    time: bool,
    weight: u32,
    text: &'a str,
}

/// The card being drawn. Every horizontal position goes through [`Self::x`], so
/// a right-to-left card comes out as a mirror image.
struct Canvas {
    svg: String,
    colors: CardColors,
    rtl: bool,
    words: String,
    times: &'static str,
}

impl Canvas {
    fn new(style: CardStyle<'_>) -> Self {
        let svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {WIDTH} {HEIGHT}" width="{WIDTH}" height="{HEIGHT}" xml:lang="{}"><rect width="{WIDTH}" height="{HEIGHT}" fill="{}"/>"#,
            escape(style.language),
            style.colors.background.to_hex(),
        );
        Self {
            svg,
            colors: style.colors,
            rtl: style.language == "ar",
            words: word_fonts(style.font, style.language),
            times: time_fonts(style.font),
        }
    }

    /// Moves a position to the other side on a right-to-left card.
    fn x(&self, x: f32) -> f32 {
        if self.rtl { W - x } else { x }
    }

    /// Returns the left edge of a box that starts at `x` and is `width` wide.
    fn left(&self, x: f32, width: f32) -> f32 {
        if self.rtl { W - x - width } else { x }
    }

    fn label(&mut self, label: &Label<'_>) {
        let anchor = match (label.align, self.rtl) {
            (Align::Middle, _) => "middle",
            (Align::Start, false) => "start",
            (Align::Start, true) => "end",
        };
        let family = if label.time {
            self.times
        } else {
            self.words.as_str()
        };
        let mut text = escape(label.text);
        if self.rtl && !label.time {
            // An embedding: the rasterizer's own paragraph is always left to
            // right, which would send numbers to the wrong end of a sentence.
            text = format!("\u{202b}{text}\u{202c}");
        }
        let _ = write!(
            self.svg,
            r#"<text x="{:.1}" y="{:.1}" font-size="{:.1}" font-family="{family}" font-weight="{}" text-anchor="{anchor}" fill="{}">{text}</text>"#,
            self.x(label.x),
            label.y,
            label.size,
            label.weight,
            label.fill.to_hex(),
        );
    }

    fn rect(&mut self, x: f32, y: f32, size: (f32, f32), radius: f32, fill: ThemeColor) {
        let _ = write!(
            self.svg,
            r#"<rect x="{x:.1}" y="{y:.1}" width="{:.1}" height="{:.1}" rx="{radius}" fill="{}"/>"#,
            size.0,
            size.1,
            fill.to_hex(),
        );
    }

    /// The event, what the result is and any praise.
    fn heading(&mut self, event: &str, title: &str, badge: Option<&str>) {
        let chip_size = 24.;
        let chip_width = text_width(event, chip_size) + 36.;
        let left = self.left(MARGIN, chip_width);
        self.rect(
            left,
            MARGIN,
            (chip_width, CHIP_HEIGHT),
            12.,
            self.colors.accent,
        );
        self.label(&Label {
            x: MARGIN + chip_width / 2.,
            y: MARGIN + CHIP_HEIGHT / 2. + chip_size * 0.35,
            size: chip_size,
            fill: self.colors.on_accent(),
            align: Align::Middle,
            time: true,
            weight: 500,
            text: event,
        });
        let size = fit_size(title, 30., COLUMN - chip_width - 18.);
        self.label(&Label {
            x: MARGIN + chip_width + 18.,
            y: MARGIN + CHIP_HEIGHT / 2. + size * 0.35,
            size,
            fill: self.colors.text,
            align: Align::Start,
            time: false,
            weight: 500,
            text: title,
        });
        if let Some(badge) = badge {
            self.label(&Label {
                x: MARGIN,
                y: MARGIN + CHIP_HEIGHT + 40.,
                size: fit_size(badge, 24., COLUMN),
                fill: self.colors.accent,
                align: Align::Start,
                time: false,
                weight: 500,
                text: badge,
            });
        }
    }

    /// The result, as large as it fits.
    fn hero(&mut self, result: &str, bottom: f32) {
        let size = fit_size(result, HERO_MAX_SIZE, COLUMN);
        let y = f32::midpoint(MARGIN + CHIP_HEIGHT, bottom) + size * 0.36;
        let anchor = if self.rtl { "end" } else { "start" };
        let _ = write!(
            self.svg,
            r#"<text x="{:.1}" y="{y:.1}" font-size="{size:.1}" font-family="{}" font-weight="500" letter-spacing="{:.1}" text-anchor="{anchor}" fill="{}">{}</text>"#,
            self.x(MARGIN - size * 0.03),
            self.times,
            size * HERO_TRACKING,
            self.colors.text.to_hex(),
            escape(result),
        );
    }

    /// The date, along the column's bottom edge.
    fn footer(&mut self, date: &str, start: f32, baseline: f32) {
        self.label(&Label {
            x: start,
            y: baseline,
            size: 20.,
            fill: self.colors.muted,
            align: Align::Start,
            time: true,
            weight: 400,
            text: date,
        });
    }

    /// Draws `preview` in the given box.
    fn preview(
        &mut self,
        preview: &Preview,
        (x, y): (f32, f32),
        (width, height): (f32, f32),
        align: &str,
    ) {
        let Some(inner) = inner_svg(&preview.svg) else {
            return;
        };
        let _ = write!(
            self.svg,
            r#"<svg x="{x:.1}" y="{y:.1}" width="{width:.1}" height="{height:.1}" viewBox="0 0 {} {}" preserveAspectRatio="{align} meet">{}</svg>"#,
            preview.width, preview.height, inner,
        );
    }

    /// The preview of an average, tucked into the corner under the result.
    fn small_preview(&mut self, preview: &Preview, top: f32) {
        let left = self.left(MARGIN, SMALL_PREVIEW_WIDTH);
        let align = if self.rtl { "xMaxYMax" } else { "xMinYMax" };
        self.preview(
            preview,
            (left, top),
            (SMALL_PREVIEW_WIDTH, SMALL_PREVIEW_HEIGHT),
            align,
        );
    }

    /// The preview of a single, on the cube's body down to `bottom`.
    fn large_preview(&mut self, preview: &Preview, bottom: f32) {
        let left = self.left(W - MARGIN - FACE_WIDTH, FACE_WIDTH);
        let height = bottom - MARGIN;
        self.rect(left, MARGIN, (FACE_WIDTH, height), 28., self.colors.body());
        let pad = FACE_PADDING + 14.;
        self.preview(
            preview,
            (left + pad, MARGIN + pad),
            (FACE_WIDTH - 2. * pad, height - 2. * pad),
            "xMidYMid",
        );
    }

    /// The scramble of a single, across the card.
    fn scramble(&mut self, lines: &ScrambleLines, top: f32) {
        for (index, line) in lines.lines.iter().enumerate() {
            self.label(&Label {
                x: MARGIN,
                y: top + lines.size + index as f32 * lines.line_height(),
                size: lines.size,
                fill: self.colors.muted,
                align: Align::Start,
                time: true,
                weight: 400,
                text: line,
            });
        }
    }

    /// The attempts of an average as the stickers of a cube face, oldest
    /// first. A grid with a cell to spare keeps it as a blank sticker.
    fn face(&mut self, attempts: &[Attempt]) {
        let colors = self.colors;
        let x = W - MARGIN - FACE_WIDTH;
        let height = H - 2. * MARGIN;
        let left = self.left(x, FACE_WIDTH);
        self.rect(left, MARGIN, (FACE_WIDTH, height), 28., colors.body());

        let (columns, rows) = sticker_grid(attempts.len());
        let inner = |length: f32, cells: usize| {
            (length - 2. * FACE_PADDING - (cells - 1) as f32 * STICKER_GAP) / cells as f32
        };
        let size = (inner(FACE_WIDTH, columns), inner(height, rows));
        let longest = attempts
            .iter()
            .map(|attempt| attempt.text.as_str())
            .max_by_key(|text| text.chars().count())
            .unwrap_or_default();
        let text_size = fit_size(longest, 46., size.0 - 28.);
        for cell in 0..columns * rows {
            let cell_x = x + FACE_PADDING + (cell % columns) as f32 * (size.0 + STICKER_GAP);
            let cell_y = MARGIN + FACE_PADDING + (cell / columns) as f32 * (size.1 + STICKER_GAP);
            if let Some(attempt) = attempts.get(cell) {
                self.sticker(attempt, cell + 1, (cell_x, cell_y), size, text_size);
            } else {
                let left = self.left(cell_x, size.0);
                let blank = mix(colors.body(), colors.sticker(), 0.4);
                self.rect(left, cell_y, size, 16., blank);
            }
        }
    }

    fn sticker(
        &mut self,
        attempt: &Attempt,
        number: usize,
        (x, y): (f32, f32),
        size: (f32, f32),
        text_size: f32,
    ) {
        let colors = self.colors;
        let (fill, text, caption) = if attempt.fastest {
            (colors.accent, colors.on_accent(), colors.on_accent())
        } else if attempt.trimmed {
            (colors.sticker(), colors.muted, colors.muted)
        } else {
            (colors.sticker(), colors.text, colors.muted)
        };
        let left = self.left(x, size.0);
        self.rect(left, y, size, 16., fill);
        self.label(&Label {
            x: x + 14.,
            y: y + 25.,
            size: 15.,
            fill: caption,
            align: Align::Start,
            time: true,
            weight: 400,
            text: &number.to_string(),
        });
        self.label(&Label {
            x: x + size.0 / 2.,
            y: y + size.1 / 2. + text_size * 0.35,
            size: text_size,
            fill: text,
            align: Align::Middle,
            time: true,
            weight: 500,
            text: &attempt.text,
        });
    }

    fn finish(mut self) -> String {
        self.svg.push_str("</svg>");
        self.svg
    }
}

/// Returns what is inside the root element of an SVG document.
fn inner_svg(svg: &str) -> Option<&str> {
    let root = svg.find("<svg")?;
    let open = root + svg[root..].find('>')?;
    let close = svg.rfind("</svg>")?;
    svg.get(open + 1..close)
}

/// The fonts a card can use: Cube Mono and whatever the system has, for the
/// scripts it does not cover. Reading them is slow, so it happens once.
fn fonts() -> Arc<usvg::fontdb::Database> {
    static FONTS: OnceLock<Arc<usvg::fontdb::Database>> = OnceLock::new();
    FONTS
        .get_or_init(|| {
            let mut fonts = usvg::fontdb::Database::new();
            fonts.load_font_data(CUBE_MONO_REGULAR.to_vec());
            fonts.load_font_data(CUBE_MONO_MEDIUM.to_vec());
            fonts.load_system_fonts();
            Arc::new(fonts)
        })
        .clone()
}

/// Rasterizes a card into a PNG. This takes a moment, so keep it off the thread
/// drawing the interface.
pub fn render_png(svg: &str) -> Result<Vec<u8>> {
    let options = usvg::Options {
        font_family: "Cube Mono".to_owned(),
        fontdb: fonts(),
        ..usvg::Options::default()
    };
    let tree = usvg::Tree::from_str(svg, &options).context("Could not read the card")?;
    let mut pixmap = tiny_skia::Pixmap::new(WIDTH * PNG_SCALE, HEIGHT * PNG_SCALE)
        .ok_or_else(|| anyhow!("Could not make room for the card"))?;
    let scale = PNG_SCALE as f32;
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    pixmap.encode_png().context("Could not encode the card")
}

/// Saves a card at `path`, adding the extension when it is missing, and
/// returns where it ended up.
pub fn save_png(path: &Path, png: &[u8]) -> Result<PathBuf> {
    let is_png = path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("png"));
    let path = if is_png {
        path.to_owned()
    } else {
        let mut name = path.as_os_str().to_owned();
        name.push(".png");
        PathBuf::from(name)
    };
    write_atomic(&path, png).with_context(|| format!("Could not save {}", path.display()))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::solve::Modifier;

    const COLORS: CardColors = CardColors {
        background: ThemeColor::BLACK,
        text: ThemeColor::WHITE,
        muted: ThemeColor::new(128, 128, 128),
        accent: ThemeColor::BLUE,
    };

    fn style(language: &str) -> CardStyle<'_> {
        CardStyle {
            colors: COLORS,
            font: CardFont::Bundled,
            language,
        }
    }

    fn solve(ms: u64, modifier: Modifier) -> Solve {
        Solve::new_with_meta(
            ms,
            WcaEvent::Cube3x3,
            "R U R' U'".to_owned(),
            1,
            modifier,
            String::new(),
        )
    }

    fn solves(count: u64) -> Vec<Solve> {
        (0..count)
            .map(|index| solve(10_000 + index * 137, Modifier::None))
            .collect()
    }

    fn labels(title: &str) -> CardLabels<'_> {
        CardLabels {
            title,
            date: "2026-11-06 12:30",
            badge: None,
        }
    }

    fn average_card(solves: &[Solve], size: usize, title: &str) -> Option<ShareCard> {
        let detail = AverageDetail::at(solves, size - 1, size).unwrap();
        ShareCard::average(&detail, labels(title))
    }

    /// Parses a card without any fonts, which is all the structure needs.
    fn parses(svg: &str) -> bool {
        usvg::Tree::from_str(svg, &usvg::Options::default()).is_ok()
    }

    #[test]
    fn a_single_says_what_it_is() {
        let card = ShareCard::single(&solve(12_340, Modifier::PlusTwo), labels("Single"));
        let svg = card.svg(style("en"));
        for text in ["3x3x3", "Single", "14.340+", "2026-11-06 12:30"] {
            assert!(svg.contains(text), "{text} is missing");
        }
        assert!(svg.contains("R U R&apos; U&apos;"));
        assert!(!svg.contains(">cube<"));
        assert!(parses(&svg));
    }

    #[test]
    fn every_average_up_to_twelve_lists_its_attempts() {
        let solves = solves(12);
        for size in [3, 5, 12] {
            let svg = average_card(&solves, size, "Average")
                .unwrap()
                .svg(style("en"));
            assert!(parses(&svg));
            let detail = AverageDetail::at(&solves, size - 1, size).unwrap();
            for position in 0..size {
                assert!(svg.contains(&detail.attempt_text(position)));
            }
        }
    }

    #[test]
    fn longer_averages_have_no_card() {
        assert!(average_card(&solves(50), 50, "Average").is_none());
    }

    #[test]
    fn an_average_marks_what_it_trims() {
        let svg = average_card(&solves(5), 5, "Average")
            .unwrap()
            .svg(style("en"));
        assert!(svg.contains("(10.000)"));
        assert!(svg.contains("(10.548)"));
        assert!(!svg.contains("(10.137)"));
    }

    #[test]
    fn a_dnf_average_says_so_and_still_marks_its_fastest() {
        let mut attempts = solves(5);
        attempts[0] = solve(1, Modifier::DNF);
        attempts[1] = solve(1, Modifier::DNF);
        let card = average_card(&attempts, 5, "Average").unwrap();
        assert_eq!(card.result, "DNF");
        let Body::Average(rows) = &card.body else {
            panic!("an average lists its attempts");
        };
        let fastest: Vec<_> = rows.iter().map(|row| row.fastest).collect();
        assert_eq!(fastest, [false, false, true, false, false]);
    }

    #[test]
    fn an_average_of_nothing_but_dnfs_has_no_fastest() {
        let attempts: Vec<_> = (0..5).map(|_| solve(1, Modifier::DNF)).collect();
        let card = average_card(&attempts, 5, "Average").unwrap();
        let Body::Average(rows) = &card.body else {
            panic!("an average lists its attempts");
        };
        assert!(rows.iter().all(|row| !row.fastest));
        assert!(parses(&card.svg(style("en"))));
    }

    #[test]
    fn stickers_fill_a_cube_face() {
        assert_eq!(sticker_grid(3), (1, 3));
        assert_eq!(sticker_grid(5), (2, 3));
        assert_eq!(sticker_grid(12), (3, 4));
        for count in 1..=MAX_ATTEMPTS {
            let (columns, rows) = sticker_grid(count);
            assert!(columns * rows >= count);
            assert!(
                columns * (rows - 1) < count,
                "a whole row of {count} is blank"
            );
        }
    }

    #[test]
    fn a_card_without_a_preview_only_uses_the_themes_colors() {
        let bare = Solve::new_with_meta(
            1_000,
            WcaEvent::Cube3x3,
            String::new(),
            1,
            Modifier::None,
            String::new(),
        );
        let attempts = vec![bare; 5];
        let svg = average_card(&attempts, 5, "Average")
            .unwrap()
            .svg(style("en"));
        let allowed = [
            COLORS.background,
            COLORS.text,
            COLORS.muted,
            COLORS.accent,
            COLORS.on_accent(),
            COLORS.body(),
            COLORS.sticker(),
            mix(COLORS.body(), COLORS.sticker(), 0.4),
        ]
        .map(ThemeColor::to_hex);
        for (at, _) in svg.match_indices('#') {
            let color = &svg[at..at + 7];
            assert!(
                allowed.iter().any(|hex| hex == color),
                "{color} is not the theme's"
            );
        }
    }

    #[test]
    fn the_text_on_the_accent_is_the_end_of_the_theme_that_reads_best() {
        let dark = CardColors {
            accent: ThemeColor::new(255, 215, 0),
            ..COLORS
        };
        assert_eq!(dark.on_accent(), COLORS.background);
        let deep = CardColors {
            accent: ThemeColor::new(20, 40, 120),
            ..COLORS
        };
        assert_eq!(deep.on_accent(), COLORS.text);
    }

    #[test]
    fn the_preview_keeps_the_colors_it_has_in_the_app() {
        let card = ShareCard::single(&solve(1_000, Modifier::None), labels("Single"));
        let svg = card.svg(style("en"));
        let preview = card.preview.unwrap();
        assert!(svg.contains(inner_svg(&preview.svg).unwrap()));
    }

    #[test]
    fn awkward_text_cannot_break_the_document() {
        let card = ShareCard::single(
            &solve(1_000, Modifier::None),
            CardLabels {
                title: "<a> & \"b\" 'c' \u{1}",
                date: "</svg>",
                badge: Some("]]>"),
            },
        );
        let svg = card.svg(style("en"));
        assert!(svg.contains("&lt;a&gt; &amp; &quot;b&quot; &apos;c&apos; "));
        assert!(parses(&svg));
    }

    #[test]
    fn right_to_left_cards_are_mirrored_and_keep_times_left_to_right() {
        let card = average_card(&solves(5), 5, "متوسط 5").unwrap();
        let ltr = card.svg(style("en"));
        let rtl = card.svg(style("ar"));
        assert!(parses(&rtl));
        assert!(!ltr.contains('\u{202b}'));
        assert!(rtl.contains("\u{202b}متوسط 5\u{202c}"));
        assert!(rtl.contains(">2026-11-06 12:30<"));
        assert!(rtl.contains(">10.137<"));
        // The event's chip sits at the far edge.
        let chip = W - MARGIN - (text_width("3x3x3", 24.) + 36.);
        assert!(rtl.contains(&format!(r#"<rect x="{chip:.1}" y="{MARGIN:.1}""#)));
    }

    #[test]
    fn scripts_cube_mono_lacks_ask_for_their_own_fonts_first() {
        for (language, font) in [
            ("ar", "Noto Sans Arabic"),
            ("ja", "Noto Sans CJK JP"),
            ("ko", "Noto Sans CJK KR"),
            ("zh", "Noto Sans CJK SC"),
        ] {
            let fonts = word_fonts(CardFont::Bundled, language);
            assert!(fonts.contains(font));
            assert!(fonts.ends_with("'Cube Mono'"));
        }
        assert_eq!(word_fonts(CardFont::Bundled, "en"), "'Cube Mono'");
    }

    #[test]
    fn system_cards_use_system_faces_and_can_still_fall_back() {
        let card = ShareCard::single(&solve(1_000, Modifier::None), labels("Single"));
        let svg = card.svg(CardStyle {
            font: CardFont::System,
            ..style("en")
        });
        assert!(svg.contains("Segoe UI"));
        assert!(word_fonts(CardFont::System, "en").ends_with("'Cube Mono'"));
        assert!(time_fonts(CardFont::System).ends_with("'Cube Mono'"));
    }

    #[test]
    fn long_results_shrink_to_fit() {
        let column = 600.;
        let size = fit_size("1:23:45.678", HERO_MAX_SIZE, column);
        assert!(size < HERO_MAX_SIZE);
        assert!(text_width("1:23:45.678", size) <= column + 0.01);
        assert!((fit_size("9.87", HERO_MAX_SIZE, column) - HERO_MAX_SIZE).abs() < 0.01);
    }

    #[test]
    fn wide_characters_count_for_more() {
        assert!(text_width("平均", 10.) > text_width("ab", 10.));
    }

    #[test]
    fn wrapping_keeps_rows_and_stays_in_its_columns() {
        let lines = wrap("R++ D-- U\nR-- D++ U'", 80);
        assert_eq!(lines, ["R++ D-- U", "R-- D++ U'"]);
        let lines = wrap("R U F L B D R U F L B D", 7);
        assert!(lines.iter().all(|line| line.chars().count() <= 7));
        assert_eq!(lines.join(" "), "R U F L B D R U F L B D");
    }

    #[test]
    fn a_scramble_always_fits_and_only_the_absurd_is_cut() {
        let seven = "3Rw2 ".repeat(100);
        let fitted = fit_scramble(&seven);
        assert!(fitted.height() <= SCRAMBLE_MAX_HEIGHT);
        assert!(!fitted.lines.concat().contains('…'));

        let absurd = "3Rw2 ".repeat(2000);
        let fitted = fit_scramble(&absurd);
        assert!(fitted.height() <= SCRAMBLE_MAX_HEIGHT);
        assert!(fitted.lines.last().unwrap().ends_with('…'));
    }

    #[test]
    fn the_featured_attempt_is_the_fastest_with_a_scramble() {
        let mut attempts = solves(5);
        attempts[0] = Solve::new_with_meta(
            1,
            WcaEvent::Cube3x3,
            String::new(),
            1,
            Modifier::None,
            String::new(),
        );
        let detail = AverageDetail::at(&attempts, 4, 5).unwrap();
        let featured = featured_attempt(&detail).unwrap();
        assert_eq!(featured.raw_ms(), 10_137);
    }

    #[test]
    fn a_solve_without_a_scramble_has_no_preview() {
        let bare = Solve::new_with_meta(
            1_000,
            WcaEvent::Cube3x3,
            String::new(),
            1,
            Modifier::None,
            String::new(),
        );
        let card = ShareCard::single(&bare, labels("Single"));
        assert_eq!(card.preview, None);
        assert!(parses(&card.svg(style("en"))));
    }

    #[test]
    fn the_preview_sits_inside_the_card() {
        let card = ShareCard::single(&solve(1_000, Modifier::None), labels("Single"));
        let svg = card.svg(style("en"));
        let preview = card.preview.unwrap();
        assert!(svg.contains(&format!(
            r#"viewBox="0 0 {} {}""#,
            preview.width, preview.height
        )));
        assert_eq!(svg.matches("<svg").count(), 2);
    }

    #[test]
    fn file_names_are_safe_on_every_system() {
        let slow = ShareCard::single(&solve(83_456, Modifier::PlusTwo), labels("Single"));
        assert_eq!(slow.file_name("single"), "cube-3x3x3-single-1-25.456.png");
    }

    #[test]
    fn cards_rasterize_at_twice_their_size() {
        for (language, title) in [("en", "Single"), ("ar", "متوسط 5"), ("ja", "5回の平均")]
        {
            let card = ShareCard::single(&solve(9_876, Modifier::None), labels(title));
            let png = render_png(&card.svg(style(language))).unwrap();
            assert_eq!(&png[1..4], b"PNG");
            let dimension =
                |at: usize| u32::from_be_bytes([png[at], png[at + 1], png[at + 2], png[at + 3]]);
            assert_eq!(dimension(16), WIDTH * PNG_SCALE);
            assert_eq!(dimension(20), HEIGHT * PNG_SCALE);
        }
    }

    #[test]
    fn the_bundled_font_draws_the_result() {
        let draw = |ms| {
            let card = ShareCard::single(&solve(ms, Modifier::None), labels("Single"));
            render_png(&card.svg(style("en"))).unwrap()
        };
        assert_ne!(draw(1_000), draw(2_000));
    }

    #[test]
    fn saving_adds_the_extension_only_when_it_is_missing() {
        let dir = std::env::temp_dir().join(format!("cube-card-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let bare = save_png(&dir.join("card"), b"png").unwrap();
        assert_eq!(bare, dir.join("card.png"));
        let named = save_png(&dir.join("other.PNG"), b"png").unwrap();
        assert_eq!(named, dir.join("other.PNG"));
        assert_eq!(fs::read(&bare).unwrap(), b"png");
        fs::remove_dir_all(&dir).unwrap();
    }
}
