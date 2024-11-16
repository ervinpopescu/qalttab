use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct Font {
    pub name: String,
    pub path: String,
}
impl Font {
    pub fn new(name: &str, path: &str) -> Self {
        Self {
            name: name.to_string(),
            path: path.to_string(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Fonts {
    pub text_font: HashMap<String, Vec<Font>>,
    pub icon_font: HashMap<String, Vec<Font>>,
}

// TODO: Add `icon_font` and `text_font`
#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    pub fonts: Fonts,
    pub window_icon_themes: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        let mut text_font = HashMap::new();
        text_font.insert(
            "Caskaydia Cove".into(),
            vec![Font::new(
                "Caskaydia Cove Regular",
                "/usr/share/fonts/OTF/Caskaydia Cove Nerd Font Complete Regular.otf",
            )],
        );
        let mut icon_font = HashMap::new();
        icon_font.insert(
            "Font Awesome".into(),
            vec![
                Font::new("fa-brands", "/usr/share/fonts/TTF/fa-brands-400.ttf"),
                Font::new("fa-regular", "/usr/share/fonts/TTF/fa-regular-400.ttf"),
                Font::new(
                    "Font Awesome Solid",
                    "/usr/share/fonts/TTF/fa-solid-900.ttf",
                ),
            ],
        );
        Self {
            fonts: Fonts {
                text_font,
                icon_font,
            },
            window_icon_themes: vec![
                "Papirus".into(),
                "Papirus-Dark".into(),
                "Papirus-Light".into(),
            ],
        }
    }
}
