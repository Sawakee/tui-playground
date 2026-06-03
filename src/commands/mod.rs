// プロンプトのコマンド群。「1コマンド1ファイル」でこのディレクトリに置く。
// 新しいコマンドを足す手順:
//   1. commands/foo.rs を作り `pub struct Cmd;` に `impl Command` を書く
//   2. 下に `mod foo;` と REGISTRY への `&foo::Cmd` を足す
// これだけで補完・ヘルプ・実行のすべてに反映される（単一の真実）。

use crate::App;

mod clear;
mod effect;
mod help;
mod quit;

// コマンドのインターフェース。エフェクト側の Effect トレイトと同じ発想で、
// 各コマンドを差し替え可能な部品にする。
pub trait Command {
    fn name(&self) -> &'static str; // 入力名・補完キー
    fn help(&self) -> &'static str; // help コマンドに出す説明
    fn run(&self, app: &mut App); // 実行本体（App を書き換える）
}

// 登録済みコマンド一覧。並び順が補完・ヘルプの表示順になる。
// 各コマンドは状態を持たないゼロサイズ型なので、参照を直接並べればよい。
pub const REGISTRY: &[&dyn Command] = &[&effect::Cmd, &help::Cmd, &clear::Cmd, &quit::Cmd];
