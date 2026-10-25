// No console window next to the GUI on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    osu_collect_gui::run();
}
