use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct Font {
    pub name: String,
    pub family: String,
    pub path: String,
}
impl Font {
    pub fn new(name: &str, family: &str, path: &str) -> Self {
        Self {
            name: name.to_string(),
            family: family.to_string(),
            path: path.to_string(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    pub fonts: Vec<Font>,
    pub icon_themes: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            fonts: vec![
                Font::new(
                    "Caskaydia Cove Regular",
                    "Caskaydia Cove",
                    "/usr/share/fonts/OTF/Caskaydia Cove Nerd Font Complete Regular.otf",
                ),
                Font::new(
                    "fa-brands",
                    "Font Awesome",
                    "/usr/share/fonts/TTF/fa-brands-400.ttf",
                ),
                Font::new(
                    "fa-regular",
                    "Font Awesome",
                    "/usr/share/fonts/TTF/fa-regular-400.ttf",
                ),
                Font::new(
                    "Font Awesome Solid",
                    "Font Awesome",
                    "/usr/share/fonts/TTF/fa-solid-900.ttf",
                ),
            ],
            icon_themes: vec![
                "Papirus".into(),
                "Papirus-Dark".into(),
                "Papirus-Light".into(),
            ],
        }
    }
}
