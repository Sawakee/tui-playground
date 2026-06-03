// effect: エフェクト画面を開く。
use super::Command;
use crate::App;

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
