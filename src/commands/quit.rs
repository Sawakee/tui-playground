// quit: アプリを終了する。
use super::Command;
use crate::App;

pub struct Cmd;

impl Command for Cmd {
    fn name(&self) -> &'static str {
        "quit"
    }

    fn help(&self) -> &'static str {
        "終了（Esc でも可）"
    }

    fn run(&self, app: &mut App) {
        app.quitting = true;
    }
}
