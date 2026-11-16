const COMMANDS: &[&str] = &["execute"];
fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}
