use eframe::egui;
use std::path::PathBuf;

pub fn run_gui(app_type: &'static str) {
    #[cfg(target_os = "macos")]
    {
        use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
        use objc2_foundation::MainThreadMarker;
        if let Some(mtm) = MainThreadMarker::new() {
            let app = NSApplication::sharedApplication(mtm);
            let _ = app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
        }
    }

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([800.0, 360.0])
            .with_resizable(false)
            .with_maximize_button(false),
        ..Default::default()
    };
    let app_name = if app_type == "client" {
        "Open Remote URL Client"
    } else {
        "Open Remote URL Host"
    };
    let _ = eframe::run_native(
        app_name,
        native_options,
        Box::new(move |cc| {
            // Customize look and feel
            let mut style = (*cc.egui_ctx.style()).clone();
            
            // Set modern dark theme colors
            let mut visuals = egui::Visuals::dark();
            visuals.window_rounding = 12.0.into();
            visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(30, 30, 46); // dark slate/navy
            visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(45, 45, 68);
            visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(60, 60, 90);
            visuals.widgets.active.bg_fill = egui::Color32::from_rgb(75, 75, 110);
            visuals.widgets.inactive.rounding = 8.0.into();
            visuals.widgets.hovered.rounding = 8.0.into();
            visuals.widgets.active.rounding = 8.0.into();
            
            style.visuals = visuals;
            cc.egui_ctx.set_style(style);
            
            Ok(Box::new(StatusApp::new(app_type)))
        }),
    );
}

struct StatusApp {
    app_type: &'static str,
    is_installed: bool,
    is_running: bool,
    exe_path: PathBuf,
    config_path: PathBuf,
    message: Option<String>,
    is_error: bool,
    is_running_from_install_dir: bool,
}

impl StatusApp {
    fn new(app_type: &'static str) -> Self {
        let (is_installed, is_running, exe_path, config_path) = crate::installer::check_status(app_type);
        let is_running_from_install_dir = if let Ok(current_exe) = std::env::current_exe() {
            current_exe == exe_path
        } else {
            false
        };
        Self {
            app_type,
            is_installed,
            is_running,
            exe_path,
            config_path,
            message: None,
            is_error: false,
            is_running_from_install_dir,
        }
    }

    fn refresh(&mut self) {
        let (is_installed, is_running, exe_path, config_path) = crate::installer::check_status(self.app_type);
        self.is_installed = is_installed;
        self.is_running = is_running;
        self.exe_path = exe_path;
        self.config_path = config_path;
    }
}

impl eframe::App for StatusApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(15.0);
            
            ui.vertical_centered(|ui| {
                let title = if self.app_type == "client" {
                    "Open Remote URL — Client Panel"
                } else {
                    "Open Remote URL — Host Panel"
                };
                ui.heading(title);
            });
            
            ui.add_space(15.0);
            ui.separator();
            ui.add_space(15.0);

            // Status display grid
            egui::Grid::new("status_grid")
                .num_columns(2)
                .spacing([20.0, 14.0])
                .show(ui, |ui| {
                    ui.strong("Service Status:");
                    ui.horizontal(|ui| {
                        // Installation status
                        ui.horizontal(|ui| {
                            let dot_color = if self.is_installed {
                                egui::Color32::from_rgb(46, 204, 113) // Green
                            } else {
                                egui::Color32::from_rgb(149, 165, 166) // Gray
                            };
                            let (rect, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
                            ui.painter().circle_filled(rect.center(), 5.0, dot_color);
                            ui.add_space(4.0);
                            ui.label(if self.is_installed { "Installed" } else { "Not Installed" });
                        });
                        
                        ui.add_space(15.0);
                        ui.label("|");
                        ui.add_space(15.0);
                        
                        // Running status
                        ui.horizontal(|ui| {
                            let dot_color = if self.is_running {
                                egui::Color32::from_rgb(46, 204, 113) // Green
                            } else {
                                egui::Color32::from_rgb(231, 76, 60) // Red
                            };
                            let (rect, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
                            ui.painter().circle_filled(rect.center(), 5.0, dot_color);
                            ui.add_space(4.0);
                            ui.label(if self.is_running { "Running (Daemon)" } else { "Stopped" });
                        });
                    });
                    ui.end_row();

                    ui.strong("Executable Path:");
                    if self.is_installed {
                        ui.label(self.exe_path.to_string_lossy().to_string());
                    } else {
                        ui.weak("(Will be set upon installation)");
                    }
                    ui.end_row();

                    ui.strong("Configuration:");
                    ui.label(self.config_path.to_string_lossy().to_string());
                    ui.end_row();
                });

            ui.add_space(20.0);
            ui.separator();
            ui.add_space(20.0);

            // Action Buttons
            ui.horizontal(|ui| {
                ui.add_space(10.0);

                let buttons_enabled = !(self.is_running_from_install_dir && !self.is_installed);

                let install_text = if self.is_installed {
                    "Reinstall / Update"
                } else {
                    "Install Service"
                };
                
                // Install/Update button
                let install_btn = ui.add_enabled(
                    buttons_enabled,
                    egui::Button::new(install_text).min_size(egui::vec2(140.0, 32.0))
                );
                if install_btn.clicked() {
                    self.message = Some("Installing service...".to_string());
                    self.is_error = false;
                    ctx.request_repaint();
                    match crate::installer::install(self.app_type) {
                        Ok(_) => {
                            self.message = Some("Service successfully installed and started!".to_string());
                            self.is_error = false;
                        }
                        Err(e) => {
                            self.message = Some(format!("Installation failed: {}", e));
                            self.is_error = true;
                        }
                    }
                    self.refresh();
                }

                ui.add_space(10.0);

                // Uninstall button
                let uninstall_btn = ui.add_enabled(
                    buttons_enabled && self.is_installed,
                    egui::Button::new("Uninstall Service").min_size(egui::vec2(140.0, 32.0))
                );
                if uninstall_btn.clicked() {
                    self.message = Some("Uninstalling service...".to_string());
                    self.is_error = false;
                    ctx.request_repaint();
                    match crate::installer::uninstall(self.app_type) {
                        Ok(_) => {
                            self.message = Some("Service successfully uninstalled!".to_string());
                            self.is_error = false;
                        }
                        Err(e) => {
                            self.message = Some(format!("Uninstallation failed: {}", e));
                            self.is_error = true;
                        }
                    }
                    self.refresh();
                }

                ui.add_space(10.0);

                // Open settings folder button
                let folder_btn = ui.add_enabled(
                    buttons_enabled,
                    egui::Button::new("Open Settings Folder").min_size(egui::vec2(160.0, 32.0))
                );
                if folder_btn.clicked() {
                    let dir_to_open = self.config_path.parent().unwrap_or(&self.config_path);
                    if self.is_installed {
                        let _ = std::fs::create_dir_all(dir_to_open);
                    }
                    if let Err(e) = crate::config::open_dir_in_file_manager(dir_to_open) {
                        self.message = Some(format!("Could not open folder: {}", e));
                        self.is_error = true;
                    }
                }
            });

            // Status message feedback
            if let Some(ref msg) = self.message {
                ui.add_space(20.0);
                ui.vertical_centered(|ui| {
                    let text_color = if self.is_error {
                        egui::Color32::from_rgb(231, 76, 60) // Red
                    } else {
                        egui::Color32::from_rgb(46, 204, 113) // Green
                    };
                    ui.colored_label(text_color, msg);
                });
            }
        });
    }
}
