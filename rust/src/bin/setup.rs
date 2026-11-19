#![windows_subsystem = "windows"]
fn main() {
    if yami::install::uninstall_command() {
        return;
    }
    yami::ui::run(Some(include_bytes!("../../target/release/Yami.exe")));
}
