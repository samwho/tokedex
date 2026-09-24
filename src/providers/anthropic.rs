use super::Provider;

pub struct Anthropic;
pub static ANTHROPIC: Anthropic = Anthropic;

const LOGO: &[&str] = &[
    "      ⢀⣀⣀⣀⡀  ⢀⣀⣀⣀⡀",
    "      ⣾⣿⣿⣿⣿⡀  ⢻⣿⣿⣷",
    "     ⣼⣿⣿⣿⣿⣿⣷  ⠈⢿⣿⣿⣧",
    "    ⣰⣿⣿⣿⠋⢿⣿⣿⣧  ⠘⣿⣿⣿⣆",
    "   ⢠⣿⣿⣿⠇ ⠘⣿⣿⣿⣆  ⠸⣿⣿⣿⡄",
    "  ⢀⣿⣿⣿⣏⣀⣀⣀⣸⣿⣿⣿⡄  ⢹⣿⣿⣿⡀",
    "  ⣾⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⡀  ⢿⣿⣿⣷",
    " ⣼⣿⣿⣿⠛⠛⠛⠛⠛⠛⠛⢿⣿⣿⣷  ⠈⣿⣿⣿⣧",
    "⣰⣿⣿⣿⠃       ⠈⣿⣿⣿⣧  ⠘⣿⣿⣿⣆",
    "⠉⠉⠉⠉         ⠈⠉⠉⠉   ⠉⠉⠉⠉",
];

impl Provider for Anthropic {
    fn name(&self) -> &'static str {
        "anthropic"
    }

    fn ascii_art_logo(&self) -> &'static [&'static str] {
        LOGO
    }
}
