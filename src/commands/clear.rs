// clear: 出力履歴を消去する。
use super::Command;
use crate::App;

pub struct Cmd;

impl Command for Cmd {
    fn name(&self) -> &'static str {
        "clear"
    }

    fn help(&self) -> &'static str {
        "出力を消去"
    }

    fn run(&self, app: &mut App) {
        app.clear_output();
    }
}
