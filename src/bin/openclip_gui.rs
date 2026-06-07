#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui;

#[path = "../config.rs"]
mod config;
use config::Config;

fn setup_custom_styles(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    
    style.spacing.item_spacing = egui::vec2(10.0, 10.0);
    style.spacing.button_padding = egui::vec2(12.0, 6.0);

    let mac_bg = egui::Color32::from_rgb(40, 40, 40);
    let mac_panel = egui::Color32::from_rgb(50, 50, 52);
    let mac_accent = egui::Color32::from_rgb(10, 132, 255);
    let mac_text = egui::Color32::from_rgb(230, 230, 230);

    style.visuals.widgets.noninteractive.bg_fill = mac_bg;
    style.visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, mac_text);

    style.visuals.widgets.inactive.bg_fill = mac_panel;
    style.visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, mac_text);
    style.visuals.widgets.inactive.bg_stroke = egui::Stroke::NONE;

    style.visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(60, 60, 64);
    style.visuals.widgets.hovered.bg_stroke = egui::Stroke::NONE;

    style.visuals.widgets.active.bg_fill = mac_accent;
    style.visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0, egui::Color32::WHITE);
    
    style.visuals.selection.bg_fill = mac_accent;
    style.visuals.selection.stroke = egui::Stroke::new(1.0, egui::Color32::WHITE);

    style.visuals.extreme_bg_color = egui::Color32::from_rgb(60, 60, 64);
    style.visuals.panel_fill = mac_bg;
    style.visuals.window_fill = mac_bg;

    ctx.set_style(style);
}

struct GuiApp {
    config: Config,
    active_tab: String,
    remember_str: String,
    display_str: String,
}

impl GuiApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        setup_custom_styles(&cc.egui_ctx);
        
        let config = Config::load();
        let remember_str = config.remember_clippings.to_string();
        let display_str = config.display_clippings.to_string();

        Self {
            config,
            active_tab: "General".to_string(),
            remember_str,
            display_str,
        }
    }
}

impl eframe::App for GuiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().frame(egui::Frame::new().fill(egui::Color32::from_rgb(40, 40, 40)).inner_margin(20.0)).show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.add_space(50.0);
                ui.spacing_mut().item_spacing.x = 20.0;
                let tabs = [("⚙ General", "General"), ("📋 Management", "Clips Management"), ("ℹ About", "About")];
                for (label, val) in tabs.iter() {
                    let is_selected = self.active_tab == *val;
                    if ui.add(egui::SelectableLabel::new(is_selected, *label)).clicked() {
                        self.active_tab = val.to_string();
                    }
                }
            });
            
            ui.add_space(20.0);

            if self.active_tab == "General" {
                ui.label(egui::RichText::new("Clippings").strong());
                ui.add_space(4.0);
                egui::Frame::new().fill(egui::Color32::from_rgb(44, 44, 46)).corner_radius(10).inner_margin(12.0).show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    egui::Grid::new("clippings_grid").num_columns(3).spacing([10.0, 10.0]).show(ui, |ui| {
                        ui.label("Remember:");
                        ui.add(egui::TextEdit::singleline(&mut self.remember_str)
                            .desired_width(40.0)
                            .horizontal_align(egui::Align::Center)
                            .margin(egui::vec2(4.0, 4.0)));
                        if let Ok(val) = self.remember_str.parse::<usize>() {
                            self.config.remember_clippings = val;
                        }
                        ui.label("clippings");
                        ui.end_row();
                        
                        ui.label("Display:");
                        ui.add(egui::TextEdit::singleline(&mut self.display_str)
                            .desired_width(40.0)
                            .horizontal_align(egui::Align::Center)
                            .margin(egui::vec2(4.0, 4.0)));
                        if let Ok(val) = self.display_str.parse::<usize>() {
                            self.config.display_clippings = val;
                        }
                        ui.label("clippings");
                        ui.end_row();
                    });
                });
                
                ui.add_space(16.0);
                
                ui.label(egui::RichText::new("Options").strong());
                ui.add_space(4.0);
                egui::Frame::new().fill(egui::Color32::from_rgb(44, 44, 46)).corner_radius(10).inner_margin(12.0).show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.vertical(|ui| {
                        if ui.checkbox(&mut self.config.start_at_startup, "Start OpenClip at system startup").changed() {
                            let mut app_path = std::env::current_exe().unwrap();
                            if app_path.to_string_lossy().contains(".app/Contents/MacOS/") {
                                app_path = app_path.ancestors().nth(3).unwrap().to_path_buf();
                            }
                            let app_path_str = app_path.to_string_lossy();
                            
                            if self.config.start_at_startup {
                                let script = format!(
                                    "tell application \"System Events\" to make login item at end with properties {{path:\"{}\", hidden:false}}",
                                    app_path_str
                                );
                                let _ = std::process::Command::new("osascript").arg("-e").arg(script).output();
                            } else {
                                let script = "tell application \"System Events\" to delete login item \"OpenClip\"";
                                let _ = std::process::Command::new("osascript").arg("-e").arg(script).output();
                            }
                        }
                        ui.add_space(8.0);
                        ui.checkbox(&mut self.config.record_history, "Record clipboard history");
                    });
                });
            } else if self.active_tab == "Clips Management" {
                ui.label(egui::RichText::new("Data Management").strong());
                ui.add_space(4.0);
                egui::Frame::new().fill(egui::Color32::from_rgb(44, 44, 46)).corner_radius(10).inner_margin(12.0).show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.label("Warning: This will permanently delete your entire encrypted clipboard history.");
                    ui.add_space(12.0);
                    if ui.button("Clear History").clicked() {
                        self.config.clear_history_flag = true;
                        self.config.save();
                        self.config.clear_history_flag = false;
                        if let Ok(ctx) = clipboard_rs::ClipboardContext::new() {
                            use clipboard_rs::Clipboard;
                            let _ = ctx.set_text("".to_string());
                        }
                    }
                });
            } else if self.active_tab == "About" {
                ui.label(egui::RichText::new("OpenClip").strong());
                ui.add_space(4.0);
                egui::Frame::new().fill(egui::Color32::from_rgb(44, 44, 46)).corner_radius(10).inner_margin(12.0).show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.label("Version 0.1.0\nA secure and lightweight clipboard manager.");
                    ui.add_space(8.0);
                    ui.label("Memory footprint: ~15 MB.");
                });
            }
        });

        if ctx.input(|i| i.viewport().close_requested()) {
            self.config.save();
        }
    }
}

fn main() -> eframe::Result<()> {
    #[cfg(target_os = "macos")]
    {
        use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
        use objc2::MainThreadMarker;
        unsafe {
            if let Some(mtm) = MainThreadMarker::new() {
                let app = NSApplication::sharedApplication(mtm);
                app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
            }
        }
    }

    let icon_data = include_bytes!("../../AppIcon.png");
    let image = image::load_from_memory(icon_data).expect("Failed to load icon").into_rgba8();
    let (width, height) = image.dimensions();
    let egui_icon = egui::IconData {
        rgba: image.into_raw(),
        width,
        height,
    };

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_visible(true)
            .with_inner_size([460.0, 360.0])
            .with_resizable(false)
            .with_maximize_button(false)
            .with_minimize_button(false)
            .with_title("Preferences")
            .with_icon(egui_icon),
        ..Default::default()
    };

    eframe::run_native(
        "OpenClip Preferences",
        options,
        Box::new(|cc| Ok(Box::new(GuiApp::new(cc)))),
    )
}
