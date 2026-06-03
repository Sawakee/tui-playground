// ウェーブ TUI 🌊
// Claude Code のように、まずプロンプトで文字を入力する。
//   - "effect" でエフェクト画面（commands/ と effects モジュール）
//   - "!<cmd>" でシェル実行
// main.rs はアプリの外枠（プロンプト・画面遷移・メインループ）だけを持ち、
// 機能の実体は commands/ と effects に分かれている。

use std::{
    io,
    process,
    time::{Duration, Instant},
};

use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Wrap},
    Frame,
};

mod commands;
// エフェクト機能は commands/effect.rs（effect コマンドと同じファイル）にある。
use commands::effect;

// ─── 定数 ────────────────────────────────────────────────────
const FPS: u64 = 30;
const FRAME_DURATION: Duration = Duration::from_millis(1000 / FPS); // ≈ 33ms

// プロンプトのカーソル点滅に使う位相の進む速さ。
const PHASE_SPEED: f64 = 0.30;

// シェルモードを表す入力の接頭辞。
const SHELL_PREFIX: char = '!';

// 出力履歴の保持上限（古い行は捨てる）。
const MAX_OUTPUT: usize = 500;

// 補完ポップアップに一度に出す候補の最大数。
const MAX_SUGGESTIONS: usize = 6;

// コマンドは commands/ 以下に「1コマンド1ファイル」で定義する。
// commands::REGISTRY が登録済み一覧（補完・ヘルプ・実行の単一の真実）。

// ─── 画面の状態 ──────────────────────────────────────────────
enum Screen {
    Prompt,
    Effects,
}

// ─── App 構造体 ───────────────────────────────────────────────
struct App {
    screen: Screen,
    input: String,            // プロンプトの入力中文字列
    output: Vec<String>,      // 画面に出す履歴（投稿コマンドやシェル出力）
    sugg_idx: usize,          // 補完候補のうち選択中のインデックス
    phase: f64,              // プロンプトのカーソル点滅用の位相
    effects: effect::Effects, // エフェクト画面（状態・描画は commands/effect.rs）
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
            phase: 0.0,
            effects: effect::Effects::new(),
            quitting: false,
        }
    }

    fn update(&mut self) {
        self.phase += PHASE_SPEED; // カーソル点滅
        if let Screen::Effects = self.screen {
            self.effects.tick(); // 表示中だけアニメを進める
        }
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
            // エフェクト画面のキー処理はモジュールに委譲し、遷移だけ受け取る。
            Screen::Effects => match self.effects.handle_key(code) {
                effect::Nav::Exit => self.screen = Screen::Prompt,
                effect::Nav::Quit => self.quitting = true,
                effect::Nav::Stay => {}
            },
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

    // いまの入力にマッチする補完候補。
    // "!" 始まりはシェルモードなのでコマンド補完はしない。
    // 空入力のときは全コマンドを候補に出す（デフォルトで一覧が見える）。
    fn matches(&self) -> Vec<&'static str> {
        if self.is_shell_mode() {
            return Vec::new();
        }
        let inp = self.input.trim();
        if inp.is_empty() {
            return commands::REGISTRY.iter().map(|c| c.name()).collect();
        }
        let ms: Vec<&'static str> = commands::REGISTRY
            .iter()
            .map(|c| c.name())
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
        // レジストリから探して実行。無ければ unknown。
        match commands::REGISTRY.iter().find(|c| c.name() == line) {
            Some(cmd) => cmd.run(self),
            None => self.push_output(format!("unknown command: {line}  ('help' でコマンド一覧)")),
        }
    }

    // ─── コマンド向けの操作 API（commands/* から呼ばれる）───────
    // コマンドは「何をしたいか」だけを呼び、フィールドの動かし方は App が持つ。

    // エフェクト画面を開く（最初から再生されるよう effects 側を初期化）。
    fn open_effects(&mut self) {
        self.effects.reset();
        self.screen = Screen::Effects;
    }

    fn clear_output(&mut self) {
        self.output.clear();
    }

    fn quit(&mut self) {
        self.quitting = true;
    }

    // シェルを実行して標準出力・標準エラーを履歴へ取り込む。
    fn run_shell(&mut self, cmd: &str) {
        if cmd.is_empty() {
            return;
        }
        match process::Command::new("sh").arg("-c").arg(cmd).output() {
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
            Screen::Effects => self.effects.render(frame),
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
