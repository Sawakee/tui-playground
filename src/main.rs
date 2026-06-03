// ウェーブ TUI 🌊
// 4つのエフェクトをカーソルで選ぶメニュー。
// 選択中のモードごとに、選択した文字を中心とした別々の光のエフェクトを表示する。
//
// 設計はスケーラブル: 効果は Effect トレイトで表し、共通の Canvas へ描く。
// 新しいモードを足したいときは「Effect を実装した型」を作り、
// 下の MODES テーブルに1行追加するだけでよい。

use std::{
    io,
    process::Command,
    time::{Duration, Instant},
};

use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Wrap},
    Frame,
};

// ─── 定数 ────────────────────────────────────────────────────
const FPS: u64 = 30;
const FRAME_DURATION: Duration = Duration::from_millis(1000 / FPS); // ≈ 33ms

// 位相は毎フレーム加算される共通の「時間」。各エフェクトがこれを使って動く。
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

static PULSE: Pulse = Pulse;
static ORBIT: Orbit = Orbit;
static STARBURST: Starburst = Starburst;
static RIPPLE: Ripple = Ripple;

const MODES: &[Mode] = &[
    Mode { label: "Pulse", effect: &PULSE },
    Mode { label: "Orbit", effect: &ORBIT },
    Mode { label: "Starburst", effect: &STARBURST },
    Mode { label: "Ripple", effect: &RIPPLE },
];

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

// ─── 画面の状態 ──────────────────────────────────────────────
// Claude Code のように、まずプロンプトで文字を入力する。
// "effect" と打つとエフェクト画面へ、"!…" でシェル実行。
enum Screen {
    Prompt,
    Effects,
}

// シェルモードを表す入力の接頭辞。
const SHELL_PREFIX: char = '!';

// 出力履歴の保持上限（古い行は捨てる）。
const MAX_OUTPUT: usize = 500;

// 補完ポップアップに一度に出す候補の最大数。
const MAX_SUGGESTIONS: usize = 6;

// プロンプトのコマンド定義。MODES と同じく、ここに1行足すだけで
// 補完・ヘルプ・実行のすべてに反映される（コマンドの単一の真実）。
struct CmdSpec {
    name: &'static str,
    help: &'static str,
    run: fn(&mut App),
}

const COMMANDS: &[CmdSpec] = &[
    CmdSpec { name: "effect", help: "エフェクト画面を開く", run: App::cmd_effect },
    CmdSpec { name: "help", help: "コマンド一覧を表示", run: App::cmd_help },
    CmdSpec { name: "clear", help: "出力を消去", run: App::cmd_clear },
    CmdSpec { name: "quit", help: "終了（Esc でも可）", run: App::cmd_quit },
];

// ─── App 構造体 ───────────────────────────────────────────────
struct App {
    screen: Screen,
    input: String,        // プロンプトの入力中文字列
    output: Vec<String>,  // 画面に出す履歴（投稿コマンドやシェル出力）
    sugg_idx: usize,      // 補完候補のうち選択中のインデックス
    cursor: usize,        // エフェクト画面で選択中のモード
    phase: f64,           // 共通の位相（時間）。アニメとカーソル点滅に使う
    quitting: bool,
}

impl App {
    fn new() -> Self {
        Self {
            screen: Screen::Prompt,
            input: String::new(),
            // 起動時の出力は空。使い方は help コマンドで見られる。
            output: Vec::new(),
            sugg_idx: 0,
            cursor: 0,
            phase: 0.0,
            quitting: false,
        }
    }

    fn update(&mut self) {
        self.phase += PHASE_SPEED;
    }

    // ─── キー処理（画面ごとに振り分け）────────────────────────
    fn handle_key(&mut self, code: KeyCode, modifiers: KeyModifiers) {
        // Ctrl-C はどの画面でも終了。
        if code == KeyCode::Char('c') && modifiers.contains(KeyModifiers::CONTROL) {
            self.quitting = true;
            return;
        }
        match self.screen {
            Screen::Prompt => self.handle_prompt_key(code),
            Screen::Effects => self.handle_effects_key(code),
        }
    }

