// effect: エフェクト画面を開く。
use super::Command;
use crate::{App, Screen};

pub struct Cmd;

impl Command for Cmd {
    fn name(&self) -> &'static str {
        "effect"
    }

    fn help(&self) -> &'static str {
        "エフェクト画面を開く"
    }

    fn run(&self, app: &mut App) {
        app.screen = Screen::Effects;
        app.cursor = 0;
        app.phase = 0.0; // 選択中のエフェクトを最初から再生
    }
}
