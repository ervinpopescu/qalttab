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

#[derive(Deserialize, Serialize, Debug, PartialEq, Eq)]
pub struct UiConfig {
    pub items: Vec<UiItem>,
    pub orientation: Orientation,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults() {
        let config = Config::default();
        assert_eq!(config.ui.orientation, Orientation::Vertical);
        assert!(!config.ui.items.is_empty());
        assert!(!config.fonts.text_font.fonts.is_empty());
        assert!(!config.fonts.icon_font.fonts.is_empty());
        assert!(config.sizes.group_spacing > 0.0);
        assert!(config.sizes.window_size.width > 0.0);
        assert!(config.sizes.window_size.height > 0.0);
        assert!(config.colors.bg_color.starts_with('#'));
        assert_eq!(config.colors.bg_color.len(), 7);
        assert!(!config.icons.themes.is_empty());
        assert!(config.icons.visible_icon_size > 0.0);
    }

    #[test]
    fn test_config_serde() {
        let config = Config::default();
        let serialized = serde_json::to_string(&config).unwrap();
        let deserialized: Config = serde_json::from_str(&serialized).unwrap();
        assert_eq!(config.ui.orientation, deserialized.ui.orientation);
        assert_eq!(
            config.fonts.text_font.family_name,
            deserialized.fonts.text_font.family_name
        );
    }

    #[test]
    fn test_custom_config() {
        let mut config = Config::default();
        config.ui.orientation = Orientation::Horizontal;
        config.sizes.group_spacing = 15.5;
        config.colors.bg_color = "#000000".to_string();

        let serialized = serde_json::to_string(&config).unwrap();
        let deserialized: Config = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.ui.orientation, Orientation::Horizontal);
        assert_eq!(deserialized.sizes.group_spacing, 15.5);
        assert_eq!(deserialized.colors.bg_color, "#000000");
    }

    #[test]
    fn test_font_utils() {
        let name = "Test Font";
        let path = "/path/to/font.ttf";
        let font = Font::new(name, path);
        assert_eq!(font.name, name);
        assert_eq!(font.path, path);

        let f1 = Font::new("N", "P");
        let f2 = Font::new("N", "P");
        let f3 = Font::new("N", "P2");
        assert_eq!(f1.name, f2.name);
        assert_ne!(f1.path, f3.path);
    }

    #[test]
    fn test_sub_struct_serde() {
        let size = WindowSize {
            width: 100.0,
            height: 200.0,
        };
        let s_serialized = serde_json::to_string(&size).unwrap();
        let s_deserialized: WindowSize = serde_json::from_str(&s_serialized).unwrap();
        assert_eq!(size.width, s_deserialized.width);

        let colors = Colors {
            bg_color: "#123456".into(),
            text_color: "#ffffff".into(),
            normal_group_color: "#000000".into(),
            group_hover_color: "#ff0000".into(),
        };
        let c_serialized = serde_json::to_string(&colors).unwrap();
        let c_deserialized: Colors = serde_json::from_str(&c_serialized).unwrap();
        assert_eq!(colors.bg_color, c_deserialized.bg_color);

        assert_eq!(
            serde_json::from_str::<Orientation>("\"Horizontal\"").unwrap(),
            Orientation::Horizontal
        );
        assert_eq!(
            serde_json::from_str::<UiItem>("\"icon\"").unwrap(),
            UiItem::Icon
        );
    }

    #[test]
    fn test_font_family_logic() {
        let config = Config::default();
        let serialized = serde_json::to_string(&config.fonts).unwrap();
        let deserialized: Fonts = serde_json::from_str(&serialized).unwrap();
        assert_eq!(
            config.fonts.text_font.family_name,
            deserialized.text_font.family_name
        );

        let ff = FontFamily {
            family_name: "Test".into(),
            fonts: vec![Font::new("F1", "P1")],
            size: 10.0,
        };
        let ff_s = serde_json::to_string(&ff).unwrap();
        let ff_d: FontFamily = serde_json::from_str(&ff_s).unwrap();
        assert_eq!(ff.family_name, ff_d.family_name);
    }
}
