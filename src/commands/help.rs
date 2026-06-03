// help: 登録済みコマンドの一覧を表示する。
// REGISTRY を走査するので、コマンドを足せば自動でここにも載る。
use super::{Command, REGISTRY};
use crate::App;

pub struct Cmd;

impl Command for Cmd {
    fn name(&self) -> &'static str {
        "help"
    }

    fn help(&self) -> &'static str {
        "コマンド一覧を表示"
    }

    fn run(&self, app: &mut App) {
        let row = |name: &str, help: &str| format!("  {name:<8}{help}");
        for c in REGISTRY {
            app.push_output(row(c.name(), c.help()));
        }
        // コマンドではない使い方も併記。
        app.push_output(row("!<cmd>", "シェルを実行（例: !ls -la）"));
        app.push_output(row("Tab", "補完 / ↑↓ で候補選択"));
    }
}
