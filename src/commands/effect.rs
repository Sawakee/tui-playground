// effect コマンドと、エフェクト機能の本体をまとめたファイル。
//   - コマンド: `Cmd`（"effect" と打つとエフェクト画面を開く）
//   - 画面コントローラ: `Effects`（状態・キー処理・描画）
//   - 描画エンジン: Effect トレイト / Canvas / Ctx / 各エフェクト / MODES
// main.rs はこの `Effects` を保持し、画面遷移だけを担当する。

use super::Command;
use crate::App;

use crossterm::event::KeyCode;
use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

// ─── effect コマンド ─────────────────────────────────────────
pub struct Cmd;

impl Command for Cmd {
    fn name(&self) -> &'static str {
        "effect"
    }

    fn help(&self) -> &'static str {
        "エフェクト画面を開く"
    }

    fn run(&self, app: &mut App) {
        app.open_effects();
    }
}

// 位相は毎フレーム加算される「時間」。各エフェクトがこれを使って動く。
const PHASE_SPEED: f64 = 0.30;
// 文字セルは「横より縦に長い」ので、縦方向の距離を補正して円を丸くする。
const CELL_ASPECT: f64 = 2.0;

// ─── モード表（ここに1行足すだけで新しい効果を追加できる）─────────
// 効果は状態を持たない（位相 ctx.phase から毎フレーム計算する）ので、
// static なゼロサイズ値への参照を &dyn Effect として持てばよい。
struct Mode {
    label: &'static str,
    effect: &'static dyn Effect,
}

const MODES: &[Mode] = &[
    Mode { label: "Pulse", effect: &Pulse },
    Mode { label: "Orbit", effect: &Orbit },
    Mode { label: "Starburst", effect: &Starburst },
    Mode { label: "Ripple", effect: &Ripple },
];

// ─── 画面コントローラ ────────────────────────────────────────
// エフェクト画面の状態（選択中モードとアニメ位相）と振る舞いを持つ。
pub struct Effects {
    cursor: usize,
    phase: f64,
}

// キー処理の結果。画面遷移の判断は呼び出し元（App）に返す。
pub enum Nav {
    Stay, // この画面に留まる
    Exit, // プロンプトへ戻る
    Quit, // アプリ終了
}

impl Effects {
    pub fn new() -> Self {
        Self { cursor: 0, phase: 0.0 }
    }

    // 画面を開くときに呼ぶ。最初のエフェクトを先頭から再生する。
    pub fn reset(&mut self) {
        self.cursor = 0;
        self.phase = 0.0;
    }

    // 毎フレームのアニメ更新。
    pub fn tick(&mut self) {
        self.phase += PHASE_SPEED;
    }

