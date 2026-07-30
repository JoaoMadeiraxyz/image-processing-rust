use eframe::egui;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver};

use crate::processing::{FilterParams, Image};

const KNOWN_EXTENSIONS: [&str; 5] = ["png", "jpg", "jpeg", "bmp", "tga"];

const PREVIEW_MAX_EDGE: u32 = 800;

const PANEL_WIDTH: f32 = 290.0;

#[derive(Default)]
pub struct ImageApp {
    source: Option<Arc<Image>>,
    preview_src: Option<Image>,
    original_tex: Option<egui::TextureHandle>,
    processed_tex: Option<egui::TextureHandle>,
    params: FilterParams,
    preview_dirty: bool,
    save_modal: Option<SaveModal>,
    save_rx: Option<Receiver<Result<PathBuf, String>>>,
    status: Option<Status>,
    #[cfg(test)]
    probe: Probe,
}

#[cfg(test)]
#[derive(Default)]
struct Probe {
    save_button_rect: Option<egui::Rect>,
    visible_bottom: f32,
}

struct SaveModal {
    filename: String,
    dir: PathBuf,
}

struct Status {
    text: String,
    is_error: bool,
}

impl Status {
    fn info(text: impl Into<String>) -> Self {
        Status { text: text.into(), is_error: false }
    }

    fn error(text: impl Into<String>) -> Self {
        Status { text: text.into(), is_error: true }
    }
}

impl eframe::App for ImageApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.draw(ui);
    }
}

impl ImageApp {
    fn draw(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();

        self.poll_save(&ctx);
        self.refresh_preview(&ctx);

        self.top_bar(ui, &ctx);
        self.status_bar(ui);
        self.filter_panel(ui);
        self.previews(ui);
        self.save_dialog(&ctx);
    }

    fn open_image(&mut self, ctx: &egui::Context) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Images", &["png", "jpg", "jpeg", "bmp", "tga"])
            .pick_file()
        else {
            return;
        };

