/// Color scheme definitions for the TUI
/// Supports popular terminal color schemes used in other applications

use ratatui::style::Color;

/// A complete color scheme for the application
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ColorScheme {
    /// Application name
    pub name: &'static str,
    /// Background color
    pub background: Color,
    /// Foreground/text color
    pub foreground: Color,
    /// Accent/highlight color (for selected items)
    pub accent: Color,
    /// Secondary accent color
    pub accent_secondary: Color,
    /// Success/positive color
    pub success: Color,
    /// Warning color
    pub warning: Color,
    /// Error/danger color
    pub error: Color,
    /// Muted/dimmed text color
    pub muted: Color,
    /// Border color
    pub border: Color,
    /// Title/header color
    pub title: Color,
}

impl ColorScheme {
    /// Get a color scheme by name
    pub fn by_name(name: &str) -> Self {
        match name.to_lowercase().as_str() {
            "monokai" => Self::monokai(),
            "solarized-dark" => Self::solarized_dark(),
            "solarized-light" => Self::solarized_light(),
            "nord" => Self::nord(),
            "dracula" => Self::dracula(),
            "gruvbox-dark" => Self::gruvbox_dark(),
            "gruvbox-light" => Self::gruvbox_light(),
            "tokyo-night" => Self::tokyo_night(),
            "catppuccin" => Self::catppuccin(),
            _ => Self::default_dark(),
        }
    }

