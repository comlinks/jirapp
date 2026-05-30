// Windows のリリースビルドではコンソールウィンドウを出さない。
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    jirapp_lib::run()
}