        self.load_path(&path, ctx);
    }

    fn load_path(&mut self, path: &Path, ctx: &egui::Context) {
        match Image::new(path) {
            Ok(img) => {
                let preview = img.thumbnail(PREVIEW_MAX_EDGE);
                set_texture(ctx, &mut self.original_tex, "original", to_color_image(&preview));

                self.status = Some(Status::info(format!(
                    "Loaded {} ({}×{})",
                    path.display(),
                    img.width(),
                    img.height()
                )));
                self.source = Some(Arc::new(img));
                self.preview_src = Some(preview);
                self.preview_dirty = true;
            }
            Err(err) => {
                self.status = Some(Status::error(format!("Could not open {}: {err}", path.display())));
            }
        }
    }

    fn refresh_preview(&mut self, ctx: &egui::Context) {
        if !self.preview_dirty {
            return;
        }
        self.preview_dirty = false;

        let Some(src) = &self.preview_src else { return };
        let mut processed = src.clone();
        processed.process_params(&self.params);

        set_texture(ctx, &mut self.processed_tex, "processed", to_color_image(&processed));
    }

    fn start_save(&mut self, target: PathBuf, ctx: &egui::Context) {
        let Some(source) = &self.source else { return };

        let source = Arc::clone(source);
        let params = self.params;
        let ctx = ctx.clone();
        let (tx, rx) = mpsc::channel();

        std::thread::spawn(move || {
            let mut img = (*source).clone();
            img.process_params(&params);
            let result = if params.compress_on {
                img.write_compressed(&target, params.compress_effort)
            } else {
                img.write(&target).map(|()| target.clone())
            }
            .map_err(|e| e.to_string());
            let _ = tx.send(result);
            ctx.request_repaint();
        });

        self.save_rx = Some(rx);
        self.status = Some(Status::info("Saving…"));
    }

    fn poll_save(&mut self, ctx: &egui::Context) {
        let Some(rx) = self.save_rx.as_ref() else { return };

        let outcome = match rx.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => {
                ctx.request_repaint();
                return;
            }
            Err(mpsc::TryRecvError::Disconnected) => Err("save worker stopped unexpectedly".to_string()),
        };

        self.save_rx = None;
        self.status = Some(match outcome {
            Ok(path) => Status::info(format!("Saved to {}", path.display())),
            Err(err) => Status::error(format!("Save failed: {err}")),
        });
    }

    fn open_save_modal(&mut self) {
        let Some(source) = &self.source else { return };
        let path = Path::new(source.filename());

        let filename = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "output.png".to_string());

        let dir = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));

        self.save_modal = Some(SaveModal { filename, dir });
    }

    fn save_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut modal) = self.save_modal.take() else { return };

        let mut close = false;
        let mut confirmed = false;
        let compress_on = self.params.compress_on;
        let mut final_target = PathBuf::new();

        let response = egui::Modal::new(egui::Id::new("save_modal")).show(ctx, |ui| {
            ui.set_width(460.0);
            ui.heading("Save image");
            ui.add_space(10.0);

            ui.label("File name");
            let name_field = ui.add(
                egui::TextEdit::singleline(&mut modal.filename)
                    .desired_width(f32::INFINITY)
                    .hint_text("output.png"),
            );
            if name_field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                confirmed = true;
            }

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label("Folder");
                if ui.button("Change…").clicked() {
                    if let Some(dir) = rfd::FileDialog::new().set_directory(&modal.dir).pick_folder() {
                        modal.dir = dir;
                    }
                }
            });

            let trimmed = modal.filename.trim();
            let name_empty = trimmed.is_empty();
            let requested = modal.dir.join(trimmed);
            let target = if compress_on {
                requested.with_extension("jpg")
            } else {
                requested.clone()
            };
            final_target = target.clone();

            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(target.display().to_string())
                    .monospace()
                    .small()
                    .weak(),
            );

            let overwrites = !name_empty && target.exists();
            if overwrites {
                ui.add_space(6.0);
                ui.colored_label(
                    egui::Color32::from_rgb(224, 108, 88),
                    "⚠  This file already exists and will be overwritten.",
                );
            }

            if compress_on && target != requested {
                ui.add_space(6.0);
                ui.colored_label(
                    egui::Color32::from_rgb(214, 178, 96),
                    format!(
                        "Compress is on — this will be saved as JPEG ({}), not {}.",
                        target.file_name().unwrap_or_default().to_string_lossy(),
                        requested.file_name().unwrap_or_default().to_string_lossy(),
                    ),
                );
            } else if !name_empty && !extension_is_known(&target) {
                ui.add_space(6.0);
                ui.colored_label(
                    egui::Color32::from_rgb(214, 178, 96),
                    "Unrecognized extension — the file will be written as PNG.",
                );
            }

            ui.add_space(14.0);
            ui.separator();
            ui.add_space(8.0);

            ui.horizontal(|ui| {
                if ui.button("Cancel").clicked() {
                    close = true;
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let label = if overwrites { "Overwrite" } else { "Save" };
                    if ui.add_enabled(!name_empty, egui::Button::new(label)).clicked() {
                        confirmed = true;
                    }
                });
            });
        });

        if response.should_close() {
            close = true;
        }

        if confirmed && !modal.filename.trim().is_empty() {
            self.start_save(final_target, ctx);
            close = true;
        }

        if !close {
            self.save_modal = Some(modal);
        }
    }

    fn top_bar(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let mut open_clicked = false;

        egui::Panel::top("top_bar").show(ui, |ui| {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("  Open image…  ").clicked() {
                    open_clicked = true;
                }
                ui.add_space(8.0);
                match &self.source {
                    Some(img) => {
                        ui.label(
                            egui::RichText::new(img.filename())
                                .monospace()
                                .strong(),
                        );
                        ui.label(
                            egui::RichText::new(format!("{}×{}", img.width(), img.height())).weak(),
                        );
                    }
                    None => {
                        ui.label(egui::RichText::new("No image loaded").weak());
                    }
                }
            });
            ui.add_space(8.0);
        });

        if open_clicked {
            self.open_image(ctx);
        }
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::bottom("status_bar").show(ui, |ui| {
            ui.add_space(4.0);
            match &self.status {
                Some(status) if status.is_error => {
                    ui.colored_label(egui::Color32::from_rgb(224, 108, 88), &status.text);
                }
                Some(status) => {
                    ui.label(egui::RichText::new(&status.text).weak());
                }
                None => {
                    ui.label(egui::RichText::new("Ready").weak());
                }
            }
            ui.add_space(4.0);
        });
    }

    fn filter_panel(&mut self, ui: &mut egui::Ui) {
        let before = self.params;
        let params = &mut self.params;

        egui::Panel::left("filter_panel")
            .exact_size(PANEL_WIDTH)
            .resizable(false)
            .show(ui, |ui| {
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.heading("Filters");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add_enabled(params.any_enabled(), egui::Button::new("Reset")).clicked() {
                            *params = FilterParams::default();
                        }
                    });
                });
                ui.add_space(10.0);

                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.group(|ui| {
                        ui.set_width(ui.available_width());
                        ui.checkbox(&mut params.brightness_on, "Brightness");
                        ui.add_enabled_ui(params.brightness_on, |ui| {
                            ui.add(egui::Slider::new(&mut params.brightness, -0.5..=0.5).text("offset"));
                        });
                    });
                    ui.add_space(8.0);

                    ui.group(|ui| {
                        ui.set_width(ui.available_width());
                        ui.checkbox(&mut params.contrast_on, "Contrast");
                        ui.add_enabled_ui(params.contrast_on, |ui| {
                            ui.add(egui::Slider::new(&mut params.contrast, -0.5..=0.5).text("amount"));
                        });
                    });
                    ui.add_space(8.0);

                    ui.group(|ui| {
                        ui.set_width(ui.available_width());
                        ui.checkbox(&mut params.saturation_on, "Saturation");
                        ui.add_enabled_ui(params.saturation_on, |ui| {
                            ui.add(egui::Slider::new(&mut params.saturation, 0.0..=3.0).text("factor"));
                        });
                    });
                    ui.add_space(8.0);

                    ui.group(|ui| {
                        ui.set_width(ui.available_width());
                        ui.checkbox(&mut params.color_mask_on, "Color mask");
                        ui.add_enabled_ui(params.color_mask_on, |ui| {
                            ui.add(egui::Slider::new(&mut params.mask[0], 0.0..=2.0).text("R"));
                            ui.add(egui::Slider::new(&mut params.mask[1], 0.0..=2.0).text("G"));
                            ui.add(egui::Slider::new(&mut params.mask[2], 0.0..=2.0).text("B"));
                        });
                    });
                    ui.add_space(8.0);

                    ui.group(|ui| {
                        ui.set_width(ui.available_width());
                        ui.checkbox(&mut params.invert_on, "Invert");
                    });
                    ui.add_space(8.0);

                    ui.group(|ui| {
                        ui.set_width(ui.available_width());
                        ui.checkbox(&mut params.scale_on, "Scale");
                        ui.add_enabled_ui(params.scale_on, |ui| {
                            ui.add(egui::Slider::new(&mut params.scale_width, 0..=5000).text("width"));
                            ui.add(egui::Slider::new(&mut params.scale_height, 0..=5000).text("height"));
                        });
                    });
                    ui.add_space(8.0);

                    ui.group(|ui| {
                        ui.set_width(ui.available_width());
                        ui.checkbox(&mut params.compress_on, "Compress");
                        ui.add_enabled_ui(params.compress_on, |ui| {
                            ui.add(
                                egui::Slider::new(&mut params.compress_effort, 0..=2u8)
                                    .text("effort"),
                            );
                            ui.label(
                                egui::RichText::new(compress_effort_label(params.compress_effort))
                                    .small()
                                    .weak(),
                            );
                        });
                    });
                    ui.add_space(8.0);

                    ui.label(
                        egui::RichText::new(
                            "Applied in order: brightness → contrast → saturation → color mask → invert.",
                        )
                        .small()
                        .weak(),
                    );
                });
            });

        if self.params != before {
            self.preview_dirty = true;
        }
    }

    fn previews(&mut self, ui: &mut egui::Ui) {
        let mut save_clicked = false;
        let mut save_button_rect = None;
        let mut visible_bottom = 0.0;

        egui::CentralPanel::default().show(ui, |ui| {
            if self.source.is_none() {
                ui.centered_and_justified(|ui| {
                    ui.label(
                        egui::RichText::new("Open an image to get started")
                            .size(18.0)
                            .weak(),
                    );
                });
                return;
            }

            let saving = self.save_rx.is_some();
            visible_bottom = ui.clip_rect().bottom();

            egui::Panel::bottom("save_row")
                .show_separator_line(false)
                .show(ui, |ui| {
                    ui.add_space(6.0);
                    ui.columns(2, |cols| {
                        cols[1].vertical_centered(|ui| {
                            if saving {
                                ui.horizontal(|ui| {
                                    ui.spinner();
                                    ui.label("Saving…");
                                });
                            } else {
                                let button =
                                    ui.add_sized([180.0, 30.0], egui::Button::new("Save"));
                                save_button_rect = Some(button.rect);
                                if button.clicked() {
                                    save_clicked = true;
                                }
                            }
                        });
                    });
                    ui.add_space(6.0);
                });

            ui.columns(2, |cols| {
                preview_box(&mut cols[0], "Original", self.original_tex.as_ref());
                preview_box(&mut cols[1], "Processed", self.processed_tex.as_ref());
            });
        });

        #[cfg(test)]
        {
            self.probe = Probe { save_button_rect, visible_bottom };
        }

        if save_clicked {
            self.open_save_modal();
        }
    }
}

