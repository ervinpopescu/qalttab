use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use crate::config::{Config, Font, Orientation};
use anyhow::bail;
use egui::{
    Color32, FontData, FontDefinitions, FontFamily, Image, ImageSource, Label, Sense, Stroke, Ui,
    Vec2,
};
use freedesktop_icons::lookup;
use qtile_client_lib::utils::client::{CallResult, InteractiveCommandClient};
use serde_json::Value;
use sysinfo::{Pid, System};
use tokio::sync::mpsc::unbounded_channel;

/// Abstraction over the Qtile IPC client.
///
/// The current impl (`IccQtileClient`) wraps the synchronous `QtileClient`
/// from the framing-support API and is intended to be called from inside
/// `tokio::task::spawn_blocking`.
///
/// TODO(#186): once the framing protocol is stable, make this trait async
/// and remove the `spawn_blocking` wrappers.
pub trait QtileClientTrait: Send + Sync {
    fn call(
        &self,
        object: Option<Vec<String>>,
        function: Option<String>,
        args: Option<Vec<String>>,
    ) -> anyhow::Result<serde_json::Value>;
}

/// Production implementation using `InteractiveCommandClient` from qtile-cmd-client main.
pub struct IccQtileClient;

impl QtileClientTrait for IccQtileClient {
    fn call(
        &self,
        object: Option<Vec<String>>,
        function: Option<String>,
        args: Option<Vec<String>>,
    ) -> anyhow::Result<serde_json::Value> {
        match InteractiveCommandClient::call(object, function, args, false)? {
            CallResult::Value(v) => Ok(v),
            CallResult::Text(t) => Ok(serde_json::Value::String(t)),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Response {
    pub message_type: MessageType,
    pub windows: Vec<HashMap<String, String>>,
    pub focus_index: Option<usize>,
}

#[derive(Debug, Clone)]
pub enum AppEvent {
    AltReleased,
    UnixSocketMsg(Response),
    OurWindowId(String),
}

/// Shared state between the tokio event processor and the egui render loop.
#[derive(Default)]
pub struct SharedState {
    pub current_focus_history: Option<Response>,
    pub is_visible: bool,
    pub cached_wid: Option<String>,
    pub last_placed_height: f32,
    pub last_width: i32,
    pub last_height: i32,
    pub focus_index: usize,
}

pub struct AsyncApp {
    shared: Arc<Mutex<SharedState>>,
    config: Config,
    qtile: Arc<dyn QtileClientTrait>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum MessageType {
    ClientFocus,
    CycleWindows,
    None,
}

/// Truncate `name` to at most `max_chars` characters, respecting Unicode char boundaries.
pub fn truncate_window_name(name: &str, max_chars: usize) -> String {
    if name.chars().count() > max_chars {
        let upto = name
            .char_indices()
            .nth(max_chars)
            .map(|(i, _)| i)
            .unwrap_or(name.len());
        name[..upto].to_string()
    } else {
        name.to_string()
    }
}

impl AsyncApp {
    pub fn add_font(fonts: &mut FontDefinitions, family: &str, font: &Font) {
        let font_path = Path::new(&font.path);
        if Path::exists(font_path) {
            let bytes = std::fs::read(font_path).unwrap();
            fonts
                .font_data
                .insert(font.name.to_owned(), FontData::from_owned(bytes).into());
            fonts
                .families
                .get_mut(&FontFamily::Name(family.into()))
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
    pub fn add_font_family(fonts: &mut FontDefinitions, font_family_name: &str) {
        fonts
            .families
            .extend([(FontFamily::Name(font_family_name.into()), Vec::new())]);
    }
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        Self::new_with_client(cc, Arc::new(IccQtileClient))
    }

    pub fn new_with_client(
        cc: &eframe::CreationContext<'_>,
        qtile: Arc<dyn QtileClientTrait>,
    ) -> Self {
        let (tx, mut rx) = unbounded_channel::<AppEvent>();
        let shared = Arc::new(Mutex::new(SharedState::default()));

        // Spawn listeners
        let tx_socket = tx.clone();
        let ctx_socket = cc.egui_ctx.clone();
        tokio::spawn(async move {
            if let Err(e) = crate::ipc::listen(tx_socket, ctx_socket, None).await {
                log::error!("Unix socket listener error: {e:?}");
            }
        });

        let tx_alt = tx.clone();
        let ctx_alt = cc.egui_ctx.clone();
        tokio::spawn(async move {
            if let Err(e) = crate::qaltd::listen_for_alt_release(tx_alt, ctx_alt).await {
                log::error!("qaltd listener error: {e:?}");
            }
        });

        // Background event processor — runs independently of egui's render loop
        let qtile_bg = Arc::clone(&qtile);
        let shared_clone = shared.clone();
        let ctx_events = cc.egui_ctx.clone();
        tokio::spawn(async move {
            #[allow(unused_assignments)]
            let mut cached_wid: Option<String> = None;
            let mut cycle_active = false;
            #[allow(unused_assignments)]
            let mut pending_hide: Option<tokio::task::JoinHandle<()>> = None;

            // Wait for eframe/winit to initialize to avoid Xwayland/IPC deadlock with Qtile
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;

            // First, discover our WID
            log::debug!("Starting background WID discovery...");
            loop {
                let qtile_c = Arc::clone(&qtile_bg);
                let res = tokio::task::spawn_blocking(move || {
                    qtile_c.call(
                        Some(vec![]),
                        Some("eval".into()),
                        Some(vec![
                            r#"__import__("json").dumps(
                        [
                            {
                                "wid": str(self.windows_map[wid].wid),
                                "name": self.windows_map[wid].name or ""
                            }
                            for wid in self.windows_map
                            if hasattr(self.windows_map[wid], "wid")
                        ]
                    )"#
                            .into(),
                        ]),
                    )
                })
                .await;

                log::debug!("WID discovery result: {:?}", res);
                if let Ok(Ok(val)) = res {
                    let val = match val {
                        Value::Array(mut a) if a.len() == 2 => a.remove(1),
                        _ => val,
                    };
                    if let Ok(json_str) = serde_json::from_value::<String>(val)
                        && let Ok(windows) =
                            serde_json::from_str::<Vec<HashMap<String, String>>>(&json_str)
                        && let Some(win) = windows
                            .iter()
                            .find(|m| m.get("name").map(|s| s.as_str()) == Some("qalttab"))
                        && let Some(wid) = win.get("wid")
                    {
                        log::info!("Discovered our Window ID: {}", wid);
                        cached_wid = Some(wid.clone());
                        shared_clone.lock().unwrap().cached_wid = Some(wid.clone());
                        // Hide off-screen initially
                        let wid_c = wid.clone();
                        let qtile_c = Arc::clone(&qtile_bg);
                        tokio::task::spawn_blocking(move || {
                            let _ = qtile_c.call(
                                Some(vec![]),
                                Some("eval".into()),
                                Some(vec![format!("self.windows_map[{wid_c}].hide()")]),
                            );
                        });
                        break;
                    }
                }
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }

            // Main event loop — processes events from channel
            while let Some(event) = rx.recv().await {
                match event {
                    AppEvent::AltReleased => {
                        log::debug!("AltReleased event | cycle_active={}", cycle_active);
                        if cycle_active {
                            // Schedule hide after delay — cancelled if new CycleWindows arrives
                            if let Some(ref handle) = pending_hide
                                && !handle.is_finished()
                            {
                                continue; // still running
                            }
                            let shared_hide = shared_clone.clone();
                            let wid_hide = cached_wid.clone();
                            let qtile_hide = Arc::clone(&qtile_bg);
                            pending_hide = Some(tokio::spawn(async move {
                                tokio::time::sleep(std::time::Duration::from_millis(150)).await;
                                log::debug!("Delayed hide executing");
                                if let Some(wid) = wid_hide {
                                    let qtile_c = Arc::clone(&qtile_hide);
                                    tokio::task::spawn_blocking(move || {
                                        let _ = qtile_c.call(
                                            Some(vec![]),
                                            Some("eval".into()),
                                            Some(vec![format!("self.windows_map[{wid}].hide()")]),
                                        );
                                    })
                                    .await
                                    .ok();
                                }
                                {
                                    let mut state = shared_hide.lock().unwrap();
                                    state.is_visible = false;
                                    state.current_focus_history = None;
                                    state.last_placed_height = 0.0;
                                }
                                let qtile_c = Arc::clone(&qtile_hide);
                                tokio::task::spawn_blocking(move || {
                                    let _ = qtile_c.call(
                                        Some(vec![]),
                                        Some("fire_user_hook".into()),
                                        Some(vec!["alt_release".to_owned()]),
                                    );
                                })
                                .await
                                .ok();
                            }));
                            cycle_active = false;
                        }
                    }
                    AppEvent::UnixSocketMsg(response) => {
                        log::debug!("UnixSocketMsg: {:?}", response.message_type);
                        match response.message_type {
                            MessageType::CycleWindows => {
                                // Cancel pending hide if user pressed tab again
                                if let Some(handle) = pending_hide.take() {
                                    handle.abort();
                                    log::debug!("Cancelled pending hide");
                                }
                                cycle_active = true;
                                let fi = response.focus_index.unwrap_or(0);
                                {
                                    let mut state = shared_clone.lock().unwrap();
                                    state.is_visible = true;
                                    state.last_placed_height = 0.0;
                                    state.focus_index = fi;
                                    state.current_focus_history = Some(response);
                                }
                                // Unhide + place centered with last known size
                                let (w, h) = {
                                    let st = shared_clone.lock().unwrap();
                                    let w = if st.last_width > 0 {
                                        st.last_width
                                    } else {
                                        300
                                    };
                                    let h = if st.last_height > 0 {
                                        st.last_height
                                    } else {
                                        400
                                    };
                                    (w, h)
                                };
                                if let Some(wid) = cached_wid.clone() {
                                    let qtile_c = Arc::clone(&qtile_bg);
                                    tokio::task::spawn_blocking(move || {
                                        let _ = qtile_c.call(
                                            Some(vec![]),
                                            Some("eval".into()),
                                            Some(vec![format!(
                                                "w = self.windows_map[{wid}]; \
                                                 w.unhide(); \
                                                 s = self.current_screen; \
                                                 x = s.dx + (s.dwidth - {w}) // 2; \
                                                 y = s.dy + (s.dheight - {h}) // 2; \
                                                 w.place(x, y, {w}, {h}, 0, None); \
                                                 w.keep_above(); \
                                                 w.bring_to_front()"
                                            )]),
                                        );
                                    });
                                }
                                ctx_events.request_repaint();
                            }
                            MessageType::ClientFocus => {
                                if cycle_active {
                                    // User clicked a window outside the overlay — cancel cycle and hide
                                    if let Some(handle) = pending_hide.take() {
                                        handle.abort();
                                    }
                                    cycle_active = false;
                                    let mut state = shared_clone.lock().unwrap();
                                    state.is_visible = false;
                                    state.current_focus_history = Some(response);
                                    state.last_placed_height = 0.0;
                                    if let Some(wid) = state.cached_wid.clone() {
                                        drop(state);
                                        let qtile_c = Arc::clone(&qtile_bg);
                                        tokio::task::spawn_blocking(move || {
                                            let _ = qtile_c.call(
                                                Some(vec![]),
                                                Some("eval".into()),
                                                Some(vec![format!(
                                                    "self.windows_map[{wid}].hide()"
                                                )]),
                                            );
                                        });
                                    }
                                } else {
                                    let mut state = shared_clone.lock().unwrap();
                                    state.current_focus_history = Some(response);
                                }
                            }
                            MessageType::None => {}
                        }
                        ctx_events.request_repaint();
                    }
                    AppEvent::OurWindowId(_) => {
                        // Handled during WID discovery above
                    }
                }
            }
        });

        let cfg: Result<Config, confy::ConfyError> = confy::load("qalttab", Some("config"));
        let mut fonts = FontDefinitions::default();
        let config = match cfg {
            Ok(cfg) => {
                log::debug!("Loaded config: {cfg:#?}");
                let (cfg_family, cfg_text_fonts) =
                    (&cfg.fonts.text_font.family_name, &cfg.fonts.text_font.fonts);
                Self::add_font_family(&mut fonts, cfg_family.as_str());
                for font in cfg_text_fonts {
                    Self::add_font(&mut fonts, cfg_family.as_str(), font);
                }
                let (cfg_family, cfg_icon_fonts) =
                    (&cfg.fonts.icon_font.family_name, &cfg.fonts.icon_font.fonts);
                Self::add_font_family(&mut fonts, cfg_family.as_str());
                for font in cfg_icon_fonts {
                    Self::add_font(&mut fonts, cfg_family.as_str(), font);
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
                Self::add_font_family(&mut fonts, def_cfg_family.as_str());
                for font in def_cfg_text_fonts {
                    Self::add_font(&mut fonts, def_cfg_family.as_str(), font);
                }
                let (def_cfg_icon_family, def_cfg_icon_fonts) = (
                    &def_cfg.fonts.icon_font.family_name,
                    &def_cfg.fonts.icon_font.fonts,
                );
                Self::add_font_family(&mut fonts, def_cfg_icon_family.as_str());
                for font in def_cfg_icon_fonts {
                    Self::add_font(&mut fonts, def_cfg_icon_family.as_str(), font);
                }
                def_cfg
            }
        };
        cc.egui_ctx.set_fonts(fonts);
        egui_extras::install_image_loaders(&cc.egui_ctx);
        Self {
            shared,
            config,
            qtile,
        }
    }

    pub fn find_icon(&self, wm_class: &str) -> Option<PathBuf> {
        let mut icon_lookup_builder = lookup(wm_class)
            .with_size(self.config.icons.lookup_icon_size as u16)
            .with_cache();
        for theme in &self.config.icons.themes {
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

    pub fn window_icon(&self, ui: &mut Ui, win: &HashMap<String, String>) -> egui::Response {
        let wm_class = win.get("class").expect("qtile sends correct format");
        let lowercase_wm_class = wm_class.to_lowercase();
        let path = self.find_icon(&lowercase_wm_class);
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
        let name = truncate_window_name(win.get("name").expect("qtile sends correct format"), 31);
        self.new_label(ui, &name, text_font_id)
    }

    pub fn render_ui(
        &mut self,
        ctx: &eframe::egui::Context,
        windows: &[HashMap<String, String>],
        is_visible: bool,
        focus_index: usize,
    ) {
        ctx.all_styles_mut(|style| {
            style.visuals.panel_fill =
                Color32::from_hex(self.config.colors.bg_color.as_str()).expect("color from hex");
        });

        let text_font_id = egui::FontId {
            family: FontFamily::Name(self.config.fonts.text_font.family_name.clone().into()),
            size: self.config.fonts.text_font.size,
        };
        let icon_font_id = egui::FontId {
            size: self.config.fonts.icon_font.size,
            family: FontFamily::Name(self.config.fonts.icon_font.family_name.clone().into()),
        };

        let mut final_width = 0.0;
        let mut final_height = 0.0;

        egui::CentralPanel::default().show(ctx, |ui| {
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

            let render_cards = |ui: &mut egui::Ui| {
                for (index, win) in windows.iter().enumerate() {
                    let is_selected = index == focus_index;

                    let bg_color = if is_selected {
                        Color32::from_hex(&self.config.colors.group_hover_color)
                            .unwrap_or(Color32::WHITE)
                            .gamma_multiply(0.15)
                    } else {
                        Color32::TRANSPARENT
                    };

                    let stroke_color = if is_selected
                        || ui.rect_contains_pointer(ui.available_rect_before_wrap())
                    {
                        Color32::from_hex(&self.config.colors.group_hover_color)
                            .unwrap_or(Color32::WHITE)
                    } else {
                        Color32::from_hex(&self.config.colors.normal_group_color)
                            .unwrap_or(Color32::GRAY)
                    };

                    let frame = egui::Frame::NONE
                        .inner_margin(egui::Margin::same(12))
                        .corner_radius(12)
                        .stroke(Stroke::new(
                            self.config.sizes.group_rect_stroke_width,
                            stroke_color,
                        ))
                        .fill(bg_color);

                    let response = frame
                        .show(ui, |ui| {
                            // Apply fixed min width if vertical so cards align perfectly
                            if self.config.ui.orientation == Orientation::Vertical {
                                ui.set_min_width(self.config.sizes.window_size.width - 24.0);
                            } else {
                                ui.set_min_width(self.config.sizes.window_size.width / 1.5);
                            }

                            ui.horizontal(|ui| {
                                // Render Icon first
                                if self.config.ui.items.contains(&crate::config::UiItem::Icon) {
                                    self.window_icon(ui, win);
                                    ui.add_space(12.0); // generous gap
                                }

                                // Render text vertically stacked next to the icon
                                ui.vertical(|ui| {
                                    ui.spacing_mut().item_spacing.y = 4.0; // tighter text spacing

                                    for item in &self.config.ui.items {
                                        match item {
                                            crate::config::UiItem::Icon => {} // Already handled
                                            crate::config::UiItem::Name => {
                                                let mut name = win
                                                    .get("name")
                                                    .unwrap_or(&String::new())
                                                    .clone();
                                                // Truncate to look clean inside cards
                                                if name.len() > 35 {
                                                    let upto = name
                                                        .char_indices()
                                                        .nth(35)
                                                        .map(|(i, _)| i)
                                                        .unwrap_or(name.len());
                                                    name.truncate(upto);
                                                    name.push_str("...");
                                                }
                                                let base_color = Color32::from_hex(
                                                    &self.config.colors.text_color,
                                                )
                                                .unwrap_or(Color32::GRAY);
                                                let color = if is_selected {
                                                    Color32::from_hex(
                                                        &self.config.colors.group_hover_color,
                                                    )
                                                    .unwrap_or(Color32::WHITE)
                                                } else {
                                                    base_color
                                                };
                                                ui.label(
                                                    egui::RichText::new(name)
                                                        .font(text_font_id.clone())
                                                        .color(color)
                                                        .strong(),
                                                );
                                            }
                                            crate::config::UiItem::GroupName => {
                                                let text = win
                                                    .get("group_name")
                                                    .cloned()
                                                    .unwrap_or_default();
                                                ui.label(
                                                    egui::RichText::new(text)
                                                        .font(egui::FontId::new(
                                                            text_font_id.size * 0.85,
                                                            text_font_id.family.clone(),
                                                        ))
                                                        .color(
                                                            Color32::from_hex(
                                                                &self.config.colors.text_color,
                                                            )
                                                            .unwrap_or(Color32::GRAY)
                                                            .gamma_multiply(0.7),
                                                        ),
                                                );
                                            }
                                            crate::config::UiItem::GroupLabel => {
                                                let text = win
                                                    .get("group_label")
                                                    .cloned()
                                                    .unwrap_or_default();
                                                ui.label(
                                                    egui::RichText::new(text)
                                                        .font(icon_font_id.clone())
                                                        .color(
                                                            Color32::from_hex(
                                                                &self.config.colors.text_color,
                                                            )
                                                            .unwrap_or(Color32::GRAY)
                                                            .gamma_multiply(0.7),
                                                        ),
                                                );
                                            }
                                        }
                                    }
                                });
                            });
                        })
                        .response
                        .interact(egui::Sense::click())
                        .on_hover_cursor(egui::CursorIcon::Crosshair);

                    if response.middle_clicked() {
                        self.close_window(win);
                    }
                    if response.clicked() {
                        self.focus_window(win);
                        // Hide window upon click selection
                        let shared = self.shared.clone();
                        let mut state = shared.lock().unwrap();
                        if let Some(wid) = state.cached_wid.clone() {
                            let qtile_c = Arc::clone(&self.qtile);
                            tokio::task::spawn_blocking(move || {
                                let _ = qtile_c.call(
                                    Some(vec![]),
                                    Some("eval".into()),
                                    Some(vec![format!("self.windows_map[{wid}].hide()")]),
                                );
                            });
                        }
                        state.is_visible = false;
                        state.current_focus_history = None;
                        state.last_placed_height = 0.0;
                    }

                    if index < windows.len() - 1 {
                        ui.add_space(self.config.sizes.group_spacing);
                    }
                }
            };

            let response = if self.config.ui.orientation == Orientation::Horizontal {
                ui.horizontal(render_cards).response
            } else {
                ui.vertical(render_cards).response
            };

            final_width = response.rect.width();
            final_height = response.rect.height();
        });

        // Compute outer window bounds including margins
        let padding_x = ctx.style().spacing.window_margin.left as f32
            + ctx.style().spacing.window_margin.right as f32;
        let padding_y = ctx.style().spacing.window_margin.top as f32
            + ctx.style().spacing.window_margin.bottom as f32;

        let width = (final_width + padding_x + self.config.sizes.group_rect_stroke_width * 2.0)
            .max(self.config.sizes.window_size.width) // Ensure we at least hit the configured width
            .min(1200.0) as i32; // But don't grow infinitely horizontally

        let height = (final_height + padding_y + self.config.sizes.group_rect_stroke_width * 2.0)
            .min(self.config.sizes.window_size.height) as i32;

        // Only resize/reposition when visible
        if is_visible {
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
                width as f32,
                height as f32,
            )));
            self.resize_and_center(width, height);
        }
    }

    pub fn focus_window(&self, win: &HashMap<String, String>) {
        let wid = win
            .get("id")
            .unwrap_or_else(|| panic!("qtile sends correct format {:?}", win.get("id")))
            .to_string();
        let qtile = Arc::clone(&self.qtile);
        tokio::task::spawn_blocking(move || {
            let _ = qtile.call(
                Some(vec![]),
                Some("eval".into()),
                Some(vec![format!(
                    "self.windows_map[{}].focus(); self.windows_map[{}].bring_to_front()",
                    wid, wid
                )]),
            );
        });
    }

    pub fn resize_and_center(&self, width: i32, height: i32) {
        let mut state = self.shared.lock().unwrap();
        if (state.last_placed_height - height as f32).abs() < 1.0 {
            return;
        }

        if let Some(wid) = state.cached_wid.clone() {
            log::debug!("Resizing window ({wid}) to {width}x{height}");
            let qtile = Arc::clone(&self.qtile);
            tokio::task::spawn_blocking(move || {
                let _ = qtile.call(
                    Some(vec![]),
                    Some("eval".into()),
                    Some(vec![format!(
                        "s = self.current_screen; \
                         x = s.dx + (s.dwidth - {width}) // 2; \
                         y = s.dy + (s.dheight - {height}) // 2; \
                         self.windows_map[{wid}].place(x, y, {width}, {height}, 0, None); \
                         self.windows_map[{wid}].keep_above(); \
                         self.windows_map[{wid}].bring_to_front()"
                    )]),
                );
            });
            state.last_placed_height = height as f32;
            state.last_width = width;
            state.last_height = height;
        }
    }

    fn close_window(&self, win: &HashMap<String, String>) {
        let wid = win.get("id").expect("qtile sends correct format").clone();
        let qtile = Arc::clone(&self.qtile);
        tokio::task::spawn_blocking(move || {
            let _ = qtile.call(
                Some(vec!["window".to_owned(), wid]),
                Some("kill".into()),
                Some(vec![]),
            );
        });
    }
}

impl eframe::App for AsyncApp {
    fn update(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) {
        let state = self.shared.lock().unwrap();
        let is_visible = state.is_visible;
        let history = state.current_focus_history.clone();
        let focus_index = state.focus_index;
        drop(state);

        log::debug!(
            "update() | visible={} history={}",
            is_visible,
            history.is_some()
        );
        // Always render if we have history — keeps the buffer populated so
        // the window has content ready when place() makes it visible.
        if let Some(history) = history {
            self.render_ui(ctx, &history.windows, is_visible, focus_index);
        }
        if is_visible {
            ctx.request_repaint();
        }
    }
}

pub fn run_ui() -> anyhow::Result<()> {
    let s = System::new_all();
    let qalttab_processes_parents = s
        .processes_by_exact_name("qalttab".as_ref())
        .map(|p| p.parent());
    let mut qalttab_processes_vec = qalttab_processes_parents.collect::<Vec<Option<Pid>>>();
    qalttab_processes_vec.sort();
    qalttab_processes_vec.dedup();
    if qalttab_processes_vec.len() >= 4 {
        bail!("qalttab already running");
    };
    match eframe::run_native(
        "qalttab",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder {
                title: Some("qalttab".to_owned()),
                app_id: Some("qalttab".to_owned()),
                decorations: Some(false),
                inner_size: Some(egui::vec2(300.0, 400.0)),
                visible: Some(false),
                taskbar: Some(false),
                title_shown: Some(false),
                window_level: Some(egui::WindowLevel::AlwaysOnTop),
                ..egui::ViewportBuilder::default()
            },
            ..eframe::NativeOptions::default()
        },
        Box::new(|cc| Ok(Box::<AsyncApp>::new(AsyncApp::new(cc)))),
    ) {
        Ok(()) => Ok(()),
        Err(e) => bail!("eframe crashed: {}", e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Font;

    #[test]
    fn truncate_short_name_unchanged() {
        assert_eq!(truncate_window_name("hello", 31), "hello");
    }

    #[test]
    fn truncate_at_exact_limit_unchanged() {
        let name = "a".repeat(31);
        assert_eq!(truncate_window_name(&name, 31), name);
    }

    #[test]
    fn truncate_over_limit() {
        let name = "a".repeat(40);
        let result = truncate_window_name(&name, 31);
        assert_eq!(result.chars().count(), 31);
        assert_eq!(result, "a".repeat(31));
    }

    #[test]
    fn truncate_multibyte_unicode() {
        // Each emoji is multiple bytes; truncating must respect char boundaries.
        let name = "😀".repeat(40);
        let result = truncate_window_name(&name, 10);
        assert_eq!(result.chars().count(), 10);
        assert_eq!(result, "😀".repeat(10));
    }

    #[test]
    fn truncate_zero_max() {
        assert_eq!(truncate_window_name("anything", 0), "");
    }

    #[test]
    fn add_font_family_inserts_key() {
        let mut fonts = FontDefinitions::default();
        AsyncApp::add_font_family(&mut fonts, "my-family");
        assert!(
            fonts
                .families
                .contains_key(&FontFamily::Name("my-family".into()))
        );
    }

    #[test]
    fn add_font_nonexistent_path_does_not_panic() {
        let mut fonts = FontDefinitions::default();
        AsyncApp::add_font_family(&mut fonts, "fam");
        let font = Font {
            name: "ghost".to_string(),
            path: "/nonexistent/path/to/font.ttf".to_string(),
        };
        AsyncApp::add_font(&mut fonts, "fam", &font);
        // Font data should NOT have been inserted.
        assert!(!fonts.font_data.contains_key("ghost"));
    }

    #[test]
    fn add_font_existing_path_inserts_data() {
        let path = "/etc/hostname";
        if !std::path::Path::new(path).exists() {
            return; // skip if unavailable
        }
        let mut fonts = FontDefinitions::default();
        AsyncApp::add_font_family(&mut fonts, "fam");
        let font = Font {
            name: "fake-font".to_string(),
            path: path.to_string(),
        };
        AsyncApp::add_font(&mut fonts, "fam", &font);
        assert!(fonts.font_data.contains_key("fake-font"));
        assert!(
            fonts
                .families
                .get(&FontFamily::Name("fam".into()))
                .unwrap()
                .contains(&"fake-font".to_string())
        );
    }

    #[test]
    fn shared_state_default() {
        let s = SharedState::default();
        assert!(!s.is_visible);
        assert!(s.cached_wid.is_none());
        assert_eq!(s.focus_index, 0);
        assert!(s.current_focus_history.is_none());
    }
}