    fn handle_prompt_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Esc => self.quitting = true,
            // Enter: 補完候補が出ていればまず選択中の候補を入力に採用し、
            // 候補がなければ（＝確定済み or シェル）実行する。
            KeyCode::Enter => {
                if !self.accept_suggestion() {
                    self.submit();
                }
            }
            KeyCode::Backspace => {
                self.input.pop();
                self.sugg_idx = 0;
            }
            // Tab: 補完候補を入力に採用する。
            KeyCode::Tab => {
                self.accept_suggestion();
            }
            // 上下で補完候補を選ぶ。
            KeyCode::Up => {
                if self.sugg_idx > 0 {
                    self.sugg_idx -= 1;
                }
            }
            KeyCode::Down => {
                let n = self.matches().len();
                if n > 0 && self.sugg_idx + 1 < n {
                    self.sugg_idx += 1;
                }
            }
            // 印字可能な文字を入力へ（スペースも可＝"!ls -la" のため）。
            KeyCode::Char(c) => {
                self.input.push(c);
                self.sugg_idx = 0;
            }
            _ => {}
        }
    }

    fn handle_effects_key(&mut self, code: KeyCode) {
        match code {
            // プロンプトへ戻る。
            KeyCode::Esc => {
                self.screen = Screen::Prompt;
            }
            KeyCode::Char('q') => self.quitting = true,
            KeyCode::Up | KeyCode::Char('k') => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                    self.phase = 0.0; // 選択先のエフェクトを最初から再生
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.cursor < MODES.len().saturating_sub(1) {
                    self.cursor += 1;
                    self.phase = 0.0;
                }
            }
            _ => {}
        }
    }

    // いまの入力にマッチする補完候補。
    // "!" 始まりはシェルモードなのでコマンド補完はしない。
    // 空入力のときは全コマンドを候補に出す（デフォルトで一覧が見える）。
    fn matches(&self) -> Vec<&'static str> {
        if self.is_shell_mode() {
            return Vec::new();
        }
        let inp = self.input.trim();
        if inp.is_empty() {
            return COMMANDS.iter().map(|c| c.name).collect();
        }
        let ms: Vec<&'static str> = COMMANDS
            .iter()
            .map(|c| c.name)
            .filter(|name| name.starts_with(inp))
            .collect();
        // 入力とぴったり一致する1件だけなら、候補表示は不要。
        if ms.len() == 1 && ms[0] == inp {
            return Vec::new();
        }
        ms
    }

    // SHELL_PREFIX で始まっていればシェルモード。
    fn is_shell_mode(&self) -> bool {
        self.input.starts_with(SHELL_PREFIX)
    }

    // 選択中の補完候補を入力欄に採用する。採用したら true。
    // 候補が無い（確定済み or シェルモード）なら false。
    // sugg_idx は入力編集のたびに 0 に戻し、Down では候補数内に収めているので、
    // 常に範囲内。get が None を返すのは候補が無いときだけ。
    fn accept_suggestion(&mut self) -> bool {
        if let Some(&cmd) = self.matches().get(self.sugg_idx) {
            self.input = cmd.to_string();
            self.sugg_idx = 0;
            true
        } else {
            false
        }
    }

    // 入力を確定して実行する。
    fn submit(&mut self) {
        let line = self.input.trim().to_string();
        self.input.clear();
        self.sugg_idx = 0;
        if line.is_empty() {
            return;
        }
        self.push_output(format!("> {line}"));

        if let Some(cmd) = line.strip_prefix(SHELL_PREFIX) {
            self.run_shell(cmd.trim());
            return;
        }
        // コマンド表から探して実行。無ければ unknown。
        match COMMANDS.iter().find(|c| c.name == line) {
            Some(cmd) => (cmd.run)(self),
            None => self.push_output(format!("unknown command: {line}  ('help' でコマンド一覧)")),
        }
    }

    // ─── コマンドの実体（COMMANDS テーブルから呼ばれる）──────────
    fn cmd_effect(&mut self) {
        self.screen = Screen::Effects;
        self.cursor = 0;
        self.phase = 0.0;
    }

    fn cmd_clear(&mut self) {
        self.output.clear();
    }

    fn cmd_quit(&mut self) {
        self.quitting = true;
    }

    fn cmd_help(&mut self) {
        for c in COMMANDS {
            self.push_output(format!("  {:<8}{}", c.name, c.help));
        }
        self.push_output(format!("  {:<8}{}", "!<cmd>", "シェルを実行（例: !ls -la）"));
        self.push_output(format!("  {:<8}{}", "Tab", "補完 / ↑↓ で候補選択"));
    }

    // シェルを実行して標準出力・標準エラーを履歴へ取り込む。
    fn run_shell(&mut self, cmd: &str) {
        if cmd.is_empty() {
            return;
        }
        match Command::new("sh").arg("-c").arg(cmd).output() {
            Ok(out) => {
                for line in String::from_utf8_lossy(&out.stdout).lines() {
                    self.push_output(line.to_string());
                }
                for line in String::from_utf8_lossy(&out.stderr).lines() {
                    self.push_output(line.to_string());
                }
                if !out.status.success() {
                    self.push_output(format!("[exit: {}]", out.status.code().unwrap_or(-1)));
                }
            }
            Err(e) => self.push_output(format!("[error] {e}")),
        }
    }

    // 履歴へ1行追加し、上限を超えたら古い行を捨てる。
    fn push_output(&mut self, line: String) {
        self.output.push(line);
        let overflow = self.output.len().saturating_sub(MAX_OUTPUT);
        if overflow > 0 {
            self.output.drain(0..overflow);
        }
    }

    // ─── 描画（画面ごとに振り分け）────────────────────────────
    fn render(&mut self, frame: &mut Frame) {
        match self.screen {
            Screen::Prompt => self.render_prompt(frame),
            Screen::Effects => self.render_effects(frame),
        }
    }

    // プロンプト画面: 上に履歴、下に入力行、入力の上に補完候補。
    fn render_prompt(&self, frame: &mut Frame) {
        let area = frame.area();
        let w = area.width;
        let h = area.height;
        if w == 0 || h < 2 {
            return;
        }

        // 入力行＝下から2行目、ヘルプ＝最下行、それより上が履歴。
        let input_y = h - 2;
        let body_h = input_y; // 0..input_y が履歴領域

        // 履歴（末尾を優先して表示）
        let start = self.output.len().saturating_sub(body_h as usize);
        let text = self.output[start..].join("\n");
        frame.render_widget(
            Paragraph::new(text)
                .wrap(Wrap { trim: false })
                .style(Style::default().fg(Color::Gray)),
            Rect::new(0, 0, w, body_h),
        );

        // 入力行（点滅カーソル付き）。シェルモードでは見た目を変える。
        let shell = self.is_shell_mode();
        let blink = (self.phase * 0.6).sin() > -0.2; // ほぼ点灯、ときどき消える
        let caret = if blink { "█" } else { " " };
        let accent = if shell { Color::Yellow } else { Color::Cyan };
        let mut spans: Vec<Span> = Vec::new();
        if shell {
            // 黄色いバッジでシェルモードであることを明示。
            spans.push(Span::styled(
                " SHELL ",
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::raw(" "));
            // 接頭辞を除いた中身を黄色で表示。
            let body = self.input.strip_prefix(SHELL_PREFIX).unwrap_or(&self.input);
            spans.push(Span::styled(body.to_string(), Style::default().fg(Color::Yellow)));
        } else {
            spans.push(Span::styled(
                "❯ ",
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::styled(self.input.clone(), Style::default().fg(Color::White)));
        }
        spans.push(Span::styled(caret, Style::default().fg(accent)));
        frame.render_widget(Paragraph::new(Line::from(spans)), Rect::new(0, input_y, w, 1));

        // ヘルプ行
        frame.render_widget(
            Paragraph::new("Enter 採用/実行   Tab 補完   ↑↓ 候補   !<cmd> シェル   Esc 終了")
                .style(Style::default().fg(Color::DarkGray)),
            Rect::new(0, h - 1, w, 1),
        );

        // シェルモードのときは、補完の代わりに説明を入力行の真上に出す。
        if shell {
            let hint = Line::from(Span::styled(
                " sh -c で実行します（例: !ls -la） ",
                Style::default().fg(Color::Yellow).bg(Color::Rgb(50, 45, 12)),
            ));
            let hw = (hint.width() as u16).min(w.saturating_sub(2)).max(1);
            frame.render_widget(
                Paragraph::new(hint),
                Rect::new(2, input_y.saturating_sub(1), hw, 1),
            );
            return;
        }

        // 補完候補ポップアップ（入力行の真上に重ねる）
        let matches = self.matches();
        if !matches.is_empty() {
            let n = matches.len().min(MAX_SUGGESTIONS) as u16;
            let top = input_y.saturating_sub(n);
            let pop_w = matches
                .iter()
                .map(|m| m.len())
                .max()
                .unwrap_or(0) as u16
                + 4;
            let pop_w = pop_w.min(w.saturating_sub(2)).max(1);

            let lines: Vec<Line> = matches
                .iter()
                .take(n as usize)
                .enumerate()
                .map(|(i, m)| {
                    let selected = i == self.sugg_idx;
                    // 幅いっぱいに背景色を敷くため空白で埋める。
                    let label = format!(" {:<width$}", m, width = (pop_w as usize).saturating_sub(1));
                    let style = if selected {
                        Style::default()
                            .fg(Color::White)
                            .bg(Color::Rgb(40, 70, 130))
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::Gray).bg(Color::Rgb(22, 27, 38))
                    };
                    Line::from(Span::styled(label, style))
                })
                .collect();
            frame.render_widget(Paragraph::new(lines), Rect::new(2, top, pop_w, n));
        }
    }

    // エフェクト画面（旧メニュー）。
    fn render_effects(&mut self, frame: &mut Frame) {
        let area = frame.area();
        let w = area.width;
        let h = area.height;

        let menu_h = MODES.len() as u16 + 2;
        let menu_top = h.saturating_sub(menu_h) / 2;
        let center_row = menu_top + 2 + self.cursor as u16;

        if w > 0 && h > 0 {
            let mut canvas = Canvas::new(w as usize, h as usize);
            let ctx = self.make_ctx(w, h, center_row);
            MODES[self.cursor].effect.render(&mut canvas, &ctx);
            frame.render_widget(Paragraph::new(canvas.into_lines()), area);
        }

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

// ─── main ────────────────────────────────────────────────────
fn main() -> io::Result<()> {
    let mut terminal = ratatui::init();
    let mut app = App::new();
    let mut last_frame = Instant::now();

    loop {
        if last_frame.elapsed() >= FRAME_DURATION {
            app.update();
            terminal.draw(|frame| app.render(frame))?;
            last_frame = Instant::now();
        }

        let timeout = FRAME_DURATION.saturating_sub(last_frame.elapsed());
        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == crossterm::event::KeyEventKind::Press {
                    app.handle_key(key.code, key.modifiers);
                }
            }
        }

        if app.quitting {
            break;
        }
    }

    ratatui::restore();
    println!("またね！🌊");
    Ok(())
}