    /// Get all available scheme names
    #[allow(dead_code)]
    pub fn available_schemes() -> Vec<&'static str> {
        vec![
            "Default Dark",
            "Default Light",
            "Monokai",
            "Solarized Dark",
            "Solarized Light",
            "Nord",
            "Dracula",
            "Gruvbox Dark",
            "Gruvbox Light",
            "Tokyo Night",
            "Catppuccin",
        ]
    }

    /// Convert display name to internal name
    #[allow(dead_code)]
    pub fn display_to_internal(display_name: &str) -> &'static str {
        match display_name {
            "Default Dark" => "default-dark",
            "Default Light" => "default-light",
            "Monokai" => "monokai",
            "Solarized Dark" => "solarized-dark",
            "Solarized Light" => "solarized-light",
            "Nord" => "nord",
            "Dracula" => "dracula",
            "Gruvbox Dark" => "gruvbox-dark",
            "Gruvbox Light" => "gruvbox-light",
            "Tokyo Night" => "tokyo-night",
            "Catppuccin" => "catppuccin",
            _ => "default-dark",
        }
    }

    /// Convert internal name to display name
    pub fn internal_to_display(internal_name: &str) -> &'static str {
        match internal_name {
            "default-dark" => "Default Dark",
            "default-light" => "Default Light",
            "monokai" => "Monokai",
            "solarized-dark" => "Solarized Dark",
            "solarized-light" => "Solarized Light",
            "nord" => "Nord",
            "dracula" => "Dracula",
            "gruvbox-dark" => "Gruvbox Dark",
            "gruvbox-light" => "Gruvbox Light",
            "tokyo-night" => "Tokyo Night",
            "catppuccin" => "Catppuccin",
            // Legacy compatibility
            "light" => "Default Light",
            "dark" => "Default Dark",
            "auto" => "Default Dark",
            _ => "Default Dark",
        }
    }

    /// Default dark scheme (similar to classic terminal)
    pub fn default_dark() -> Self {
        Self {
            name: "Default Dark",
            background: Color::Black,
            foreground: Color::White,
            accent: Color::Yellow,
            accent_secondary: Color::Cyan,
            success: Color::Green,
            warning: Color::Yellow,
            error: Color::Red,
            muted: Color::DarkGray,
            border: Color::DarkGray,
            title: Color::Cyan,
        }
    }

    /// Default light scheme
    #[allow(dead_code)]
    pub fn default_light() -> Self {
        Self {
            name: "Default Light",
            background: Color::White,
            foreground: Color::Black,
            accent: Color::Blue,
            accent_secondary: Color::Cyan,
            success: Color::Green,
            warning: Color::Yellow,
            error: Color::Red,
            muted: Color::Gray,
            border: Color::Gray,
            title: Color::Blue,
        }
    }

    /// Monokai color scheme (popular in Sublime Text, VS Code)
    pub fn monokai() -> Self {
        Self {
            name: "Monokai",
            background: Color::Rgb(39, 40, 34),      // #272822
            foreground: Color::Rgb(248, 248, 242),   // #F8F8F2
            accent: Color::Rgb(249, 38, 114),        // #F92672 (pink)
            accent_secondary: Color::Rgb(102, 217, 239), // #66D9EF (cyan)
            success: Color::Rgb(166, 226, 46),       // #A6E22E (green)
            warning: Color::Rgb(253, 151, 31),       // #FD971F (orange)
            error: Color::Rgb(249, 38, 114),         // #F92672 (pink)
            muted: Color::Rgb(117, 113, 94),         // #75715E
            border: Color::Rgb(73, 72, 62),          // #49483E
            title: Color::Rgb(102, 217, 239),        // #66D9EF (cyan)
        }
    }

    /// Solarized Dark color scheme
    pub fn solarized_dark() -> Self {
        Self {
            name: "Solarized Dark",
            background: Color::Rgb(0, 43, 54),       // #002B36
            foreground: Color::Rgb(131, 148, 150),   // #839496
            accent: Color::Rgb(181, 137, 0),         // #B58900 (yellow)
            accent_secondary: Color::Rgb(42, 161, 152), // #2AA198 (cyan)
            success: Color::Rgb(133, 153, 0),        // #859900 (green)
            warning: Color::Rgb(203, 75, 22),        // #CB4B16 (orange)
            error: Color::Rgb(220, 50, 47),          // #DC322F (red)
            muted: Color::Rgb(88, 110, 117),         // #586E75
            border: Color::Rgb(7, 54, 66),           // #073642
            title: Color::Rgb(38, 139, 210),         // #268BD2 (blue)
        }
    }

    /// Solarized Light color scheme
    pub fn solarized_light() -> Self {
        Self {
            name: "Solarized Light",
            background: Color::Rgb(253, 246, 227),   // #FDF6E3
            foreground: Color::Rgb(101, 123, 131),   // #657B83
            accent: Color::Rgb(181, 137, 0),         // #B58900 (yellow)
            accent_secondary: Color::Rgb(42, 161, 152), // #2AA198 (cyan)
            success: Color::Rgb(133, 153, 0),        // #859900 (green)
            warning: Color::Rgb(203, 75, 22),        // #CB4B16 (orange)
            error: Color::Rgb(220, 50, 47),          // #DC322F (red)
            muted: Color::Rgb(147, 161, 161),        // #93A1A1
            border: Color::Rgb(238, 232, 213),       // #EEE8D5
            title: Color::Rgb(38, 139, 210),         // #268BD2 (blue)
        }
    }

    /// Nord color scheme
    pub fn nord() -> Self {
        Self {
            name: "Nord",
            background: Color::Rgb(46, 52, 64),      // #2E3440
            foreground: Color::Rgb(216, 222, 233),   // #D8DEE9
            accent: Color::Rgb(136, 192, 208),       // #88C0D0 (frost)
            accent_secondary: Color::Rgb(129, 161, 193), // #81A1C1 (frost)
            success: Color::Rgb(163, 190, 140),      // #A3BE8C (green)
            warning: Color::Rgb(235, 203, 139),      // #EBCB8B (yellow)
            error: Color::Rgb(191, 97, 106),         // #BF616A (red)
            muted: Color::Rgb(76, 86, 106),          // #4C566A
            border: Color::Rgb(59, 66, 82),          // #3B4252
            title: Color::Rgb(143, 188, 187),        // #8FBCBB (frost)
        }
    }

    /// Dracula color scheme
    pub fn dracula() -> Self {
        Self {
            name: "Dracula",
            background: Color::Rgb(40, 42, 54),      // #282A36
            foreground: Color::Rgb(248, 248, 242),   // #F8F8F2
            accent: Color::Rgb(255, 121, 198),       // #FF79C6 (pink)
            accent_secondary: Color::Rgb(139, 233, 253), // #8BE9FD (cyan)
            success: Color::Rgb(80, 250, 123),       // #50FA7B (green)
            warning: Color::Rgb(241, 250, 140),      // #F1FA8C (yellow)
            error: Color::Rgb(255, 85, 85),          // #FF5555 (red)
            muted: Color::Rgb(98, 114, 164),         // #6272A4
            border: Color::Rgb(68, 71, 90),          // #44475A
            title: Color::Rgb(189, 147, 249),        // #BD93F9 (purple)
        }
    }

    /// Gruvbox Dark color scheme
    pub fn gruvbox_dark() -> Self {
        Self {
            name: "Gruvbox Dark",
            background: Color::Rgb(40, 40, 40),      // #282828
            foreground: Color::Rgb(235, 219, 178),   // #EBDBB2
            accent: Color::Rgb(254, 128, 25),        // #FE8019 (orange)
            accent_secondary: Color::Rgb(131, 165, 152), // #83A598 (blue)
            success: Color::Rgb(184, 187, 38),       // #B8BB26 (green)
            warning: Color::Rgb(250, 189, 47),       // #FABD2F (yellow)
            error: Color::Rgb(251, 73, 52),          // #FB4934 (red)
            muted: Color::Rgb(146, 131, 116),        // #928374
            border: Color::Rgb(60, 56, 54),          // #3C3836
            title: Color::Rgb(142, 192, 124),        // #8EC07C (aqua)
        }
    }

    /// Gruvbox Light color scheme
    pub fn gruvbox_light() -> Self {
        Self {
            name: "Gruvbox Light",
            background: Color::Rgb(251, 241, 199),   // #FBF1C7
            foreground: Color::Rgb(60, 56, 54),      // #3C3836
            accent: Color::Rgb(175, 58, 3),          // #AF3A03 (orange)
            accent_secondary: Color::Rgb(7, 102, 120), // #076678 (blue)
            success: Color::Rgb(121, 116, 14),       // #79740E (green)
            warning: Color::Rgb(181, 118, 20),       // #B57614 (yellow)
            error: Color::Rgb(204, 36, 29),          // #CC241D (red)
            muted: Color::Rgb(146, 131, 116),        // #928374
            border: Color::Rgb(213, 196, 161),       // #D5C4A1
            title: Color::Rgb(66, 123, 88),          // #427B58 (aqua)
        }
    }

    /// Tokyo Night color scheme
    pub fn tokyo_night() -> Self {
        Self {
            name: "Tokyo Night",
            background: Color::Rgb(26, 27, 38),      // #1A1B26
            foreground: Color::Rgb(169, 177, 214),   // #A9B1D6
            accent: Color::Rgb(125, 207, 255),       // #7DCFFF (cyan)
            accent_secondary: Color::Rgb(187, 154, 247), // #BB9AF7 (purple)
            success: Color::Rgb(158, 206, 106),      // #9ECE6A (green)
            warning: Color::Rgb(224, 175, 104),      // #E0AF68 (yellow)
            error: Color::Rgb(247, 118, 142),        // #F7768E (red)
            muted: Color::Rgb(86, 95, 137),          // #565F89
            border: Color::Rgb(30, 32, 48),          // #1E2030
            title: Color::Rgb(125, 207, 255),        // #7DCFFF (cyan)
        }
    }

    /// Catppuccin (Mocha variant) color scheme
    pub fn catppuccin() -> Self {
        Self {
            name: "Catppuccin",
            background: Color::Rgb(30, 30, 46),      // #1E1E2E
            foreground: Color::Rgb(205, 214, 244),   // #CDD6F4
            accent: Color::Rgb(245, 194, 231),       // #F5C2E7 (pink)
            accent_secondary: Color::Rgb(137, 220, 235), // #89DCEB (sky)
            success: Color::Rgb(166, 227, 161),      // #A6E3A1 (green)
            warning: Color::Rgb(249, 226, 175),      // #F9E2AF (yellow)
            error: Color::Rgb(243, 139, 168),        // #F38BA8 (red)
            muted: Color::Rgb(108, 112, 134),        // #6C7086
            border: Color::Rgb(49, 50, 68),          // #313244
            title: Color::Rgb(148, 226, 213),        // #94E2D5 (teal)
        }
    }
}

impl Default for ColorScheme {
    fn default() -> Self {
        Self::default_dark()
    }
}
