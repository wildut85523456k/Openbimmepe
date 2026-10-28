//! Graph symbols rasterized from vector primitives at the physical display scale.
use eframe::egui::{self, Color32, Vec2};
pub fn image(ui: &egui::Ui, id: &str, dark: bool, size: Vec2) -> egui::Image<'static> {
    let side = (size.x*ui.ctx().pixels_per_point()).ceil().clamp(16.,512.) as usize;
    let key = egui::Id::unique(("automation-icon",id.to_owned(),dark,side));
    let cached = ui.ctx().data_mut(|d| d.get_temp::<egui::TextureHandle>(key));
    let texture = cached.unwrap_or_else(|| {
        let mut pixels = Vec::with_capacity(side*side);
        for y in 0..side { for x in 0..side {
            let p = Vec2::new((x as f32+0.5)/side as f32,(y as f32+0.5)/side as f32);
            let mut distance = f32::MAX;
            for (a,b) in [(Vec2::new(0.23,0.3),Vec2::new(0.73,0.3)),(Vec2::new(0.23,0.3),Vec2::new(0.73,0.75))] {
                let v = b-a;
                let t = ((p-a).dot(v)/v.length_sq()).clamp(0.,1.);
                distance = distance.min((p-a-v*t).length()-0.027);
            }
            for center in [Vec2::new(0.23,0.3),Vec2::new(0.73,0.3),Vec2::new(0.73,0.75)] {
                distance = distance.min(((p-center).length()-0.10).abs()-0.025);
            }
            if id == "automation_player" && p.x > 0.37 && p.x < 0.68 && (p.y-0.70).abs() < (0.68-p.x)*0.65 { distance = -1.; }
            if id=="automation_mcp" {
                for cy in [0.48,0.62,0.76] {
                    let q=(p-Vec2::new(0.43,cy)).abs()-Vec2::new(0.19,0.045);
                    distance=distance.min(q.x.max(q.y));
                }
            }
            if id=="automation_snapshot" && p.x>0.27 && p.x<0.58 && p.y>0.50 && p.y<0.80 {
                let edge=(p.x-0.27).min(0.58-p.x).min((p.y-0.50).min(0.80-p.y));
                distance=distance.min(edge-0.025);
            }
            let alpha = ((0.5-distance*side as f32).clamp(0.,1.)*255.) as u8;
            let (r,g,b) = if dark { (110,200,250) } else { (25,100,165) };
            pixels.push(Color32::from_rgba_unmultiplied(r,g,b,alpha));
        }}
        let handle = ui.ctx().load_texture(format!("{id}/{dark}/{side}"),egui::ColorImage::new([side,side],pixels),egui::TextureOptions::LINEAR);
        ui.ctx().data_mut(|d| d.insert_temp(key,handle.clone()));
        handle
    });
    egui::Image::new((texture.id(),size)).fit_to_exact_size(size)
}
