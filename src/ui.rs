use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::mpsc::Receiver,
};

use crate::config::{Config, Font, Orientation};
use egui::{
    Color32, FontData, FontDefinitions, FontFamily, Image, ImageSource, Label, Rounding, Sense,
    Stroke, Ui, Vec2,
};
use freedesktop_icons::lookup;
use qtile_client_lib::utils::client::InteractiveCommandClient;
use serde_json::Value;

pub struct AsyncApp {
    rx: Option<Receiver<Response>>,
    current_focus_history: Option<Response>,
    previous_focus_history: Option<Response>,
    config: Config,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Response {
    pub message_type: MessageType,
    pub windows: Vec<HashMap<String, String>>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum MessageType {
    ClientFocus,
    CycleWindows,
    None,
}

impl AsyncApp {
    pub fn add_font(fonts: &mut FontDefinitions, family: String, font: &Font) {
        let font_path = Path::new(&font.path);
        if Path::exists(font_path) {
            let bytes = std::fs::read(font_path).unwrap().clone();
            fonts
                .font_data
                .insert(font.name.to_owned(), FontData::from_owned(bytes));
            fonts
                .families
                .get_mut(&FontFamily::Name(family.clone().into()))
                .unwrap()
                .insert(0, font.name.to_owned());
        } else {
            log::warn!(
                "Font {:?} was not loaded since path {:?} does not exist",
                font.name,
                font_path,
            )
        }
    }
    pub fn add_font_family(fonts: &mut FontDefinitions, font_family_name: String) {
        fonts.families.extend([(
            FontFamily::Name(font_family_name.clone().into()),
            Vec::new(),
        )]);
    }
    pub fn new(cc: &eframe::CreationContext<'_>, rx: Option<Receiver<Response>>) -> Self {
        let cfg: Result<Config, confy::ConfyError> = confy::load("qalttab", Some("config"));
        let fonts = &mut FontDefinitions::default();
        let config = match cfg {
            Ok(cfg) => {
                log::debug!("Loaded config: {:#?}", cfg);
                let (cfg_family, cfg_text_fonts) =
                    (&cfg.fonts.text_font.family_name, &cfg.fonts.text_font.fonts);
                Self::add_font_family(fonts, cfg_family.clone());
                for font in cfg_text_fonts.iter() {
                    Self::add_font(fonts, cfg_family.clone(), font);
                }
                let (cfg_family, cfg_icon_fonts) =
                    (&cfg.fonts.icon_font.family_name, &cfg.fonts.icon_font.fonts);
                Self::add_font_family(fonts, cfg_family.clone());
                for font in cfg_icon_fonts.iter() {
                    Self::add_font(fonts, cfg_family.clone(), font);
                }
                cfg
            }
            Err(e) => {
                log::debug!("Failed to load config: {e}");
                let def_cfg = Config::default();
                let (def_cfg_family, def_cfg_text_fonts) = (
                    &def_cfg.fonts.text_font.family_name,
                    &def_cfg.fonts.text_font.fonts,
                );
                Self::add_font_family(fonts, def_cfg_family.clone());
                for font in def_cfg_text_fonts.iter() {
                    Self::add_font(fonts, def_cfg_family.clone(), font);
                }
                let (def_cfg_icon_family, def_cfg_icon_fonts) = (
                    &def_cfg.fonts.icon_font.family_name,
                    &def_cfg.fonts.icon_font.fonts,
                );
                Self::add_font_family(fonts, def_cfg_icon_family.clone());
                for font in def_cfg_icon_fonts.iter() {
                    Self::add_font(fonts, def_cfg_icon_family.clone(), font);
                }
                def_cfg
            }
        };
        cc.egui_ctx.set_fonts(fonts.clone());
        egui_extras::install_image_loaders(&cc.egui_ctx);
        Self {
            rx,
            current_focus_history: None,
            previous_focus_history: None,
            config,
        }
    }

    pub fn find_icon(&self, wm_class: &str) -> Option<PathBuf> {
        let mut icon_lookup_builder = lookup(wm_class)
            .with_size(self.config.icons.lookup_icon_size as u16)
            .with_cache();
        let themes = self.config.icons.themes.clone();
        for theme in themes.iter() {
            icon_lookup_builder = icon_lookup_builder.with_theme(theme);
        }

        icon_lookup_builder.find()
    }

    pub fn new_image(&self, ui: &mut Ui, path: &str) -> egui::Response {
        ui.add(
            Image::new(ImageSource::Uri(format!("file://{}", path).into())).max_size(Vec2 {
                x: self.config.icons.visible_icon_size,
                y: self.config.icons.visible_icon_size,
            }),
        )
        .interact(Sense::hover())
    }

    pub fn window_icon(&self, ui: &mut Ui, win: &HashMap<String, String>) -> egui::Response {
        let wm_class = win.get("class").expect("qtile sends correct format");
        let lowercase_wm_class = wm_class.clone().to_lowercase();
        let lowercase_wm_class = lowercase_wm_class.as_str();
        let path = self.find_icon(lowercase_wm_class);
        match path {
            Some(p) => match p.to_str() {
                Some(p) => self.new_image(ui, p),
                None => self.new_image(ui, &self.config.icons.default_icon),
            },
            None => match self.find_icon(wm_class) {
                Some(p) => match p.to_str() {
                    Some(p) => self.new_image(ui, p),
                    None => self.new_image(ui, &self.config.icons.default_icon),
                },
                None => self.new_image(ui, &self.config.icons.default_icon),
            },
        }
    }

    pub fn new_label(&self, ui: &mut Ui, text: &String, font: &egui::FontId) -> egui::Response {
        ui.add(Label::new(egui::RichText::new(text).font(font.clone())).wrap())
            .interact(Sense::hover())
    }

    pub fn window_name(
        &self,
        ui: &mut Ui,
        text_font_id: &egui::FontId,
        win: &HashMap<String, String>,
    ) -> egui::Response {
        let mut name = win.get("name").expect("qtile sends correct format").clone();
        if name.len() > 31 {
            let upto = name
                .char_indices()
                .map(|(i, _)| i)
                .nth(30)
                .unwrap_or(name.len());
            name.truncate(upto);
        }
        self.new_label(ui, &name, text_font_id)
    }

    pub fn render_ui(&self, ctx: &eframe::egui::Context, windows: &[HashMap<String, String>]) {
        ctx.all_styles_mut(|style| {
            style.visuals.panel_fill =
                Color32::from_hex(self.config.colors.bg_color.as_str()).expect("color from hex");
        });
        let mut sum_of_heights = 0.0;
        let text_font_id = egui::FontId {
            family: FontFamily::Name(self.config.fonts.text_font.family_name.clone().into()),
            size: self.config.fonts.text_font.size,
        };
        let icon_font_id = egui::FontId {
            size: self.config.fonts.icon_font.size,
            family: FontFamily::Name(self.config.fonts.icon_font.family_name.clone().into()),
        };
        egui::CentralPanel::default().show(ctx, |ui| {
            // TODO: horizontal layout as well
            if self.config.ui.orientation == Orientation::Vertical {
                let vertical = ui
                    .vertical(|ui| {
                        for (index, win) in windows.iter().enumerate() {
                            ui.style_mut().visuals.widgets.noninteractive.bg_stroke = Stroke {
                                width: 0.0,
                                color: Color32::from_hex(self.config.colors.text_color.as_str())
                                    .expect("color from hex"),
                            };
                            ui.style_mut().visuals.widgets.noninteractive.fg_stroke = Stroke {
                                width: 0.0,
                                color: Color32::from_hex(self.config.colors.text_color.as_str())
                                    .expect("color from hex"),
                            };
                            ui.style_mut().interaction.selectable_labels = false;
                            let group = ui
                                .group(|ui| {
                                    ui.with_layout(
                                        egui::Layout::top_down(egui::Align::Center),
                                        |ui| {
                                            for item in self.config.ui.items.iter() {
                                                match item {
                                                    crate::config::UiItem::Icon => {
                                                        self.window_icon(ui, win);
                                                    }
                                                    crate::config::UiItem::Name => {
                                                        self.window_name(ui, &text_font_id, win);
                                                    }
                                                    crate::config::UiItem::GroupName => {
                                                        self.new_label(
                                                            ui,
                                                            win.get("group_name").expect(
                                                                "qtile sends correct format",
                                                            ),
                                                            &text_font_id,
                                                        );
                                                    }
                                                    crate::config::UiItem::GroupLabel => {
                                                        self.new_label(
                                                            ui,
                                                            win.get("group_label").expect(
                                                                "qtile sends correct format",
                                                            ),
                                                            &icon_font_id,
                                                        );
                                                    }
                                                }
                                            }
                                        },
                                    );
                                })
                                .response
                                .interact(egui::Sense::click())
                                .on_hover_cursor(egui::CursorIcon::Crosshair);
                            sum_of_heights += group.rect.height();

                            if index != 0 {
                                if !group.hovered() {
                                    ui.painter().rect_stroke(
                                        group.rect,
                                        Rounding::same(10.0),
                                        Stroke {
                                            width: self.config.sizes.group_rect_stroke_width,
                                            color: Color32::from_hex(
                                                self.config.colors.normal_group_color.as_str(),
                                            )
                                            .expect("color from hex"), // Highlight color
                                        },
                                    );
                                } else {
                                    ui.painter().rect_stroke(
                                        group.rect,
                                        Rounding::same(10.0),
                                        Stroke {
                                            width: self.config.sizes.group_rect_stroke_width,
                                            color: Color32::from_hex(
                                                self.config.colors.group_hover_color.as_str(),
                                            )
                                            .expect("color from hex"),
                                        },
                                    );
                                }
                            } else if !group.hovered() {
                                ui.painter().rect_stroke(
                                    group.rect,
                                    Rounding::same(10.0),
                                    Stroke {
                                        width: self.config.sizes.group_rect_stroke_width,
                                        color: Color32::from_hex(
                                            self.config.colors.group_hover_color.as_str(),
                                        )
                                        .expect("color from hex"),
                                    },
                                );
                            } else {
                                ui.painter().rect_stroke(
                                    group.rect,
                                    Rounding::same(10.0),
                                    Stroke {
                                        width: self.config.sizes.group_rect_stroke_width,
                                        color: Color32::from_hex(
                                            self.config.colors.normal_group_color.as_str(),
                                        )
                                        .expect("color from hex"),
                                    },
                                );
                            }
                            if group.middle_clicked() {
                                self.close_window(win);
                            }
                            if group.clicked() {
                                self.focus_window(win);
                                self.hide_our_window();
                            };
                            if index < windows.len() - 1 {
                                ui.add_space(self.config.sizes.group_spacing);
                            }
                        }
                    })
                    .response
                    .rect
                    .height();
                log::debug!("vertical: {}", vertical);
                sum_of_heights = vertical;
            }
        });
        let width = (self.config.sizes.window_size.width as i32).to_string();
        let height = sum_of_heights.min(self.config.sizes.window_size.height)
            + ctx.style().spacing.window_margin.top
            + ctx.style().spacing.window_margin.bottom
            + self.config.sizes.group_rect_stroke_width;
        log::debug!("height: {}", height);
        let height = (height as i32).to_string();
        self.place_our_window(width, height);
        ctx.request_repaint();
    }

    pub fn focus_window(&self, win: &HashMap<String, String>) {
        let wid = win
            .get("id")
            .unwrap_or_else(|| panic!("qtile sends correct format {:?}", win.get("id")))
            .to_string();
        //  qtile.current_screen.set_group(next_window.group)
        let _ = InteractiveCommandClient::call(
            Some(vec![]),
            Some("eval".into()),
            Some(vec![format!(
                "self.current_screen.set_group(self.windows_map[{}].group)",
                wid
            )]),
            false,
        );
        //  next_window.group.focus(next_window, warp=False)  # type: ignore
        let _ = InteractiveCommandClient::call(
            Some(vec!["window".to_owned(), wid.clone()]),
            Some("focus".into()),
            Some(vec![]),
            false,
        );
        //  next_window.bring_to_front()
        let _ = InteractiveCommandClient::call(
            Some(vec!["window".to_owned(), wid]),
            Some("bring_to_front".into()),
            Some(vec![]),
            false,
        );
    }

    pub fn get_our_window_id(&self) -> Option<String> {
        let response = InteractiveCommandClient::call(
            Some(vec![]),
            Some("eval".into()),
            Some(vec![r#"__import__("json").dumps(
            [
                {
                    "name": self.windows_map[wid].name,
                    "wid": str(self.windows_map[wid].wid)
                }
                for wid in self.windows_map
                if hasattr(self.windows_map[wid], "name")
                and hasattr(self.windows_map[wid], "wid")
            ]
        )"#
            .into()]),
            false,
        )
        .expect("qtile has a `windows_map`");

        let response = serde_json::from_value::<Vec<Value>>(response)
            .expect("response is a tuple of (bool, actual_response)")[1]
            .clone();

        let response = serde_json::from_value::<String>(response).expect("response is string");

        let response = serde_json::from_str::<Vec<HashMap<String, String>>>(&response)
            .expect("windows is a dict(str,str)");

        let win = response
            .iter()
            .filter(|map| map.get("name") == Some(&"qalttab".to_string()))
            .cloned()
            .collect::<Vec<HashMap<String, String>>>();
        let win = win.first();
        match win {
            Some(win) => win.get("wid").cloned(),
            None => {
                log::debug!("Window is not yet ready");
                None
            }
        }
    }

    pub fn hide_our_window(&self) {
        let wid = self.get_our_window_id();
        match wid {
            Some(wid) => {
                let _response = InteractiveCommandClient::call(
                    Some(vec!["window".to_owned(), wid.to_owned()]),
                    Some("hide".into()),
                    Some(vec![]),
                    false,
                );
            }
            None => log::debug!("Could not hide window"),
        }
    }

    pub fn place_our_window(&self, width: String, height: String) {
        let wid = self.get_our_window_id();

        match wid {
            Some(wid) => {
                let _response = InteractiveCommandClient::call(
                    Some(vec!["window".to_owned(), wid.to_owned()]),
                    Some("set_size_floating".into()),
                    Some(vec![width, height]),
                    false,
                );

                let _response = InteractiveCommandClient::call(
                    Some(vec!["window".to_owned(), wid.to_owned()]),
                    Some("center".into()),
                    Some(vec![]),
                    false,
                );
            }
            None => log::debug!("Could not place our window"),
        }
    }

    fn close_window(&self, win: &HashMap<String, String>) {
        let wid = win.get("id").expect("qtile sends correct format");
        let _response = InteractiveCommandClient::call(
            Some(vec!["window".to_owned(), wid.to_owned()]),
            Some("kill".into()),
            Some(vec![]),
            false,
        );
    }
}

impl eframe::App for AsyncApp {
    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) {
        // Check for new messages and display them
        if let Some(receiver) = &self.rx {
            while let Ok(message) = receiver.try_recv() {
                self.current_focus_history = Some(message);
            }
        };
        if let Some(current_focus_history) = &self.current_focus_history {
            self.render_ui(ctx, &current_focus_history.windows);
            match current_focus_history.message_type {
                MessageType::CycleWindows => {
                    if let Some(previous_focus_history) = &self.previous_focus_history {
                        if current_focus_history.windows != previous_focus_history.windows {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                            self.render_ui(ctx, &current_focus_history.windows);
                            self.previous_focus_history = Some(current_focus_history.clone());
                        }
                    }
                }
                MessageType::ClientFocus => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
                    self.render_ui(ctx, &current_focus_history.windows);
                    self.previous_focus_history = Some(current_focus_history.clone());
                }
                MessageType::None => todo!(),
            }
        }
        ctx.request_repaint();
    }
}
