use rayon::iter::ParallelIterator;
use rayon::iter::IndexedParallelIterator;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
use eframe::{egui, App};
use image::{DynamicImage, GenericImageView, Rgba, RgbaImage};
use rayon::prelude::ParallelSliceMut;

mod flags;
use crate::flags::*;

struct MyApp {
    img_path: String,
    img: Option<DynamicImage>,
    selected_flag: usize,
    blend_factor: f32,
    output_image: Option<RgbaImage>,
    egui_texture: Option<egui::TextureHandle>,
    needs_update: bool,
    update_sender: Sender<RgbaImage>,
    update_receiver: Receiver<RgbaImage>,
    computing: bool,
}

impl Default for MyApp {
    fn default() -> Self {
        let (tx, rx) = channel();
        Self {
            img_path: "".to_owned(),
            img: None,
            selected_flag: 0,
            blend_factor: 0.5,
            output_image: None,
            egui_texture: None,
            needs_update: false,
            update_sender: tx,
            update_receiver: rx,
            computing: false,
        }
    }
}

impl MyApp {
    fn update_texture(&mut self, ctx: &egui::Context) {
        if let Some(img) = &self.output_image {
            let size = img.dimensions();
            let texture = ctx.load_texture(
                "output_image",
                egui::ColorImage::from_rgba_unmultiplied(
                    [size.0 as usize, size.1 as usize],
                    img,
                ),
                egui::TextureOptions::default(),
            );
            self.egui_texture = Some(texture);
        }
    }
}


impl App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            // open file
            ui.horizontal(|ui| {
                ui.label("Chemin de l'image:");
                ui.text_edit_singleline(&mut self.img_path);
                if ui.button("Ouvrir un fichier image").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("Image", &["png", "jpg", "jpeg", "bmp"])
                        .pick_file()
                    {
                        self.img_path = path.to_string_lossy().to_string();

                        match image::open(&self.img_path) {
                            Ok(img) => {
                                self.img = Some(img.clone());
                                self.output_image = Some(img.to_rgba8());
                                self.update_texture(ctx);  // crée la texture egui
                            }
                            Err(e) => {
                                eprintln!("Erreur chargement image: {}", e);
                            }
                        }
                    }
                    self.needs_update = true;
                }

            });
            ui.separator();
            // select flag
            ui.horizontal(|ui| {
                ui.label("Sélection du drapeau :");
                egui::ComboBox::from_label("")
                    .selected_text(format!("{:?}", ALL_FLAGS[self.selected_flag]))
                    .show_ui(ui, |ui| {
                        for (i, flag) in ALL_FLAGS.iter().enumerate() {
                            if ui.selectable_value(&mut self.selected_flag, i, format!("{:?}", flag)).clicked() {
                                self.needs_update = true;
                            }
                        }
                    });
            });
            // blend factor
            if ui.add(egui::Slider::new(&mut self.blend_factor, 0.0..=1.0).text("Blend factor")).changed() {
                self.needs_update = true;
            }
            // save image
            if let Some(output) = &self.output_image {
                if ui.button("Sauvegarder").clicked() {
                    let save_path = "output_flagged.png";
                    if let Err(e) = output.save(save_path) {
                        eprintln!("Erreur sauvegarde : {}", e);
                    } else {
                        println!("Image sauvegardée dans {}", save_path);
                    }
                }
            }
            if self.needs_update && !self.computing {
                if let Some(original) = &self.img {
                    let flag_data = create_pride_flag_overlay(&ALL_FLAGS[self.selected_flag], original.width(), original.height());
                    let img_clone = original.clone();
                    let blend_factor = self.blend_factor;
                    let sender = self.update_sender.clone();

                    self.computing = true;  // on indique qu'on calcule

                    thread::spawn(move || {
                        let new_img = apply_flag_overlay_to_image_with_blend(img_clone, flag_data, blend_factor);
                        let _ = sender.send(new_img);
                    });
                }
                self.needs_update = false;
            }
            if let Ok(new_img) = self.update_receiver.try_recv() {
                self.output_image = Some(new_img);
                self.update_texture(ctx);
                self.computing = false;
            }
            ui.separator();
            // show image as texture
            if let Some(texture) = &self.egui_texture {
                //ui.image(texture);
                ui.add(
                    egui::Image::new(texture)
                        .max_size(ui.available_size())
                        .corner_radius(10),
                );
            }
        });
    }
}

fn apply_flag_overlay_to_image_with_blend(img: DynamicImage, flag_data: Vec<u8>, blend_factor: f32) -> RgbaImage {
    //let (width, height) = img.dimensions();
    let mut img_rgba = img.to_rgba8();

    let pixels = img_rgba.as_mut();

    pixels
        .par_chunks_mut(4)
        .enumerate()
        .for_each(|(i, pixel)| {
            let index = i * 4;

            let orig_r = pixel[0] as f32;
            let orig_g = pixel[1] as f32;
            let orig_b = pixel[2] as f32;
            let orig_a = pixel[3] as f32;

            let flag_r = flag_data[index] as f32;
            let flag_g = flag_data[index + 1] as f32;
            let flag_b = flag_data[index + 2] as f32;
            let flag_a = flag_data[index + 3] as f32;

            let blended_r = (orig_r * (1.0 - blend_factor) + flag_r * blend_factor).min(255.0) as u8;
            let blended_g = (orig_g * (1.0 - blend_factor) + flag_g * blend_factor).min(255.0) as u8;
            let blended_b = (orig_b * (1.0 - blend_factor) + flag_b * blend_factor).min(255.0) as u8;
            let blended_a = (orig_a * (1.0 - blend_factor) + flag_a * blend_factor).min(255.0) as u8;

            pixel[0] = blended_r;
            pixel[1] = blended_g;
            pixel[2] = blended_b;
            pixel[3] = blended_a;
        });

    img_rgba
}

fn main() {
    let options = eframe::NativeOptions::default();
    let _ = eframe::run_native(
        "Pride Flag Overlay GUI",
        options,
        Box::new(|_cc| Ok(Box::new(MyApp::default())),
    ));
}
