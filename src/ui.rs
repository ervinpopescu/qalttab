use std::{collections::HashMap, path::Path, sync::mpsc::Receiver};

use egui::{Color32, FontData, FontDefinitions, FontFamily, Label, Rounding, Stroke, Ui, Vec2};
use qtile_client_lib::utils::client::InteractiveCommandClient;
use serde_json::Value;

const MAX_HEIGHT: f32 = 1000.0;
const FONT_SIZE: f32 = 20.0;

pub struct AsyncApp {
    is_first_run: bool,
    rx: Option<Receiver<Response>>,
    current_focus_history: Option<Response>,
    previous_focus_history: Option<Response>,
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
    pub fn add_font(
        fonts: &mut FontDefinitions,
        font_name: &str,
        font_family_name: &str,
        font_path: &str,
    ) {
        let font_path = Path::new(font_path);
        if Path::exists(font_path) {
            let bytes = std::fs::read(font_path).unwrap().clone();
            fonts
                .font_data
                .insert(font_name.to_owned(), FontData::from_owned(bytes));
            fonts
                .families
                .get_mut(&FontFamily::Name(font_family_name.into()))
                .unwrap()
                .insert(0, font_name.to_owned());
        } else {
            log::warn!(
                "Font {:?} was not loaded since path {:?} does not exist",
                font_name,
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
        let fonts = &mut FontDefinitions::default();
        Self::add_font_family(fonts, "Caskaydia Cove".to_owned());
        Self::add_font_family(fonts, "Font Awesome".to_owned());
        Self::add_font(
            fonts,
            "caskaydia-cove-regular",
            "Caskaydia Cove",
            "/usr/share/fonts/OTF/Caskaydia Cove Nerd Font Complete Regular.otf",
        );
        Self::add_font(
            fonts,
            "fa-brands",
            "Font Awesome",
            "/usr/share/fonts/TTF/fa-brands-400.ttf",
        );
        Self::add_font(
            fonts,
            "fa-v4compatibility",
            "Font Awesome",
            "/usr/share/fonts/TTF/fa-v4compatibility.ttf",
        );
        Self::add_font(
            fonts,
            "fa-regular",
            "Font Awesome",
            "/usr/share/fonts/TTF/fa-regular-400.ttf",
        );
        Self::add_font(
            fonts,
            "fa-solid",
            "Font Awesome",
            "/usr/share/fonts/TTF/fa-solid-900.ttf",
        );
        cc.egui_ctx.set_fonts(fonts.clone());
        Self {
            is_first_run: true,
            rx,
            current_focus_history: None,
            previous_focus_history: None,
        }
    }
    pub fn new_label(ui: &mut Ui, text: &String, font: &egui::FontId) -> egui::Response {
        ui.add(Label::new(egui::RichText::new(text).font(font.clone())).wrap())
    }
    pub fn render_ui(&self, ctx: &eframe::egui::Context, windows: &[HashMap<String, String>]) {
        ctx.all_styles_mut(|style| {
            style.visuals.panel_fill = Color32::from_hex("#1E1E2E").expect("color from hex");
            style.spacing.default_area_size = Vec2::new(200.0, 1000.0);
            // style.debug.debug_on_hover = true;
        });
        let spacing = 8.0;
        let mut sum_of_heights = 0.0;
        let caskaydia_font_id = egui::FontId {
            size: FONT_SIZE,
            family: FontFamily::Name("Caskaydia Cove".into()),
        };
        let fa_font_id = egui::FontId {
            size: FONT_SIZE,
            family: FontFamily::Name("Font Awesome".into()),
        };
        egui::CentralPanel::default().show(ctx, |ui| {
            if self.is_first_run {
                self.hide_our_window();
            }
            ui.vertical(|ui| {
                for (index, win) in windows.iter().enumerate() {
                    ui.style_mut().visuals.widgets.noninteractive.bg_stroke = Stroke {
                        width: 0.0,
                        color: Color32::from_hex("#6c7086").expect("color from hex"),
                    };
                    ui.style_mut().visuals.widgets.noninteractive.fg_stroke = Stroke {
                        width: 0.0,
                        color: Color32::from_hex("#6c7086").expect("color from hex"),
                    };
                    let group = ui
                        .group(|ui| {
                            ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                                let label_response = Self::new_label(
                                    ui,
                                    win.get("class").expect("qtile sends correct format"),
                                    &caskaydia_font_id,
                                );
                                sum_of_heights += label_response.rect.height();

                                let mut name =
                                    win.get("name").expect("qtile sends correct format").clone();
                                if name.len() > 31 {
                                    let upto = name
                                        .char_indices()
                                        .map(|(i, _)| i)
                                        .nth(30)
                                        .unwrap_or(name.len());
                                    name.truncate(upto);
                                }
                                let label_response = Self::new_label(ui, &name, &caskaydia_font_id);
                                sum_of_heights += label_response.rect.height();

                                let label_response = Self::new_label(
                                    ui,
                                    win.get("group_name").expect("qtile sends correct format"),
                                    &caskaydia_font_id,
                                );
                                sum_of_heights += label_response.rect.height();

                                let label_response = Self::new_label(
                                    ui,
                                    win.get("group_label").expect("qtile sends correct format"),
                                    &fa_font_id,
                                );
                                sum_of_heights += label_response.rect.height();
                            });
                        })
                        .response
                        .interact(egui::Sense::click())
                        .on_hover_cursor(egui::CursorIcon::Crosshair);

                    if index != 0 {
                        if !group.hovered() {
                            ui.painter().rect_stroke(
                                group.rect,
                                Rounding::same(10.0),
                                Stroke {
                                    width: 3.0,
                                    color: Color32::from_hex("#313244").expect("color from hex"), // Highlight color
                                },
                            );
                        } else {
                            ui.painter().rect_stroke(
                                group.rect,
                                Rounding::same(10.0),
                                Stroke {
                                    width: 3.0,
                                    color: Color32::from_hex("#b4befe").expect("color from hex"),
                                },
                            );
                        }
                    } else if !group.hovered() {
                        ui.painter().rect_stroke(
                            group.rect,
                            Rounding::same(10.0),
                            Stroke {
                                width: 3.0,
                                color: Color32::from_hex("#b4befe").expect("color from hex"),
                            },
                        );
                    } else {
                        ui.painter().rect_stroke(
                            group.rect,
                            Rounding::same(10.0),
                            Stroke {
                                width: 3.0,
                                color: Color32::from_hex("#313244").expect("color from hex"),
                            },
                        );
                    }
                    if group.clicked() {
                        self.focus_window(win);
                        self.hide_our_window();
                    };
                    if index < windows.len() - 1 {
                        ui.add_space(spacing);
                        sum_of_heights += spacing;
                    }
                }
            });
            // available_height = ui.available_height();
            // sum_of_heights = MAX_HEIGHT - ui.available_height();
        });
        let width = (200.0 as i32).to_string();
        let height = sum_of_heights.min(MAX_HEIGHT) - 5.0;
        let height = (height as i32).to_string();
        self.place_our_window(width, height);
        // ctx.request_repaint();
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

    pub fn get_our_window_id(&self) -> String {
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

        let wid = response
            .iter()
            .filter(|map| map.get("name") == Some(&"qalttab".to_string()))
            .cloned()
            .collect::<Vec<HashMap<String, String>>>();
        let wid = wid.first().unwrap().get("wid").unwrap();
        wid.clone()
    }

    pub fn hide_our_window(&self) {
        let wid = self.get_our_window_id();
        let _response = InteractiveCommandClient::call(
            Some(vec!["window".to_owned(), wid.to_owned()]),
            Some("hide".into()),
            Some(vec![]),
            false,
        );
    }

    pub fn place_our_window(&self, width: String, height: String) {
        let wid = self.get_our_window_id();

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
