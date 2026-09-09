fn main() -> anvil_ui::Result<()> {
    env_logger::init();
    prefer_x11_on_wsl();
    anvil_ui::run()
}

/// WSLg advertises a Wayland socket that winit cannot always connect to
/// (connection reset). Its X11 server is reliable, so on WSL we drop the
/// Wayland variable and let winit fall back to X11. Set ANVIL_WAYLAND=1 to
/// keep Wayland.
fn prefer_x11_on_wsl() {
    let on_wsl = std::env::var_os("WSL_DISTRO_NAME").is_some()
        || std::fs::read_to_string("/proc/version").map(|v| v.to_lowercase().contains("microsoft")).unwrap_or(false);
    if on_wsl && std::env::var_os("ANVIL_WAYLAND").is_none() {
        std::env::remove_var("WAYLAND_DISPLAY");
    }
}
