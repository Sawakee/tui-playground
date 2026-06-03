# tui-playground 🌊

ターミナル上で動く、小さな **エフェクト集 TUI**。
カーソルで4つのエフェクトを選ぶと、選んだ文字を中心に光のアニメーションが表示される。

Go 経験者が Rust を学ぶための練習場（playground）として、コードには
Go との対比を交えた日本語コメントを多めに入れている。

## エフェクト

| ラベル | 効果 |
|---|---|
| **Pulse** | 中心でゆっくり明滅する呼吸グロー |
| **Orbit** | 中心を周回する3つのコメット（尾つき） |
| **Starburst** | 放射状に脈打つ光線（回転するスターバースト） |
| **Ripple** | 中心から伝播する同心円の波紋 |

いずれも選択中の文字を中心に描画され、`Pulse → Ripple` の順に強度がエスカレートする。
カーソルを動かすたびに位相が 0 に戻り、エフェクトが最初から再生される。

## 操作

| キー | 動作 |
|---|---|
| `↑` / `k` | 上のエフェクトへ |
| `↓` / `j` | 下のエフェクトへ |
| `q` / `Esc` / `Ctrl-C` | 終了 |

## 実行

```sh
cargo run
```

依存は [ratatui](https://ratatui.rs/) 0.29 + [crossterm](https://docs.rs/crossterm) 0.28、Rust edition 2024。

## 設計

エフェクトはプラガブルな構成になっている。

- **`Effect` トレイト** … `render(&self, canvas, ctx)` でキャンバスへ描く責務
- **`Canvas`** … 文字セルの2次元バッファ。`set(x, y, ch, color)` で書き込み、最後に一括描画
- **`Ctx`** … 中心座標・最大半径＋極座標変換 `polar()` / 距離計算 `radius_at()`
- **`MODES` テーブル** … ラベルと `&dyn Effect` の対応表

### 新しいエフェクトを追加する

1. `struct Foo;` を作り `impl Effect for Foo` に描画を書く
2. `static FOO: Foo = Foo;` を足す
3. `MODES` に `Mode { label: "Foo", effect: &FOO }` を1行追加

メニュー項目数・選択行・中心座標・ヘルプはすべて `MODES` から自動算出されるので、
他のコードを変更する必要はない。各エフェクトの速さ・半径・密度は、各 `impl` 内の
定数（`RADIUS` / `SPOKES` / `FREQ` など）や共通の `PHASE_SPEED` で調整できる。
