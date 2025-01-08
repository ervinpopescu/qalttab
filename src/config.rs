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
pub struct FontFamily {
    pub family_name: String,
    pub fonts: Vec<Font>,
    pub size: f32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Fonts {
    pub text_font: FontFamily,
    pub icon_font: FontFamily,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct WindowSize {
    pub width: f32,
    pub height: f32,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct Colors {
    pub bg_color: String,
    pub text_color: String,
    pub normal_group_color: String,
    pub group_hover_color: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct IconConfig {
    pub themes: Vec<String>,
    pub lookup_icon_size: f32,
    pub visible_icon_size: f32,
    pub default_icon: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Sizes {
    pub group_spacing: f32,
    pub group_rect_stroke_width: f32,
    pub window_size: WindowSize,
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Eq)]
pub enum Orientation {
    Horizontal,
    Vertical,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct UiConfig {
    pub items: Vec<UiItem>,
    pub orientation: Orientation,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum UiItem {
    #[serde(rename = "icon")]
    Icon,
    #[serde(rename = "name")]
    Name,
    #[serde(rename = "group_name")]
    GroupName,
    #[serde(rename = "group_label")]
    GroupLabel,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    pub fonts: Fonts,
    pub colors: Colors,
    pub icons: IconConfig,
    pub sizes: Sizes,
    pub ui: UiConfig,
}

impl Default for Config {
    fn default() -> Self {
        let font_size = 20.0;
        let lookup_icon_size = 48.0;
        Self {
            fonts: Fonts {
                text_font: FontFamily {
                    family_name: "Caskaydia Cove".into(),
                    fonts: vec![Font::new(
                        "Caskaydia Cove Regular",
                        "/usr/share/fonts/OTF/Caskaydia Cove Nerd Font Complete Regular.otf",
                    )],
                    size: font_size,
                },
                icon_font: FontFamily {
                    family_name: "Font Awesome".into(),
                    fonts: vec![
                        Font::new("fa-brands", "/usr/share/fonts/TTF/fa-brands-400.ttf"),
                        Font::new("fa-regular", "/usr/share/fonts/TTF/fa-regular-400.ttf"),
                        Font::new(
                            "Font Awesome Solid",
                            "/usr/share/fonts/TTF/fa-solid-900.ttf",
                        ),
                    ],
                    size: font_size,
                },
            },
            sizes: Sizes {
                group_spacing: 8.0,
                group_rect_stroke_width: 3.0,
                window_size: WindowSize {
                    width: 400.0,
                    height: 1000.0,
                },
            },
            icons: IconConfig {
                themes: vec![
                    "Papirus".into(),
                    "Papirus-Dark".into(),
                    "Papirus-Light".into(),
                ],
                lookup_icon_size,
                visible_icon_size: lookup_icon_size,
                default_icon: "./assets/default.svg".into(),
            },
            colors: Colors {
                bg_color: "#1E1E2E".into(),
                group_hover_color: "#B4BEFE".into(),
                normal_group_color: "#313244".into(),
                text_color: "#6C7086".into(),
            },
            ui: UiConfig {
                items: vec![
                    UiItem::Icon,
                    UiItem::Name,
                    UiItem::GroupName,
                    UiItem::GroupLabel,
                ],
                orientation: Orientation::Vertical,
            },
        }
    }
}