fn compress_effort_label(effort: u8) -> &'static str {
    match effort {
        0 => "Fast",
        2 => "Max compression (slower)",
        _ => "Balanced",
    }
}

fn extension_is_known(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| KNOWN_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

fn to_color_image(img: &Image) -> egui::ColorImage {
    egui::ColorImage::from_rgba_unmultiplied(
        [img.width() as usize, img.height() as usize],
        img.as_rgba_bytes(),
    )
}

fn set_texture(
    ctx: &egui::Context,
    slot: &mut Option<egui::TextureHandle>,
    name: &str,
    image: egui::ColorImage,
) {
    match slot {
        Some(handle) => handle.set(image, egui::TextureOptions::LINEAR),
        None => *slot = Some(ctx.load_texture(name, image, egui::TextureOptions::LINEAR)),
    }
}

fn preview_box(ui: &mut egui::Ui, title: &str, tex: Option<&egui::TextureHandle>) {
    ui.vertical_centered(|ui| {
        ui.strong(title);
    });
    ui.add_space(6.0);

    let frame = egui::Frame::group(ui.style());
    let inner_height = (ui.available_height() - frame.total_margin().sum().y).max(60.0);

    frame.show(ui, |ui| {
        ui.set_min_size(egui::vec2(ui.available_width(), inner_height));
        ui.centered_and_justified(|ui| match tex {
            Some(tex) => {
                ui.add(egui::Image::from_texture(tex).shrink_to_fit());
            }
            None => {
                ui.label(egui::RichText::new("—").weak());
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_IMAGE: &str = "src/test.png";

    const DEFAULT_VIEWPORT: [f32; 2] = [1280.0, 800.0];

    fn run_frames_sized(
        app: &mut ImageApp,
        size: [f32; 2],
        frames: usize,
        before: impl FnOnce(&mut ImageApp, &egui::Context),
    ) {
        let ctx = egui::Context::default();

        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size.into())),
            ..Default::default()
        };

        let mut before = Some(before);
        for _ in 0..frames {
            let _ = ctx.run_ui(input.clone(), |ui| {
                if let Some(f) = before.take() {
                    f(app, &ui.ctx().clone());
                }
                app.draw(ui);
            });
        }
    }

    fn run_frames(app: &mut ImageApp, frames: usize, before: impl FnOnce(&mut ImageApp, &egui::Context)) {
        run_frames_sized(app, DEFAULT_VIEWPORT, frames, before);
    }

    #[test]
    fn empty_state_renders_without_panicking() {
        let mut app = ImageApp::default();
        run_frames(&mut app, 3, |_, _| {});

        assert!(app.source.is_none());
        assert!(app.processed_tex.is_none(), "nothing to process yet");
    }

    #[test]
    fn loading_an_image_builds_both_preview_textures() {
        let mut app = ImageApp::default();
        run_frames(&mut app, 2, |app, ctx| app.load_path(Path::new(TEST_IMAGE), ctx));

        assert!(app.source.is_some(), "full-res source retained");
        assert!(app.original_tex.is_some(), "original preview uploaded");
        assert!(app.processed_tex.is_some(), "processed preview uploaded");
        assert!(!app.preview_dirty, "preview recomputed and flag cleared");
    }

    #[test]
    fn previews_use_the_downscaled_copy_not_the_full_buffer() {
        let mut app = ImageApp::default();
        run_frames(&mut app, 2, |app, ctx| app.load_path(Path::new(TEST_IMAGE), ctx));

        let source = app.source.as_ref().unwrap();
        assert_eq!((source.width(), source.height()), (2560, 1441));

        let expected = [PREVIEW_MAX_EDGE as usize, 450];
        assert_eq!(app.original_tex.as_ref().unwrap().size(), expected);
        assert_eq!(app.processed_tex.as_ref().unwrap().size(), expected);
    }

    #[test]
    fn changing_a_filter_recomputes_the_processed_preview_only() {
        let mut app = ImageApp::default();
        run_frames(&mut app, 2, |app, ctx| app.load_path(Path::new(TEST_IMAGE), ctx));

        let original_id = app.original_tex.as_ref().unwrap().id();

        app.params.invert_on = true;
        app.preview_dirty = true;
        run_frames(&mut app, 1, |_, _| {});

        assert!(!app.preview_dirty, "dirty flag consumed");
        assert_eq!(
            app.original_tex.as_ref().unwrap().id(),
            original_id,
            "the original preview must never be reprocessed"
        );
        assert_eq!(app.source.as_ref().unwrap().width(), 2560);
    }

    #[test]
    fn save_button_is_laid_out_inside_the_visible_area() {
        for size in [[1280.0, 800.0], [900.0, 600.0], [1000.0, 400.0], [700.0, 320.0]] {
            let mut app = ImageApp::default();
            run_frames_sized(&mut app, size, 3, |app, ctx| {
                app.load_path(Path::new(TEST_IMAGE), ctx)
            });

            let rect = app
                .probe
                .save_button_rect
                .unwrap_or_else(|| panic!("Save button was never laid out at {size:?}"));

            assert!(
                rect.bottom() <= app.probe.visible_bottom + 0.5,
                "at {:?} the Save button bottom ({}) fell below the visible area ({})",
                size,
                rect.bottom(),
                app.probe.visible_bottom,
            );
            assert!(
                rect.width() > 0.0 && rect.height() > 0.0,
                "at {size:?} the Save button had no size",
            );
        }
    }

    #[test]
    fn save_modal_defaults_to_the_original_filename_and_folder() {
        let mut app = ImageApp::default();
        run_frames(&mut app, 2, |app, ctx| app.load_path(Path::new(TEST_IMAGE), ctx));

        app.open_save_modal();

        let modal = app.save_modal.as_ref().expect("modal should be open");
        assert_eq!(modal.filename, "test.png", "input pre-filled with original name");
        assert_eq!(modal.dir, Path::new("src"), "defaults to the source folder");
        assert!(modal.dir.join(&modal.filename).exists());
    }

    #[test]
    fn save_modal_renders_and_survives_frames() {
        let mut app = ImageApp::default();
        run_frames(&mut app, 2, |app, ctx| app.load_path(Path::new(TEST_IMAGE), ctx));

        app.open_save_modal();
        run_frames(&mut app, 2, |_, _| {});

        assert!(app.save_modal.is_some(), "modal stays open until cancelled or confirmed");
    }

    #[test]
    fn failing_to_load_reports_an_error_instead_of_panicking() {
        let mut app = ImageApp::default();
        run_frames(&mut app, 2, |app, ctx| {
            app.load_path(Path::new("src/definitely-not-here.png"), ctx)
        });

        assert!(app.source.is_none());
        let status = app.status.as_ref().expect("a status should be set");
        assert!(status.is_error, "load failure surfaces as an error: {}", status.text);
    }

    #[test]
    fn save_worker_writes_full_resolution_output() {
        let target = std::env::temp_dir().join("ipr-ui-save-test.png");
        std::fs::remove_file(&target).ok();

        let mut app = ImageApp::default();
        run_frames(&mut app, 2, |app, ctx| app.load_path(Path::new(TEST_IMAGE), ctx));

        app.params.invert_on = true;
        run_frames(&mut app, 1, |app, ctx| app.start_save(target.clone(), ctx));

        for _ in 0..600 {
            run_frames(&mut app, 1, |_, _| {});
            if app.save_rx.is_none() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        assert!(app.save_rx.is_none(), "save should have completed");
        let status = app.status.as_ref().unwrap();
        assert!(!status.is_error, "save reported an error: {}", status.text);

        let saved = Image::new(&target).expect("output file should exist");
        assert_eq!(
            (saved.width(), saved.height()),
            (2560, 1441),
            "saved at full resolution, not the 800px preview"
        );

        std::fs::remove_file(&target).ok();
    }

    #[test]
    fn save_worker_forces_jpg_extension_when_compress_is_on() {
        let requested = std::env::temp_dir().join("ipr-ui-compress-test.png");
        let expected = requested.with_extension("jpg");
        std::fs::remove_file(&requested).ok();
        std::fs::remove_file(&expected).ok();

        let mut app = ImageApp::default();
        run_frames(&mut app, 2, |app, ctx| app.load_path(Path::new(TEST_IMAGE), ctx));

        app.params.compress_on = true;
        app.params.compress_effort = 2;
        run_frames(&mut app, 1, |app, ctx| app.start_save(requested.clone(), ctx));

        for _ in 0..600 {
            run_frames(&mut app, 1, |_, _| {});
            if app.save_rx.is_none() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        assert!(app.save_rx.is_none(), "save should have completed");
        let status = app.status.as_ref().unwrap();
        assert!(!status.is_error, "save reported an error: {}", status.text);
        assert!(
            status.text.ends_with(".jpg"),
            "status should report the forced .jpg path: {}",
            status.text
        );

        assert!(expected.exists(), "compressed output should exist at the forced .jpg path");
        assert!(!requested.exists(), "the originally-requested .png path must not be created");

        std::fs::remove_file(&expected).ok();
    }
}
