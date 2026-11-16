#[cfg(feature = "desktop")]
pub mod desktop_window {
    use dioxus::desktop::tao::dpi::LogicalSize;
    use dioxus::desktop::{Config, WindowBuilder};

    #[derive(Debug, Clone, Copy)]
    pub struct ScreenDimensions {
        pub width: u32,
        pub height: u32,
    }

    /// Obtains monitor resolution if available, with robust fallbacks
    pub fn get_monitor_dimensions() -> ScreenDimensions {
        // Fallback default full HD
        let mut dims = ScreenDimensions {
            width: 1920,
            height: 1080,
        };

        #[cfg(target_os = "linux")]
        {
            // Try reading modes from sysfs /sys/class/drm
            if let Ok(entries) = std::fs::read_dir("/sys/class/drm") {
                for entry in entries.flatten() {
                    let modes_path = entry.path().join("modes");
                    if modes_path.exists()
                        && let Ok(content) = std::fs::read_to_string(modes_path)
                            && let Some(first_line) = content.lines().next()
                                && let Some((w, h)) = first_line.split_once('x')
                                    && let (Ok(parsed_w), Ok(parsed_h)) = (w.trim().parse::<u32>(), h.trim().parse::<u32>()) {
                                        dims.width = parsed_w;
                                        dims.height = parsed_h;
                                        return dims;
                                    }
                }
            }

            // Fallback via xrandr or xdpyinfo
            if let Ok(output) = std::process::Command::new("xrandr").arg("--current").output() {
                let out_str = String::from_utf8_lossy(&output.stdout);
                for line in out_str.lines() {
                    if line.contains(" connected") && line.contains("+") {
                        // e.g. "eDP-1 connected primary 1920x1080+0+0"
                        for part in line.split_whitespace() {
                            if let Some((res, _)) = part.split_once('+')
                                && let Some((w, h)) = res.split_once('x')
                                    && let (Ok(pw), Ok(ph)) = (w.parse::<u32>(), h.parse::<u32>()) {
                                        dims.width = pw;
                                        dims.height = ph;
                                        return dims;
                                    }
                        }
                    }
                }
            }
        }

        dims
    }

    const TAILWIND_CSS_RAW: &str = include_str!("../../assets/tailwind.css");
    const MAIN_CSS_RAW: &str = include_str!("../../assets/main.css");

    /// Creates an optimized desktop config for full screen / maximized OLED IDE experience
    pub fn create_fullscreen_config(title: &str) -> Config {
        let dims = get_monitor_dimensions();

        let head_content = format!(
            r#"<style>
html, body {{
    background-color: #0a0a0a !important;
    color: #ededed !important;
    margin: 0 !important;
    padding: 0 !important;
    overflow: hidden !important;
}}
{}
{}
</style>"#,
            MAIN_CSS_RAW, TAILWIND_CSS_RAW
        );

        Config::new()
            .with_menu(None)
            .with_custom_head(head_content)
            .with_window(
                WindowBuilder::new()
                    .with_title(title)
                    .with_inner_size(LogicalSize::new(dims.width as f64, dims.height as f64))
                    .with_maximized(true),
            )
    }
}
