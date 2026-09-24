use super::Provider;

pub struct OpenAi;
pub static OPENAI: OpenAi = OpenAi;

const LOGO: &[&str] = &[
    "      ⣠⣴⣾⠿⠿⣿⣶⣤⣀⣀⣀⣀⡀",
    "    ⢀⣾⡟⠉  ⢀⣠⣾⡿⠟⠛⠛⠻⢿⣷⣄",
    "  ⢀⣤⣾⣿ ⢀⣴⣾⠿⠛⠉ ⣀⣤⣀  ⠙⣿⣧",
    "⢀⣴⡿⠛⣿⣿ ⢸⣿⡇⢀⣠⣴⣾⠿⠛⠻⣿⣦⣄⣸⣿",
    "⣾⡿⠁ ⣿⣿ ⢸⣿⣷⡿⠟⠻⢿⣶⣤⡀ ⠉⠻⢿⣿⣄",
    "⣿⣇  ⣿⣿⡀⢸⣿⡇    ⢸⣿⡿⣷⣦⡄ ⠙⣿⣆",
    "⠹⣿⣄ ⠘⠻⢿⣾⣿⡇    ⢸⣿⡇⠈⣿⣿  ⢹⣿",
    " ⠙⣿⣷⣦⣀ ⠈⠛⠿⣷⣦⣴⣾⢿⣿⡇ ⣿⣿ ⢀⣾⡿",
    "  ⣿⡏⠙⠻⣿⣦⣤⣶⡿⠟⠋⠁⢸⣿⡇ ⣿⣿⣤⣾⠟⠁",
    "  ⢻⣷⣄  ⠉⠛⠉ ⣀⣤⣶⡿⠟⠁ ⣿⡿⠛⠁",
    "   ⠙⢿⣷⣦⣤⣤⣴⣾⡿⠋⠁  ⣀⣼⡿⠁",
    "     ⠈⠉⠉⠉⠉⠛⠿⣿⣶⣶⡿⠟⠋",
];

impl Provider for OpenAi {
    fn name(&self) -> &'static str {
        "openai"
    }

    fn ascii_art_logo(&self) -> &'static [&'static str] {
        LOGO
    }
}
