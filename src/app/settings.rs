use super::*;

impl Notes {
    pub(super) fn settings_ui(&mut self, ui: &mut Ui) {
        ui.style_mut().override_font_id = Some(FontId::new(
            16.0,
            FontFamily::Name(self.prefs.ui_font.clone().into()),
        ));
        ui.spacing_mut().interact_size.y = 32.0;
        ui.spacing_mut().button_padding = vec2(10.0, 6.0);
        ui.spacing_mut().item_spacing = vec2(10.0, 8.0);
        let width = 760.0_f32.min((ui.available_width() - 48.0).max(260.0));
        let pad = ((ui.available_width() - width) / 2.0).max(24.0);
        egui::ScrollArea::vertical()
            .id_salt("settings")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    ui.add_space(pad);
                    ui.vertical(|ui| {
                        ui.set_width(width);
                        ui.add_space(45.0);
                        ui.label(RichText::new("Settings").size(28.0).strong());
                        ui.add_space(28.0);
                        section(ui, "Appearance");
                        settings_group(ui, |ui| {
                            setting_row(
                                ui,
                                "Theme",
                                "Choose the appearance of your workspace",
                                |ui| {
                                    egui::ComboBox::from_id_salt("theme")
                                        .selected_text(if self.prefs.light {
                                            "Light"
                                        } else {
                                            "Dark"
                                        })
                                        .width(140.0)
                                        .show_ui(ui, |ui| {
                                            ui.selectable_value(
                                                &mut self.prefs.light,
                                                false,
                                                "Dark",
                                            );
                                            ui.selectable_value(
                                                &mut self.prefs.light,
                                                true,
                                                "Light",
                                            );
                                        });
                                },
                            );
                        });
                        ui.add_space(22.0);
                        section(
                            ui,
                            if self.prefs.light {
                                "Light Theme"
                            } else {
                                "Dark Theme"
                            },
                        );
                        settings_group(ui, |ui| {
                            color_row(
                                ui,
                                "Accent",
                                "Links, checkboxes and highlights",
                                &mut self.prefs.accent,
                                [255, 101, 0],
                            );
                            color_row(
                                ui,
                                "Background",
                                "Main writing surface",
                                if self.prefs.light {
                                    &mut self.prefs.light_background
                                } else {
                                    &mut self.prefs.background
                                },
                                if self.prefs.light {
                                    [250, 249, 247]
                                } else {
                                    [17, 17, 17]
                                },
                            );
                            color_row(
                                ui,
                                "Foreground",
                                "Document text color",
                                if self.prefs.light {
                                    &mut self.prefs.light_foreground
                                } else {
                                    &mut self.prefs.foreground
                                },
                                if self.prefs.light {
                                    [40, 40, 40]
                                } else {
                                    [207, 207, 207]
                                },
                            );
                            setting_row(ui, "UI Font", "Font used throughout the app", |ui| {
                                font_picker(ui, "ui-font", &mut self.prefs.ui_font);
                            });
                            setting_row(ui, "Editor Font", "Font used for your notes", |ui| {
                                font_picker(ui, "editor-font", &mut self.prefs.editor_font);
                            });
                            setting_row(
                                ui,
                                "Sidebar tint",
                                "Tint strength, from subtle to vivid",
                                |ui| {
                                    ui.add(
                                        egui::Slider::new(
                                            &mut self.prefs.translucency,
                                            0.0..=100.0,
                                        )
                                        .show_value(true),
                                    );
                                },
                            );
                            setting_row(
                                ui,
                                "Contrast",
                                "Definition of borders and control backgrounds",
                                |ui| {
                                    ui.add(
                                        egui::Slider::new(&mut self.prefs.contrast, 0.0..=40.0)
                                            .show_value(true),
                                    );
                                },
                            );
                        });
                        ui.add_space(22.0);
                        section(ui, "Editor");
                        settings_group(ui, |ui| {
                            setting_row(ui, "Font Size", "Base size of the document text", |ui| {
                                ui.add(
                                    egui::Slider::new(&mut self.prefs.font_size, 12.0..=28.0)
                                        .suffix(" px"),
                                );
                            });
                            setting_row(
                                ui,
                                "Content Width",
                                "Maximum width of the writing column",
                                |ui| {
                                    ui.add(
                                        egui::Slider::new(
                                            &mut self.prefs.content_width,
                                            480.0..=1600.0,
                                        )
                                        .suffix(" px"),
                                    );
                                },
                            );
                            setting_row(
                                ui,
                                "Zoom",
                                "Scale document text without changing the interface",
                                |ui| {
                                    ui.add(egui::Slider::new(&mut self.prefs.zoom, 0.5..=3.0));
                                },
                            );
                            setting_row(
                                ui,
                                "Top Padding",
                                "Space before the first heading",
                                |ui| {
                                    ui.add(
                                        egui::Slider::new(
                                            &mut self.prefs.top_padding,
                                            24.0..=180.0,
                                        )
                                        .suffix(" px"),
                                    );
                                },
                            );
                        });
                        ui.add_space(22.0);
                        section(ui, "Workspace");
                        ui.checkbox(
                            &mut self.prefs.show_library,
                            "Show pinned and recent notes in sidebar",
                        );
                        settings_group(ui, |ui| {
                            ui.label(self.prefs.root.display().to_string());
                            if ui.button("Open another workspace…").clicked() {
                                self.open_workspace();
                            }
                        });
                        ui.add_space(22.0);
                        section(ui, "Updates");
                        ui.checkbox(&mut self.prefs.check_updates, "Check for updates daily");
                        ui.checkbox(
                            &mut self.prefs.install_updates,
                            "Download verified updates and install when Notes closes",
                        );
                        if self.updater.ui(ui) && !self.defer_until_saved(file_io::Deferred::Update)
                        {
                            self.install_update();
                        }
                        ui.add_space(22.0);
                        section(ui, "Keyboard Shortcuts");
                        settings_group(ui, |ui| {
                            for (key, label) in [
                                ("Ctrl+P", "Find notes and commands"),
                                ("Ctrl+N", "New note"),
                                ("Ctrl+S", "Save"),
                                ("Ctrl+W", "Close tab"),
                                ("Ctrl+\\", "Toggle sidebar"),
                                ("Ctrl+,", "Settings"),
                                ("Ctrl+F", "Find in note"),
                                ("Ctrl+H", "Find and replace"),
                                ("F3 / Shift+F3", "Next / previous match"),
                                ("Ctrl+B / I / K", "Bold / italic / link"),
                                ("F4", "Toggle rendered preview"),
                                ("Alt+Left / Right", "Navigate history"),
                            ] {
                                ui.horizontal(|ui| {
                                    ui.label(label);
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        ui.label(RichText::new(key).color(secondary(ui)));
                                    });
                                });
                            }
                        });
                        ui.add_space(28.0);
                        ui.label(
                            RichText::new(
                                "Notes for Windows · Rust + egui\nAll notes stay on your computer.",
                            )
                            .size(14.0)
                            .color(secondary(ui)),
                        );
                        ui.add_space(60.0);
                    });
                });
            });
    }
}

