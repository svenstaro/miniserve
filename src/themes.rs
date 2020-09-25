use serde::Deserialize;
use structopt::clap::arg_enum;
use strum_macros::EnumIter;

arg_enum! {
    #[derive(PartialEq, Deserialize, Clone, EnumIter, Copy)]
    #[serde(rename_all = "lowercase")]
    pub enum ColorScheme {
        Archlinux,
        Zenburn,
        Monokai,
        Squirrel,
    }
}

impl ColorScheme {
    /// Returns the name identifying the theme
    pub fn to_slug(self) -> &'static str {
        match self {
            ColorScheme::Archlinux => "archlinux",
            ColorScheme::Zenburn => "zenburn",
            ColorScheme::Monokai => "monokai",
            ColorScheme::Squirrel => "squirrel",
        }
    }

    /// Returns wether a color scheme is dark
    pub fn is_dark(self) -> bool {
        match self {
            ColorScheme::Archlinux => true,
            ColorScheme::Zenburn => true,
            ColorScheme::Monokai => true,
            ColorScheme::Squirrel => false,
        }
    }
}
