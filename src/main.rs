#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod i18n;
mod platform;
mod speech;
mod ui;

fn main() {
    ui::run();
}
