use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use crate::config::{Config, Font, Orientation};
use anyhow::bail;
use egui::{
    Color32, CornerRadius, FontData, FontDefinitions, FontFamily, Image, ImageSource, Label, Sense,
    Stroke, Ui, Vec2,
};
use freedesktop_icons::lookup;
use qtile_client_lib::utils::client::{CallResult, CommandQuery, QtileClient};
use serde_json::Value;
use sysinfo::{Pid, System};
use tokio::sync::mpsc::UnboundedReceiver;

use std::panic::{RefUnwindSafe, UnwindSafe};

pub trait QtileClientTrait: Send + Sync + RefUnwindSafe + UnwindSafe {
    fn call(&self, query: CommandQuery) -> anyhow::Result<CallResult>;
}

impl QtileClientTrait for QtileClient {
    fn call(&self, query: CommandQuery) -> anyhow::Result<CallResult> {
        self.call(query)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Response {
    pub message_type: MessageType,
    pub windows: Vec<HashMap<String, String>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AppEvent {
    AltReleased,
    UnixSocketMsg(Response),
    // Add more as needed
}

pub struct AsyncApp {
    rx: Option<UnboundedReceiver<AppEvent>>,
    current_focus_history: Option<AppEvent>,
    previous_focus_history: Option<AppEvent>,
    config: Config,
    qtile_client: Box<dyn QtileClientTrait>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum MessageType {
    ClientFocus,
    CycleWindows,
    None,
}

impl TryFrom<&str> for MessageType {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "client_focus" => Ok(MessageType::ClientFocus),
            "cycle_windows" => Ok(MessageType::CycleWindows),
            _ => bail!("MessageType {value} not known"),
        }
    }
}

impl AsyncApp {
    pub fn add_font(fonts: &mut FontDefinitions, family: String, font: &Font) {
        let font_path = Path::new(&font.path);
        if Path::exists(font_path) {
            let bytes = std::fs::read(font_path).unwrap().clone();
            fonts
                .font_data
                .insert(font.name.to_owned(), FontData::from_owned(bytes).into());
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
    pub fn init_fonts(fonts: &mut FontDefinitions) -> Config {
        let cfg = Self::load_config();
        Self::register_fonts(fonts, &cfg);
        cfg
    }

    pub fn load_config() -> Config {
        confy::load("qalttab", Some("config")).unwrap_or_else(|e| {
            log::debug!("Failed to load config: {e}");
            Config::default()
        })
    }

    pub fn register_fonts(fonts: &mut FontDefinitions, cfg: &Config) {
        let (cfg_family, cfg_text_fonts) =
            (&cfg.fonts.text_font.family_name, &cfg.fonts.text_font.fonts);
        Self::add_font_family(fonts, cfg_family.clone());
        for font in cfg_text_fonts.iter() {
            Self::add_font(fonts, cfg_family.clone(), font);
        }
        let (cfg_icon_family, cfg_icon_fonts) =
            (&cfg.fonts.icon_font.family_name, &cfg.fonts.icon_font.fonts);
        Self::add_font_family(fonts, cfg_icon_family.clone());
        for font in cfg_icon_fonts.iter() {
            Self::add_font(fonts, cfg_icon_family.clone(), font);
        }
    }

    pub fn new(
        cc: &eframe::CreationContext<'_>,
        rx: Option<UnboundedReceiver<AppEvent>>,
        qtile_client: Box<dyn QtileClientTrait>,
    ) -> Self {
        let mut fonts = FontDefinitions::default();
        let config = Self::init_fonts(&mut fonts);
        cc.egui_ctx.set_fonts(fonts);
        egui_extras::install_image_loaders(&cc.egui_ctx);
        Self {
            rx,
            current_focus_history: None,
            previous_focus_history: None,
            config,
            qtile_client,
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
            Image::new(ImageSource::Uri(format!("file://{path}").into())).max_size(Vec2 {
                x: self.config.icons.visible_icon_size,
                y: self.config.icons.visible_icon_size,
            }),
        )
        .interact(Sense::hover())
    }

    pub fn resolve_icon_path(&self, wm_class: &str) -> String {
        let lowercase_wm_class = wm_class.to_lowercase();
        let path = self.find_icon(&lowercase_wm_class);
        match path {
            Some(p) => match p.to_str() {
                Some(p) => p.to_string(),
                None => self.config.icons.default_icon.clone(),
            },
            None => match self.find_icon(wm_class) {
                Some(p) => match p.to_str() {
                    Some(p) => p.to_string(),
                    None => self.config.icons.default_icon.clone(),
                },
                None => self.config.icons.default_icon.clone(),
            },
        }
    }

    pub fn window_icon(&self, ui: &mut Ui, win: &HashMap<String, String>) -> egui::Response {
        let wm_class = win.get("class").expect("qtile sends correct format");
        let path = self.resolve_icon_path(wm_class);
        self.new_image(ui, &path)
    }

    pub fn new_label(&self, ui: &mut Ui, text: &String, font: &egui::FontId) -> egui::Response {
        ui.add(Label::new(egui::RichText::new(text).font(font.clone())).wrap())
            .interact(Sense::hover())
    }

    pub fn truncate_window_name(name: &str) -> String {
        if name.chars().count() > 31 {
            name.chars().take(31).collect()
        } else {
            name.to_string()
        }
    }

    pub fn window_name(
        &self,
        ui: &mut Ui,
        text_font_id: &egui::FontId,
        win: &HashMap<String, String>,
    ) -> egui::Response {
        let name = win.get("name").expect("qtile sends correct format");
        let truncated_name = Self::truncate_window_name(name);
        self.new_label(ui, &truncated_name, text_font_id)
    }

    pub fn calculate_window_dimensions(
        &self,
        ctx: &eframe::egui::Context,
        sum_of_heights: f32,
    ) -> (String, String) {
        let width = (self.config.sizes.window_size.width as i32).to_string();
        let height = sum_of_heights.min(self.config.sizes.window_size.height)
            + ctx.style().spacing.window_margin.top as f32
            + ctx.style().spacing.window_margin.bottom as f32
            + self.config.sizes.group_rect_stroke_width;
        let height = (height as i32).to_string();
        (width, height)
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
            if self.config.ui.orientation == Orientation::Vertical {
                sum_of_heights = self.render_vertical(ui, windows, &text_font_id, &icon_font_id);
            } else {
                // TODO: actual horizontal rendering
                ui.label("Horizontal layout not yet implemented");
            }
        });
        let (width, height) = self.calculate_window_dimensions(ctx, sum_of_heights);
        self.place_our_window(width, height);
        ctx.request_repaint();
    }

    pub fn handle_window_click(&self, win: &HashMap<String, String>) {
        self.focus_window(win);
        self.hide_our_window();
    }

    pub fn handle_window_middle_click(&self, win: &HashMap<String, String>) {
        self.close_window(win);
    }

    pub fn render_window_group(
        &self,
        ui: &mut Ui,
        index: usize,
        win: &HashMap<String, String>,
        text_font_id: &egui::FontId,
        icon_font_id: &egui::FontId,
    ) -> egui::Response {
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
                ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                    for item in self.config.ui.items.iter() {
                        match item {
                            crate::config::UiItem::Icon => {
                                self.window_icon(ui, win);
                            }
                            crate::config::UiItem::Name => {
                                self.window_name(ui, text_font_id, win);
                            }
                            crate::config::UiItem::GroupName => {
                                self.new_label(
                                    ui,
                                    win.get("group_name").expect("qtile sends correct format"),
                                    text_font_id,
                                );
                            }
                            crate::config::UiItem::GroupLabel => {
                                self.new_label(
                                    ui,
                                    win.get("group_label").expect("qtile sends correct format"),
                                    icon_font_id,
                                );
                            }
                        }
                    }
                });
            })
            .response
            .interact(egui::Sense::click())
            .on_hover_cursor(egui::CursorIcon::Crosshair);

        if index != 0 {
            if !group.hovered() {
                ui.painter().rect_stroke(
                    group.rect,
                    CornerRadius::same(10),
                    Stroke {
                        width: self.config.sizes.group_rect_stroke_width,
                        color: Color32::from_hex(self.config.colors.normal_group_color.as_str())
                            .expect("color from hex"),
                    },
                    egui::StrokeKind::Middle,
                );
            } else {
                ui.painter().rect_stroke(
                    group.rect,
                    CornerRadius::same(10),
                    Stroke {
                        width: self.config.sizes.group_rect_stroke_width,
                        color: Color32::from_hex(self.config.colors.group_hover_color.as_str())
                            .expect("color from hex"),
                    },
                    egui::StrokeKind::Middle,
                );
            }
        } else if !group.hovered() {
            ui.painter().rect_stroke(
                group.rect,
                CornerRadius::same(10),
                Stroke {
                    width: self.config.sizes.group_rect_stroke_width,
                    color: Color32::from_hex(self.config.colors.group_hover_color.as_str())
                        .expect("color from hex"),
                },
                egui::StrokeKind::Middle,
            );
        } else {
            ui.painter().rect_stroke(
                group.rect,
                CornerRadius::same(10),
                Stroke {
                    width: self.config.sizes.group_rect_stroke_width,
                    color: Color32::from_hex(self.config.colors.normal_group_color.as_str())
                        .expect("color from hex"),
                },
                egui::StrokeKind::Middle,
            );
        }
        if group.middle_clicked() {
            self.handle_window_middle_click(win);
        }
        if group.clicked() {
            self.handle_window_click(win);
        };
        group
    }

    pub fn render_vertical(
        &self,
        ui: &mut Ui,
        windows: &[HashMap<String, String>],
        text_font_id: &egui::FontId,
        icon_font_id: &egui::FontId,
    ) -> f32 {
        let mut sum_of_heights = 0.0;
        ui.vertical(|ui| {
            for (index, win) in windows.iter().enumerate() {
                let group = self.render_window_group(ui, index, win, text_font_id, icon_font_id);
                sum_of_heights += group.rect.height();
                if index < windows.len() - 1 {
                    ui.add_space(self.config.sizes.group_spacing);
                }
            }
        })
        .response
        .rect
        .height()
    }

    pub fn focus_window(&self, win: &HashMap<String, String>) {
        let wid = win
            .get("id")
            .unwrap_or_else(|| panic!("qtile sends correct format {:?}", win.get("id")))
            .to_string();
        //  qtile.current_screen.set_group(next_window.group)
        let _ = self
            .qtile_client
            .call(
                CommandQuery::new()
                    .function("eval".into())
                    .args(vec![format!(
                        "self.current_screen.set_group(self.windows_map[{}].group)",
                        wid
                    )]),
            );
        //  next_window.group.focus(next_window, warp=False)  # type: ignore
        let _ = self.qtile_client.call(
            CommandQuery::new()
                .object(vec!["window".to_owned(), wid.clone()])
                .function("focus".into()),
        );
        //  next_window.bring_to_front()
        let _ = self.qtile_client.call(
            CommandQuery::new()
                .object(vec!["window".to_owned(), wid])
                .function("bring_to_front".into()),
        );
    }

    pub fn parse_window_id_response(response: CallResult) -> Option<String> {
        let CallResult::Value(response) = response else {
            return None;
        };

        let response = serde_json::from_value::<Vec<Value>>(response)
            .ok()?
            .get(1)?
            .clone();

        let response = serde_json::from_value::<String>(response).ok()?;

        let response = serde_json::from_str::<Vec<HashMap<String, String>>>(&response).ok()?;

        response
            .iter()
            .filter(|map| map.get("name") == Some(&"qalttab".to_string()))
            .find_map(|map| map.get("wid").cloned())
    }

    pub fn get_our_window_id(&self) -> Option<String> {
        let response = self
            .qtile_client
            .call(CommandQuery::new().function("eval".into()).args(vec![
                r#"__import__("json").dumps(
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
                .into(),
            ]))
            .ok()?;

        Self::parse_window_id_response(response)
    }

    pub fn hide_our_window(&self) {
        let wid = self.get_our_window_id();
        match wid {
            Some(wid) => {
                let _response = self.qtile_client.call(
                    CommandQuery::new()
                        .object(vec!["window".to_owned(), wid.to_owned()])
                        .function("hide".into()),
                );
            }
            None => log::debug!("Could not hide window"),
        }
    }

    pub fn place_our_window(&self, width: String, height: String) {
        let wid = self.get_our_window_id();

        match wid {
            Some(wid) => {
                let _response = self.qtile_client.call(
                    CommandQuery::new()
                        .object(vec!["window".to_owned(), wid.to_owned()])
                        .function("set_size_floating".into())
                        .args(vec![width, height]),
                );

                let _response = self.qtile_client.call(
                    CommandQuery::new()
                        .object(vec!["window".to_owned(), wid.to_owned()])
                        .function("center".into()),
                );
            }
            None => log::debug!("Could not place our window"),
        }
    }

    fn close_window(&self, win: &HashMap<String, String>) {
        let wid = win.get("id").expect("qtile sends correct format");
        let _response = self.qtile_client.call(
            CommandQuery::new()
                .object(vec!["window".to_owned(), wid.to_owned()])
                .function("kill".into()),
        );
    }

    pub fn handle_event(&mut self, event: AppEvent) -> Option<egui::ViewportCommand> {
        self.current_focus_history = Some(event.clone());

        if let AppEvent::UnixSocketMsg(ref current_focus_history) = event {
            match current_focus_history.message_type {
                MessageType::CycleWindows => {
                    if let Some(AppEvent::UnixSocketMsg(ref previous)) = self.previous_focus_history
                    {
                        if current_focus_history.windows != previous.windows {
                            self.previous_focus_history = Some(event);
                            return Some(egui::ViewportCommand::Visible(true));
                        }
                    } else {
                        self.previous_focus_history = Some(event);
                        return Some(egui::ViewportCommand::Visible(true));
                    }
                }
                MessageType::ClientFocus => {
                    self.previous_focus_history = Some(event);
                    return Some(egui::ViewportCommand::Visible(false));
                }
                MessageType::None => {}
            }
        }
        None
    }

    pub fn process_events(&mut self, ctx: &egui::Context) {
        let mut rx = self.rx.take();
        if let Some(ref mut receiver) = rx {
            while let Ok(message) = receiver.try_recv() {
                if let Some(cmd) = self.handle_event(message) {
                    ctx.send_viewport_cmd(cmd);
                }
            }
        };
        self.rx = rx;
    }
}