    pub fn handle_key(&mut self, code: KeyCode) -> Nav {
        match code {
            KeyCode::Esc => Nav::Exit,
            KeyCode::Char('q') => Nav::Quit,
            KeyCode::Up | KeyCode::Char('k') => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                    self.phase = 0.0; // 選択先のエフェクトを最初から再生
                }
                Nav::Stay
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.cursor < MODES.len().saturating_sub(1) {
                    self.cursor += 1;
                    self.phase = 0.0;
                }
                Nav::Stay
            }
            _ => Nav::Stay,
        }
    }

    pub fn render(&self, frame: &mut Frame) {
        let area = frame.area();
        let w = area.width;
        let h = area.height;

        let menu_h = MODES.len() as u16 + 2;
        let menu_top = h.saturating_sub(menu_h) / 2;
        let center_row = menu_top + 2 + self.cursor as u16;

        // 背景にエフェクトを描く。
        if w > 0 && h > 0 {
            let mut canvas = Canvas::new(w as usize, h as usize);
            let ctx = self.make_ctx(w, h, center_row);
            MODES[self.cursor].effect.render(&mut canvas, &ctx);
            frame.render_widget(Paragraph::new(canvas.into_lines()), area);
        }

        // メニュー（エフェクトの上に重ねる）。
        self.render_menu(frame, Rect::new(0, menu_top, w, menu_h));

        let help = Paragraph::new("↑↓ / k j: 移動    Esc: 戻る    q: 終了")
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(help, Rect::new(0, h.saturating_sub(1), w, 1));
    }

    // 描画コンテキストを組み立てる（中心座標と最大半径）。
    fn make_ctx(&self, w: u16, h: u16, center_row: u16) -> Ctx {
        let cx = w as f64 / 2.0 - 0.5;
        let cy = center_row as f64;
        let max_dx = cx.max(w as f64 - 1.0 - cx);
        let max_dy = (cy.max(h as f64 - 1.0 - cy)) * CELL_ASPECT;
        let max_r = (max_dx * max_dx + max_dy * max_dy).sqrt().max(1.0);
        Ctx {
            phase: self.phase,
            cx,
            cy,
            max_r,
        }
    }

    fn render_menu(&self, frame: &mut Frame, area: Rect) {
        let mut lines: Vec<Line> = Vec::new();

        lines.push(Line::from(Span::styled(
            "エフェクトを選んでね",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from("")); // 空行

        for (i, mode) in MODES.iter().enumerate() {
            let selected = i == self.cursor;
            let (text, style) = if selected {
                (
                    format!("▶ {}", mode.label),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                (mode.label.to_string(), Style::default().fg(Color::Gray))
            };
            lines.push(Line::from(Span::styled(text, style)));
        }

        frame.render_widget(Paragraph::new(lines).alignment(Alignment::Center), area);
    }
}

// ─── Canvas: 文字セルの2次元バッファ ──────────────────────────
// 各エフェクトはここへ「文字＋色」を書き込む。最後に Vec<Line> へ変換して描画。
struct Canvas {
    w: usize,
    h: usize,
    cells: Vec<Option<(char, Color)>>, // None = 何も描かない（背景）
}

impl Canvas {
    fn new(w: usize, h: usize) -> Self {
        Self {
            w,
            h,
            cells: vec![None; w * h],
        }
    }

    // セルへ書き込む。座標は isize で受け、範囲外は無視（境界チェックを一箇所に集約）。
    fn set(&mut self, x: isize, y: isize, ch: char, color: Color) {
        if x < 0 || y < 0 {
            return;
        }
        let (x, y) = (x as usize, y as usize);
        if x >= self.w || y >= self.h {
            return;
        }
        self.cells[y * self.w + x] = Some((ch, color));
    }

    // Vec<Line> へ変換（1文字 = 1 Span で個別に色付け）。
    fn into_lines(self) -> Vec<Line<'static>> {
        let w = self.w;
        self.cells
            .chunks(w)
            .map(|row| {
                let spans: Vec<Span> = row
                    .iter()
                    .map(|cell| match cell {
                        Some((ch, color)) => {
                            Span::styled(ch.to_string(), Style::default().fg(*color))
                        }
                        None => Span::raw(" "),
                    })
                    .collect();
                Line::from(spans)
            })
            .collect()
    }
}

// ─── Effect トレイト ─────────────────────────────────────────
// 「選択中の文字を中心に、位相 phase で動く光」を Canvas へ描く責務。
// ctx に中心座標や最大半径などの描画コンテキストを渡す。
struct Ctx {
    phase: f64,
    cx: f64,    // 中心の x（横方向セル単位）
    cy: f64,    // 中心の y（縦方向セル単位 = 選択行）
    max_r: f64, // 中心から一番遠い角までの距離（横方向セル単位）
}

impl Ctx {
    // 極座標 (半径 r, 角度 a) を Canvas のセル座標へ。縦はセル比率で補正。
    fn polar(&self, r: f64, a: f64) -> (isize, isize) {
        let x = self.cx + r * a.cos();
        let y = self.cy + r * a.sin() / CELL_ASPECT;
        (x.round() as isize, y.round() as isize)
    }

    // セル (x, y) の中心からの距離（縦をセル比率で補正＝見た目の円距離）。
    fn radius_at(&self, x: usize, y: usize) -> f64 {
        let dx = x as f64 - self.cx;
        let dy = (y as f64 - self.cy) * CELL_ASPECT;
        (dx * dx + dy * dy).sqrt()
    }
}

trait Effect {
    fn render(&self, canvas: &mut Canvas, ctx: &Ctx);
}

// ── Pulse: 中心でゆっくり明滅する光（呼吸） ──────────────────
struct Pulse;
impl Effect for Pulse {
    fn render(&self, canvas: &mut Canvas, ctx: &Ctx) {
        const RADIUS: f64 = 16.0;
        // ゆっくりした明滅。完全には消えないよう下限を残す（0.35〜1.0）。
        let breath = 0.675 + 0.325 * (ctx.phase * 0.45).sin();
        for y in 0..canvas.h {
            for x in 0..canvas.w {
                let r = ctx.radius_at(x, y);
                if r > RADIUS {
                    continue;
                }
                // 中心ほど明るい円形グロー × 明滅。
                let lvl = ((1.0 - r / RADIUS) * breath).clamp(0.0, 1.0);
                if lvl > 0.12 {
                    canvas.set(x as isize, y as isize, glyph(lvl), shimmer_color(lvl));
                }
            }
        }
    }
}

// ── Orbit: 中心を周回する光（コメット） ─────────────────────
struct Orbit;
impl Effect for Orbit {
    fn render(&self, canvas: &mut Canvas, ctx: &Ctx) {
        const RADIUS: f64 = 11.0;
        const DOTS: usize = 3; // 周回する光の数
        const TRAIL: usize = 10; // 尾の長さ
        for k in 0..DOTS {
            // 各ドットは等間隔の角度に配置し、位相で回転させる。
            let base = ctx.phase * 0.8 + k as f64 * std::f64::consts::TAU / DOTS as f64;
            for t in 0..TRAIL {
                // 尾: 少し過去の角度ほど暗くする。
                let a = base - t as f64 * 0.13;
                let lvl = 1.0 - t as f64 / TRAIL as f64;
                let (x, y) = ctx.polar(RADIUS, a);
                canvas.set(x, y, glyph(lvl), shimmer_color(lvl));
            }
        }
    }
}

// ── Starburst: 放射状に脈打つ光線（スターバースト） ──────────
struct Starburst;
impl Effect for Starburst {
    fn render(&self, canvas: &mut Canvas, ctx: &Ctx) {
        const SPOKES: usize = 16;
        let max_len = (ctx.max_r * 0.9).max(1.0);
        for s in 0..SPOKES {
            // スポークはゆっくり回転する。
            let a = s as f64 * std::f64::consts::TAU / SPOKES as f64 + ctx.phase * 0.12;
            let mut r = 1.0;
            while r < max_len {
                // 光が外へ脈打って流れる（sin の山を描く）。
                let wave = (r * 0.5 - ctx.phase * 1.3).sin();
                let atten = (1.0 - r / max_len).clamp(0.0, 1.0);
                let lvl = (wave * atten).clamp(0.0, 1.0);
                if lvl > 0.20 {
                    let (x, y) = ctx.polar(r, a);
                    canvas.set(x, y, glyph(lvl), shimmer_color(lvl));
                }
                r += 1.0;
            }
        }
    }
}

// ── Ripple: 中心から伝播する同心円の波紋 ─────────────────────
struct Ripple;
impl Effect for Ripple {
    fn render(&self, canvas: &mut Canvas, ctx: &Ctx) {
        const FREQ: f64 = 0.42; // 円の間隔
        const EDGE: f64 = 8.0; // 波面の縁をフェードインさせる幅（セル数）
        // 波面（先端）の半径。位相が増えると外へ広がる。
        // リングの山と同じ速さ（phase/FREQ）で進めると、山が波面の内側に並ぶ。
        let front = ctx.phase / FREQ;
        for y in 0..canvas.h {
            for x in 0..canvas.w {
                let r = ctx.radius_at(x, y);
                // 波面がまだ届いていない外側は静かな水面（背景）のまま。
                // これで「中心から徐々に半径が大きくなって伝播する」見た目になる。
                if r > front {
                    continue;
                }
                let wave = (r * FREQ - ctx.phase).sin();
                let atten = (1.0 - r / ctx.max_r).clamp(0.0, 1.0);
                // 波が届いたばかりの先端を弱→強で柔らかく立ち上げる。
                let edge = ((front - r) / EDGE).clamp(0.0, 1.0);
                let intensity = wave * atten * edge; // -1.0〜1.0
                // 波の山（crest）だけを描く。閾値以下は水面（背景）。
                if intensity > 0.30 {
                    let lvl = ((intensity - 0.30) / 0.70).clamp(0.0, 1.0);
                    canvas.set(x as isize, y as isize, glyph(lvl), shimmer_color(lvl));
                }
            }
        }
    }
}

// ─── 共通ヘルパ ──────────────────────────────────────────────
// 強さ lvl(0.0〜1.0) を濃淡文字に対応させる（全エフェクト共通）。
fn glyph(lvl: f64) -> char {
    if lvl > 0.75 {
        '█'
    } else if lvl > 0.50 {
        '▓'
    } else if lvl > 0.25 {
        '▒'
    } else {
        '░'
    }
}

// 強さ lvl(0.0〜1.0) を、暗い青→明るいシアン白へ補間した RGB 色にする。
// Color::Rgb で滑らかなグラデーションが作れる（SuperThink 風の青いシマー）。
fn shimmer_color(lvl: f64) -> Color {
    let lerp = |a: f64, b: f64| (a + (b - a) * lvl) as u8;
    Color::Rgb(
        lerp(40.0, 200.0),  // R
        lerp(70.0, 240.0),  // G
        lerp(130.0, 255.0), // B
    )
}
