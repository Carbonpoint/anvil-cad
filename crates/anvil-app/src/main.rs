fn main() -> anvil_ui::Result<()> {
    env_logger::init();
    match anvil_ui::run() {
        Ok(()) => Ok(()),
        Err(e) if std::env::var_os("WAYLAND_DISPLAY").is_some() => {
            // WSLg sometimes refuses the Wayland socket after a sleep or
            // restart. Retry on X11 before giving up.
            eprintln!("Wayland failed ({e}); retrying on X11");
            std::env::remove_var("WAYLAND_DISPLAY");
            anvil_ui::run()
        }
        Err(e) => Err(e),
    }
}
