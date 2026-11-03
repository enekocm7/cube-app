#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    cube_updater::cleanup();
    cube_ui::run();
}