fn section(ui: &mut Ui, title: &str) {
    ui.label(RichText::new(title).size(18.0).strong());
    ui.add_space(10.0);
}
fn settings_group(ui: &mut Ui, content: impl FnOnce(&mut Ui)) {
    Frame::new()
        .fill(if ui.visuals().dark_mode {
            Color32::from_gray(28)
        } else {
            Color32::from_gray(243)
        })
        .corner_radius(8)
        .inner_margin(Margin::symmetric(18, 12))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            content(ui);
        });
}
fn secondary(ui: &Ui) -> Color32 {
    if ui.visuals().dark_mode {
        Color32::from_gray(180)
    } else {
        Color32::from_gray(85)
    }
}

fn setting_row(ui: &mut Ui, label: &str, description: &str, control: impl FnOnce(&mut Ui)) {
    let labels = |ui: &mut Ui| {
        ui.label(RichText::new(label).size(16.0).strong());
        ui.label(RichText::new(description).size(14.0).color(secondary(ui)));
    };
    if ui.available_width() < 560.0 {
        ui.vertical(|ui| {
            labels(ui);
            ui.add_space(4.0);
            ui.horizontal(control);
        });
        ui.add_space(12.0);
    } else {
        ui.horizontal(|ui| {
            ui.set_min_height(76.0);
            ui.vertical(|ui| {
                ui.set_width((ui.available_width() - 250.0).max(160.0));
                labels(ui);
            });
            ui.with_layout(Layout::right_to_left(Align::Center), control);
        });
    }
}
fn color_row(ui: &mut Ui, label: &str, description: &str, color: &mut [u8; 3], default: [u8; 3]) {
    setting_row(ui, label, description, |ui| {
        let id = ui.make_persistent_id(("color-hex", label));
        let formatted = format!("#{:02X}{:02X}{:02X}", color[0], color[1], color[2]);
        let mut hex = ui
            .ctx()
            .data_mut(|d| d.get_temp::<String>(id))
            .unwrap_or_else(|| formatted.clone());
        if !ui.ctx().memory(|m| m.has_focus(id)) {
            hex = formatted;
        }
        let response = ui.add(
            egui::TextEdit::singleline(&mut hex)
                .id(id)
                .desired_width(100.0)
                .font(FontId::monospace(14.0)),
        );
        if response.changed()
            && let Some(rgb) = parse_color(&hex)
        {
            *color = rgb;
        }
        ui.ctx().data_mut(|d| d.insert_temp(id, hex));
        ui.color_edit_button_srgb(color);
        if ui.button("Reset").clicked() {
            *color = default;
        }
    });
}
fn parse_color(text: &str) -> Option<[u8; 3]> {
    let text = text.strip_prefix('#').unwrap_or(text);
    if text.len() != 6 || !text.is_ascii() {
        return None;
    }
    Some([
        u8::from_str_radix(&text[0..2], 16).ok()?,
        u8::from_str_radix(&text[2..4], 16).ok()?,
        u8::from_str_radix(&text[4..6], 16).ok()?,
    ])
}

fn font_picker(ui: &mut Ui, id: &str, font: &mut String) {
    egui::ComboBox::from_id_salt(id)
        .selected_text(font.as_str())
        .width(160.0)
        .show_ui(ui, |ui| {
            for name in ["Segoe UI", "Arial", "Calibri", "Consolas"] {
                ui.selectable_value(font, name.into(), name);
            }
        });
}