impl eframe::App for AsyncApp {
    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) {
        // Check for new messages and display them
        self.process_events(ctx);

        if let Some(AppEvent::UnixSocketMsg(current)) = &self.current_focus_history {
            self.render_ui(ctx, &current.windows);
        }
        ctx.request_repaint();
    }
}

pub fn is_qalttab_running() -> bool {
    let s = System::new_all();
    let qalttab_processes_parents = s
        .processes_by_exact_name("qalttab".as_ref())
        .map(|p| p.parent());
    let mut qalttab_processes_vec = qalttab_processes_parents.collect::<Vec<Option<Pid>>>();
    qalttab_processes_vec.sort();
    qalttab_processes_vec.dedup();
    qalttab_processes_vec.len() >= 4
}

pub fn run_ui(rx: UnboundedReceiver<AppEvent>) -> anyhow::Result<()> {
    if is_qalttab_running() {
        bail!("qalttab already running");
    };
    match eframe::run_native(
        "qalttab",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder {
                title: Some("qalttab".to_owned()),
                app_id: Some("qalttab".to_owned()),
                // resizable: Some(false),
                // transparent: Some(true),
                decorations: Some(false),
                visible: Some(false),
                taskbar: Some(false),
                title_shown: Some(false),
                window_level: Some(egui::WindowLevel::AlwaysOnTop),
                ..egui::ViewportBuilder::default()
            },
            // event_loop_builder: Some(Box::new(|elb| {})),
            // window_builder: Some(Box::new(|vb| {})),
            ..eframe::NativeOptions::default()
        },
        Box::new(|cc| {
            Ok(Box::<AsyncApp>::new(AsyncApp::new(
                cc,
                Some(rx),
                Box::new(QtileClient::new(false)),
            )))
        }),
    ) {
        Ok(()) => Ok(()),
        Err(e) => bail!("eframe crashed: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::FontDefinitions;

    struct MockQtileClient {
        response: anyhow::Result<CallResult>,
    }
    impl QtileClientTrait for MockQtileClient {
        fn call(&self, _query: CommandQuery) -> anyhow::Result<CallResult> {
            match &self.response {
                Ok(r) => Ok(r.clone()),
                Err(e) => anyhow::bail!("{e}"),
            }
        }
    }

    #[test]
    fn test_response_logic() {
        let mut windows = Vec::new();
        let mut win = HashMap::new();
        win.insert("id".to_string(), "123".to_string());
        windows.push(win);

        let r1 = Response {
            message_type: MessageType::ClientFocus,
            windows: windows.clone(),
        };
        let r2 = r1.clone();
        assert_eq!(r1, r2);

        let r_empty = Response {
            message_type: MessageType::None,
            windows: vec![],
        };
        assert!(r_empty.windows.is_empty());
    }

    #[test]
    fn test_message_type_logic() {
        assert_eq!(
            MessageType::try_from("client_focus").unwrap(),
            MessageType::ClientFocus
        );
        assert_eq!(
            MessageType::try_from("cycle_windows").unwrap(),
            MessageType::CycleWindows
        );
        assert!(MessageType::try_from("unknown").is_err());
        assert_ne!(MessageType::ClientFocus, MessageType::CycleWindows);
        assert_eq!(format!("{:?}", MessageType::ClientFocus), "ClientFocus");
    }

    #[test]
    fn test_app_event_logic() {
        let r = Response {
            message_type: MessageType::ClientFocus,
            windows: vec![],
        };
        let e1 = AppEvent::UnixSocketMsg(r.clone());
        let e2 = AppEvent::UnixSocketMsg(r);
        if let (AppEvent::UnixSocketMsg(r1), AppEvent::UnixSocketMsg(r2)) = (e1, e2) {
            assert_eq!(r1, r2);
        }
        assert!(matches!(AppEvent::AltReleased, AppEvent::AltReleased));
    }

    #[test]
    fn test_ui_utils() {
        assert_eq!(AsyncApp::truncate_window_name("Short"), "Short");
        assert_eq!(
            AsyncApp::truncate_window_name(&"A".repeat(50))
                .chars()
                .count(),
            31
        );

        let mut fonts = FontDefinitions::default();
        AsyncApp::add_font_family(&mut fonts, "Test".into());
        assert!(
            fonts
                .families
                .contains_key(&egui::FontFamily::Name("Test".into()))
        );

        // Test non-existent font loading
        let family = "Test Family".to_string();
        let font = Font::new("Fake Font", "/tmp/non-existent-font-file-12345");
        AsyncApp::add_font(&mut fonts, family, &font);
    }

    #[test]
    fn test_add_font_success() {
        let mut fonts = FontDefinitions::default();
        let family = "Test".to_string();
        AsyncApp::add_font_family(&mut fonts, family.clone());

        // Create a temporary "font" file in current dir
        let path = "./dummy_font.ttf";
        std::fs::write(path, b"dummy font data").unwrap();

        let font = Font::new("Dummy Font", path);
        AsyncApp::add_font(&mut fonts, family.clone(), &font);

        assert!(fonts.font_data.contains_key("Dummy Font"));
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn test_config_loading_and_registration() {
        let mut fonts = FontDefinitions::default();
        let config = AsyncApp::load_config();
        AsyncApp::register_fonts(&mut fonts, &config);
        assert!(fonts.families.contains_key(&egui::FontFamily::Name(
            config.fonts.text_font.family_name.into()
        )));
    }

    #[test]
    fn test_init_fonts_full() {
        let mut fonts = FontDefinitions::default();
        let config = AsyncApp::init_fonts(&mut fonts);
        assert!(fonts.families.len() >= 2);
        assert_eq!(config.fonts.text_font.family_name, "Caskaydia Cove");
    }

    #[test]
    fn test_handle_event_transitions() {
        let config = Config::default();
        let mut app = AsyncApp {
            rx: None,
            current_focus_history: None,
            previous_focus_history: None,
            config,
            qtile_client: Box::new(MockQtileClient {
                response: Ok(CallResult::Text("ok".into())),
            }),
        };

        let event = AppEvent::UnixSocketMsg(Response {
            message_type: MessageType::CycleWindows,
            windows: vec![HashMap::from([("id".into(), "1".into())])],
        });

        // First event should make it visible
        let cmd = app.handle_event(event.clone());
        assert!(matches!(cmd, Some(egui::ViewportCommand::Visible(true))));

        // Same event again should NOT return a command (idempotent UI)
        let cmd = app.handle_event(event);
        assert!(cmd.is_none());

        // ClientFocus event should hide it
        let focus_event = AppEvent::UnixSocketMsg(Response {
            message_type: MessageType::ClientFocus,
            windows: vec![],
        });
        let cmd = app.handle_event(focus_event);
        assert!(matches!(cmd, Some(egui::ViewportCommand::Visible(false))));

        // AltReleased event
        let alt_event = AppEvent::AltReleased;
        let cmd = app.handle_event(alt_event);
        assert!(cmd.is_none());

        // MessageType::None event
        let none_event = AppEvent::UnixSocketMsg(Response {
            message_type: MessageType::None,
            windows: vec![],
        });
        let cmd = app.handle_event(none_event);
        assert!(cmd.is_none());
    }

    #[test]
    fn test_handle_event_complex() {
        let config = Config::default();
        let mut app = AsyncApp {
            rx: None,
            current_focus_history: None,
            previous_focus_history: None,
            config,
            qtile_client: Box::new(MockQtileClient {
                response: Ok(CallResult::Text("ok".into())),
            }),
        };

        // Transition from None to CycleWindows
        let event = AppEvent::UnixSocketMsg(Response {
            message_type: MessageType::CycleWindows,
            windows: vec![HashMap::from([("id".into(), "1".into())])],
        });
        assert!(app.handle_event(event).is_some());

        // Transition from CycleWindows to ClientFocus
        let focus_event = AppEvent::UnixSocketMsg(Response {
            message_type: MessageType::ClientFocus,
            windows: vec![],
        });
        assert!(app.handle_event(focus_event).is_some());

        // Test AltReleased
        assert!(app.handle_event(AppEvent::AltReleased).is_none());

        // Test MessageType::None
        let none_event = AppEvent::UnixSocketMsg(Response {
            message_type: MessageType::None,
            windows: vec![],
        });
        assert!(app.handle_event(none_event).is_none());
    }

    #[test]
    fn test_calculate_window_dimensions_bounds() {
        let config = Config::default();
        let app = AsyncApp {
            rx: None,
            current_focus_history: None,
            previous_focus_history: None,
            config,
            qtile_client: Box::new(MockQtileClient {
                response: Ok(CallResult::Text("ok".into())),
            }),
        };
        let ctx = egui::Context::default();

        // Test height clamping
        let (_, h) = app.calculate_window_dimensions(&ctx, 5000.0);
        let h_val: f32 = h.parse().unwrap();
        assert!(h_val >= app.config.sizes.window_size.height);
    }

    #[test]
    fn test_find_icon_none() {
        let config = Config::default();
        let app = AsyncApp {
            rx: None,
            current_focus_history: None,
            previous_focus_history: None,
            config,
            qtile_client: Box::new(MockQtileClient {
                response: Ok(CallResult::Text("ok".into())),
            }),
        };
        let icon = app.find_icon("non-existent-app-12345");
        assert!(icon.is_none());
    }

    #[test]
    fn test_ui_helpers() {
        let config = Config::default();
        let app = AsyncApp {
            rx: None,
            current_focus_history: None,
            previous_focus_history: None,
            config,
            qtile_client: Box::new(MockQtileClient {
                response: Ok(CallResult::Text("ok".into())),
            }),
        };

        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let mut win = HashMap::new();
                win.insert("name".to_string(), "Test Window".to_string());
                let font_id = egui::FontId::default();

                // Exercise window_name and new_label
                app.window_name(ui, &font_id, &win);
                app.new_label(ui, &"Label".to_string(), &font_id);
            });
        });
    }

    #[test]
    fn test_render_ui_logic() {
        let config = Config::default();
        let mock_json = serde_json::json!([true, "[{\"name\": \"qalttab\", \"wid\": \"12345\"}]"]);
        let app = AsyncApp {
            rx: None,
            current_focus_history: None,
            previous_focus_history: None,
            config,
            qtile_client: Box::new(MockQtileClient {
                response: Ok(CallResult::Value(mock_json)),
            }),
        };

        let ctx = egui::Context::default();
        let mut fonts = egui::FontDefinitions::default();
        AsyncApp::init_fonts(&mut fonts);
        ctx.set_fonts(fonts);

        let mut win = HashMap::new();
        win.insert("id".to_string(), "1".to_string());
        win.insert("name".to_string(), "win1".to_string());
        win.insert("class".to_string(), "class1".to_string());
        win.insert("group_name".to_string(), "g1".to_string());
        win.insert("group_label".to_string(), "L1".to_string());

        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            app.render_ui(ctx, &[win.clone()]);
        });
    }

    #[test]
    fn test_render_ui_horizontal() {
        let mut config = Config::default();
        config.ui.orientation = Orientation::Horizontal;
        let mock_json = serde_json::json!([true, "[]"]);
        let app = AsyncApp {
            rx: None,
            current_focus_history: None,
            previous_focus_history: None,
            config,
            qtile_client: Box::new(MockQtileClient {
                response: Ok(CallResult::Value(mock_json)),
            }),
        };

        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            app.render_ui(ctx, &[]);
        });
    }

    #[test]
    fn test_process_events_loop() {
        let config = Config::default();
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let mut app = AsyncApp {
            rx: Some(rx),
            current_focus_history: None,
            previous_focus_history: None,
            config,
            qtile_client: Box::new(MockQtileClient {
                response: Ok(CallResult::Text("ok".into())),
            }),
        };

        let event = AppEvent::AltReleased;
        tx.send(event).unwrap();

        let ctx = egui::Context::default();
        app.process_events(&ctx);
        assert!(app.current_focus_history.is_some());
    }

    #[test]
    fn test_is_qalttab_running() {
        // Just ensure it doesn't panic and returns a boolean
        let _ = is_qalttab_running();
    }

    #[test]
    fn test_resolve_icon_path() {
        let config = Config::default();
        let app = AsyncApp {
            rx: None,
            current_focus_history: None,
            previous_focus_history: None,
            config,
            qtile_client: Box::new(MockQtileClient {
                response: Ok(CallResult::Text("ok".into())),
            }),
        };
        // Fallback case (non-existent)
        let path = app.resolve_icon_path("non-existent-app-12345");
        assert_eq!(path, app.config.icons.default_icon);

        // Lowercase hit
        let _ = app.resolve_icon_path("ALACRITTY");

        // Exact hit
        let _ = app.resolve_icon_path("Alacritty");
    }

    #[test]
    fn test_window_icon_logic() {
        let config = Config::default();
        let app = AsyncApp {
            rx: None,
            current_focus_history: None,
            previous_focus_history: None,
            config,
            qtile_client: Box::new(MockQtileClient {
                response: Ok(CallResult::Text("ok".into())),
            }),
        };

        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let mut win = HashMap::new();
                win.insert("class".to_string(), "Alacritty".to_string());

                // This will exercise window_icon branches
                app.window_icon(ui, &win);
            });
        });
    }

    #[test]
    fn test_render_window_group_logic() {
        let config = Config::default();
        let app = AsyncApp {
            rx: None,
            current_focus_history: None,
            previous_focus_history: None,
            config,
            qtile_client: Box::new(MockQtileClient {
                response: Ok(CallResult::Text("ok".into())),
            }),
        };

        let ctx = egui::Context::default();
        let mut fonts = egui::FontDefinitions::default();
        AsyncApp::init_fonts(&mut fonts);
        ctx.set_fonts(fonts);

        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let mut win = HashMap::new();
                win.insert("id".to_string(), "1".to_string());
                win.insert("name".to_string(), "win1".to_string());
                win.insert("class".to_string(), "class1".to_string());
                win.insert("group_name".to_string(), "g1".to_string());
                win.insert("group_label".to_string(), "L1".to_string());

                let font_id = egui::FontId::default();
                // Test index 0
                app.render_window_group(ui, 0, &win, &font_id, &font_id);
                // Test index > 0
                app.render_window_group(ui, 1, &win, &font_id, &font_id);
            });
        });
    }

    #[test]
    fn test_qtile_interaction_methods() {
        let config = Config::default();
        let mock_json = serde_json::json!([true, "[{\"name\": \"qalttab\", \"wid\": \"12345\"}]"]);
        let app = AsyncApp {
            rx: None,
            current_focus_history: None,
            previous_focus_history: None,
            config,
            qtile_client: Box::new(MockQtileClient {
                response: Ok(CallResult::Value(mock_json)),
            }),
        };

        assert_eq!(app.get_our_window_id(), Some("12345".into()));
        app.hide_our_window();
        app.place_our_window("100".into(), "200".into());

        let mut win = HashMap::new();
        win.insert("id".into(), "1".into());
        app.focus_window(&win);
        app.close_window(&win);

        // Test extracted handlers
        app.handle_window_click(&win);
        app.handle_window_middle_click(&win);
    }

    #[test]
    fn test_handle_event_cycle_no_previous() {
        let config = Config::default();
        let mut app = AsyncApp {
            rx: None,
            current_focus_history: None,
            previous_focus_history: None,
            config,
            qtile_client: Box::new(MockQtileClient {
                response: Ok(CallResult::Text("ok".into())),
            }),
        };

        let event = AppEvent::UnixSocketMsg(Response {
            message_type: MessageType::CycleWindows,
            windows: vec![HashMap::from([("id".into(), "1".into())])],
        });

        // No previous focus history branch
        assert!(app.handle_event(event.clone()).is_some());

        // Same windows as previous history branch
        assert!(app.handle_event(event).is_none());
    }

    #[test]
    fn test_parse_window_id_response() {
        // Valid case
        let mock_json = serde_json::json!([true, "[{\"name\": \"qalttab\", \"wid\": \"12345\"}]"]);
        let res = AsyncApp::parse_window_id_response(CallResult::Value(mock_json));
        assert_eq!(res, Some("12345".into()));

        // Invalid CallResult variant
        let res = AsyncApp::parse_window_id_response(CallResult::Text("ok".into()));
        assert!(res.is_none());

        // Malformed JSON
        let res = AsyncApp::parse_window_id_response(CallResult::Value(serde_json::json!([])));
        assert!(res.is_none());
    }

    #[test]
    fn test_focus_window_panic_on_missing_id() {
        let config = Config::default();
        let app = AsyncApp {
            rx: None,
            current_focus_history: None,
            previous_focus_history: None,
            config,
            qtile_client: Box::new(MockQtileClient {
                response: Ok(CallResult::Text("ok".into())),
            }),
        };
        let win = HashMap::new();
        let res = std::panic::catch_unwind(move || {
            app.focus_window(&win);
        });
        assert!(res.is_err());
    }
}
