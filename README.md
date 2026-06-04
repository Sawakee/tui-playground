# tui-playground 🌊

ターミナル上で動く、小さな **プロンプト型 TUI**。
Claude Code のように、まずプロンプトで文字を入力する。

- `effect` … エフェクト画面を開く（下記の光アニメーション）
- `!<cmd>` … シェルコマンドを実行（例: `!ls -la`）。標準出力・標準エラー・終了コードを表示
- `help` / `clear` / `quit` … ヘルプ表示 / 出力消去 / 終了
- **Tab** で補完、**↑↓** で候補選択

Go 経験者が Rust を学ぶための練習場（playground）として、コードには
Go との対比を交えた日本語コメントを多めに入れている。

## プロンプト操作

| キー | 動作 |
|---|---|
| 文字入力 | プロンプトに入力（`!` で始めるとシェル） |
| `Tab` | 補完候補を採用 |
| `↑` / `↓` | 補完候補を選択 |
| `Enter` | 実行 |
| `Esc` / `Ctrl-C` | 終了 |

補完候補は `COMMANDS` テーブルに1行足すだけで増やせる。

## エフェクト

| ラベル | 効果 |
|---|---|
| **Pulse** | 中心でゆっくり明滅する呼吸グロー |
| **Orbit** | 中心を周回する3つのコメット（尾つき） |
| **Starburst** | 放射状に脈打つ光線（回転するスターバースト） |
| **Ripple** | 中心から伝播する同心円の波紋 |

いずれも選択中の文字を中心に描画され、`Pulse → Ripple` の順に強度がエスカレートする。
カーソルを動かすたびに位相が 0 に戻り、エフェクトが最初から再生される。

## エフェクト画面の操作

| キー | 動作 |
|---|---|
| `↑` / `k` | 上のエフェクトへ |
| `↓` / `j` | 下のエフェクトへ |
| `Esc` | プロンプトへ戻る |
| `q` / `Ctrl-C` | 終了 |

## 実行

```sh
cargo run
```

依存は [ratatui](https://ratatui.rs/) 0.29 + [crossterm](https://docs.rs/crossterm) 0.28、Rust edition 2024。

## 全体構成

機能ごとにモジュールが分かれており、`main.rs` は外枠だけを持つ。

```
src/
  main.rs            ← App（プロンプト）・画面遷移・main ループ
  commands/          ← プロンプトのコマンド（1コマンド1ファイル）
    effect.rs        ← effect コマンド ＋ エフェクト機能の全部（エンジン + 画面）
    help.rs / clear.rs / quit.rs / mod.rs
```

`main.rs` は具体的な画面を知らない。汎用の **`Screen` トレイト**
（`update()` / `handle_key() -> Transition` / `render()`）だけを定義し、
`Option<Box<dyn Screen>>` として「いま開いている画面」を駆動する。
画面を開きたいコマンドが `app.open_screen(Box::new(...))` で `Box<dyn Screen>` を
渡すだけでよく、main の変更は不要（＝画面を増やしてもスケーラブル）。

## エフェクト（`src/commands/effect.rs`）

effect コマンドとエフェクト機能は同じファイルにまとまっている。

- **`Cmd`** … `effect` コマンド本体（`app.open_screen(Box::new(Effects::new()))`）
- **`Effects`** … エフェクト画面。`impl Screen` で `update` / `handle_key` / `render` を提供
  （状態は選択中モードと位相）
- **`Effect` トレイト** … `render(&self, canvas, ctx)` でキャンバスへ描く責務
- **`Canvas`** … 文字セルの2次元バッファ。`set(x, y, ch, color)` で書き込み、最後に一括描画
- **`Ctx`** … 中心座標・最大半径＋極座標変換 `polar()` / 距離計算 `radius_at()`
- **`MODES` テーブル** … ラベルと `&dyn Effect` の対応表

### 新しいエフェクトを追加する（すべて `commands/effect.rs` 内で完結）

1. `struct Foo;` を作り `impl Effect for Foo` に描画を書く
2. `MODES` に `Mode { label: "Foo", effect: &Foo }` を1行追加

メニュー項目数・選択行・中心座標・ヘルプはすべて `MODES` から自動算出される。
各エフェクトの速さ・半径・密度は、各 `impl` 内の定数（`RADIUS` / `SPOKES` /
`FREQ` など）や `PHASE_SPEED` で調整できる。

## コマンド（1コマンド1ファイル）

プロンプトのコマンドは `src/commands/` 以下に**1コマンド1ファイル**で置く。

```
src/commands/
  mod.rs      ← Command トレイト + REGISTRY
  effect.rs   ← pub struct Cmd; impl Command
  help.rs
  clear.rs
  quit.rs
```

- **`Command` トレイト** … `name()` / `help()` / `run(&self, app)` の3つ（エフェクトの `Effect` トレイトと同じ発想）
- **`REGISTRY`** … `&[&dyn Command]`。補完・ヘルプ・実行すべての単一の真実

### 新しいコマンドを追加する

1. `src/commands/foo.rs` を作り、`pub struct Cmd;` に `impl Command` を書く
2. `mod.rs` に `mod foo;` と `REGISTRY` へ `&foo::Cmd` を1行追加

`run()` には `&mut App` が渡る。画面遷移・出力・終了などは App の操作 API
（`open_effects()` / `clear_output()` / `quit()` / `push_output()`）を呼ぶ。
補完候補も `help` の一覧も `REGISTRY` から自動生成されるため、追加漏れでズレない。
