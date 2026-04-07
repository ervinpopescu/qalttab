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

#[derive(Serialize, Deserialize, Debug, PartialEq)]
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
    fn default_config_has_expected_colors() {
        let cfg = Config::default();
        assert_eq!(cfg.colors.bg_color, "#1E1E2E");
        assert_eq!(cfg.colors.group_hover_color, "#B4BEFE");
        assert_eq!(cfg.colors.normal_group_color, "#313244");
        assert_eq!(cfg.colors.text_color, "#6C7086");
    }

    #[test]
    fn default_config_has_expected_sizes() {
        let cfg = Config::default();
        assert_eq!(cfg.sizes.group_spacing, 8.0);
        assert_eq!(cfg.sizes.group_rect_stroke_width, 3.0);
        assert_eq!(cfg.sizes.window_size.width, 400.0);
        assert_eq!(cfg.sizes.window_size.height, 1000.0);
    }

    #[test]
    fn default_config_orientation_is_vertical() {
        assert_eq!(Config::default().ui.orientation, Orientation::Vertical);
    }

    #[test]
    fn default_config_items_include_all_four() {
        let items = &Config::default().ui.items;
        assert!(items.contains(&UiItem::Icon));
        assert!(items.contains(&UiItem::Name));
        assert!(items.contains(&UiItem::GroupName));
        assert!(items.contains(&UiItem::GroupLabel));
    }

    #[test]
    fn font_new_sets_name_and_path() {
        let f = Font::new("MyFont", "/usr/share/fonts/myfont.ttf");
        assert_eq!(f.name, "MyFont");
        assert_eq!(f.path, "/usr/share/fonts/myfont.ttf");
    }

    #[test]
    fn ui_item_serde_round_trip() {
        let items = vec![
            UiItem::Icon,
            UiItem::Name,
            UiItem::GroupName,
            UiItem::GroupLabel,
        ];
        let json = serde_json::to_string(&items).unwrap();
        let decoded: Vec<UiItem> = serde_json::from_str(&json).unwrap();
        assert_eq!(items, decoded);
    }

    #[test]
    fn ui_item_deserializes_from_snake_case_strings() {
        assert_eq!(
            serde_json::from_str::<UiItem>(r#""icon""#).unwrap(),
            UiItem::Icon
        );
        assert_eq!(
            serde_json::from_str::<UiItem>(r#""name""#).unwrap(),
            UiItem::Name
        );
        assert_eq!(
            serde_json::from_str::<UiItem>(r#""group_name""#).unwrap(),
            UiItem::GroupName
        );
        assert_eq!(
            serde_json::from_str::<UiItem>(r#""group_label""#).unwrap(),
            UiItem::GroupLabel
        );
    }

    #[test]
    fn config_json_round_trip() {
        let cfg = Config::default();
        let json = serde_json::to_string(&cfg).unwrap();
        let decoded: Config = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.colors.bg_color, cfg.colors.bg_color);
        assert_eq!(decoded.sizes.group_spacing, cfg.sizes.group_spacing);
        assert_eq!(
            decoded.fonts.text_font.family_name,
            cfg.fonts.text_font.family_name
        );
        assert_eq!(decoded.ui.orientation, cfg.ui.orientation);
        assert_eq!(decoded.icons.themes, cfg.icons.themes);
    }

    #[test]
    fn default_icon_config_themes_and_size() {
        let icons = Config::default().icons;
        assert_eq!(
            icons.themes,
            vec!["Papirus", "Papirus-Dark", "Papirus-Light"]
        );
        assert_eq!(icons.lookup_icon_size, 48.0);
    }

    #[test]
    fn default_window_size_is_400_by_1000() {
        let ws = Config::default().sizes.window_size;
        assert_eq!(ws.width, 400.0);
        assert_eq!(ws.height, 1000.0);
    }

    #[test]
    fn default_text_font_family_and_size() {
        let tf = Config::default().fonts.text_font;
        assert_eq!(tf.family_name, "Caskaydia Cove");
        assert_eq!(tf.size, 20.0);
    }

    #[test]
    fn default_icon_font_family_size_and_count() {
        let icf = Config::default().fonts.icon_font;
        assert_eq!(icf.family_name, "Font Awesome");
        assert_eq!(icf.size, 20.0);
        assert_eq!(icf.fonts.len(), 3);
    }

    #[test]
    fn orientation_serde_round_trip() {
        for orientation in [Orientation::Horizontal, Orientation::Vertical] {
            let json = serde_json::to_string(&orientation).unwrap();
            let decoded: Orientation = serde_json::from_str(&json).unwrap();
            assert_eq!(orientation, decoded);
        }
    }

    #[test]
    fn ui_item_unknown_variant_fails_to_deserialize() {
        assert!(serde_json::from_str::<UiItem>(r#""bogus""#).is_err());
    }

    #[test]
    fn orientation_unknown_variant_fails_to_deserialize() {
        assert!(serde_json::from_str::<Orientation>(r#""Diagonal""#).is_err());
    }

    #[test]
    fn config_missing_required_field_fails() {
        // Missing all but one field — must fail.
        let json = r##"{"colors":{"bg_color":"#000","text_color":"#fff","normal_group_color":"#111","group_hover_color":"#222"}}"##;
        assert!(serde_json::from_str::<Config>(json).is_err());
    }

    #[test]
    fn config_with_extra_unknown_field_still_parses() {
        let cfg = Config::default();
        let mut value = serde_json::to_value(&cfg).unwrap();
        value
            .as_object_mut()
            .unwrap()
            .insert("unknown_extra".into(), serde_json::json!("ignored"));
        let json = serde_json::to_string(&value).unwrap();
        let decoded: Config = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.colors.bg_color, cfg.colors.bg_color);
    }

    #[test]
    fn ui_items_empty_vec_round_trip() {
        let ui = UiConfig {
            items: vec![],
            orientation: Orientation::Horizontal,
        };
        let json = serde_json::to_string(&ui).unwrap();
        let decoded: UiConfig = serde_json::from_str(&json).unwrap();
        assert!(decoded.items.is_empty());
        assert_eq!(decoded.orientation, Orientation::Horizontal);
    }

    #[test]
    fn font_family_with_empty_fonts_round_trip() {
        let ff = FontFamily {
            family_name: "X".into(),
            fonts: vec![],
            size: 12.5,
        };
        let json = serde_json::to_string(&ff).unwrap();
        let decoded: FontFamily = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.family_name, "X");
        assert!(decoded.fonts.is_empty());
        assert_eq!(decoded.size, 12.5);
    }

    #[test]
    fn orientation_horizontal_serializes_as_pascal_case() {
        let json = serde_json::to_string(&Orientation::Horizontal).unwrap();
        assert_eq!(json, "\"Horizontal\"");
    }

    #[test]
    fn ui_item_icon_serializes_as_snake_case() {
        let json = serde_json::to_string(&UiItem::Icon).unwrap();
        assert_eq!(json, "\"icon\"");
    }
}
