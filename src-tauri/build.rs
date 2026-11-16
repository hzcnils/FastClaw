fn main() {
    // Ensure sqlx::migrate! sees newly added/renamed migration files.
    println!("cargo:rerun-if-changed=migrations");

    // FastClaw Tauri build script
    // This is needed for tauri-build to work properly
    tauri_build::build()
}
