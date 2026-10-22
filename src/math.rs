use eframe::egui::{self, Ui};
use std::{
    hash::{Hash, Hasher},
    sync::{Arc, OnceLock},
};

fn svg(source: &str, size: f32, color: egui::Color32, display: bool) -> Result<Arc<[u8]>, String> {
    if source.len() > 8_192 {
        return Err("Formula exceeds 8 KB".into());
    }
    static FONT: OnceLock<Result<latex_rust::MathFont, String>> = OnceLock::new();
    let font = FONT
        .get_or_init(|| latex_rust::MathFont::stix_two_math().map_err(|e| e.to_string()))
        .as_ref()
        .map_err(Clone::clone)?;
    let mut options = latex_rust::SvgOptions::new();
    options.font_size_pt = latex_rust::Dim::from_i64((size * 0.75).round() as i64);
    options.color = latex_rust::Color::rgb(color.r(), color.g(), color.b());
    options.display = display;
    let source = normalize_environments(source);
    latex_rust::latex_to_svg(&source, font, &options)
        .map(|s| Arc::from(s.into_bytes()))
        .map_err(|e| e.to_string())
}
fn normalize_environments(source: &str) -> String {
    // LaTeX-Rust supports aligned rows but rejects starred environment names.
    // Preserve the unnumbered meaning of align* rather than numbering its rows.
    source
        .replace("\\begin{align*}", "\\begin{aligned}")
        .replace("\\end{align*}", "\\end{aligned}")
        .replace("\\begin{equation*}", "\\begin{aligned}")
        .replace("\\end{equation*}", "\\end{aligned}")
        .replace("\\begin{displaymath}", "\\begin{aligned}")
        .replace("\\end{displaymath}", "\\end{aligned}")
        .replace("\\begin{gather*}", "\\begin{aligned}")
        .replace("\\end{gather*}", "\\end{aligned}")
        .replace("\\begin{multline*}", "\\begin{aligned}")
        .replace("\\end{multline*}", "\\end{aligned}")
}
pub fn show(ui: &mut Ui, source: &str, size: f32, display: bool) -> egui::Response {
    let color = ui.visuals().text_color();
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    (source, size.to_bits(), color, display).hash(&mut hash);
    let key = hash.finish();
    let result = ui.ctx().data_mut(|data| {
        let id = egui::Id::new(("math", key));
        if let Some(result) = data.get_temp::<Result<Arc<[u8]>, String>>(id) {
            return result;
        }
        let result = svg(source, size, color, display);
        data.insert_temp(id, result.clone());
        result
    });
    match result {
        Ok(bytes) => {
            let image = egui::Image::from_bytes(format!("bytes://math-{key}.svg"), bytes.clone());
            let natural = natural_size(&bytes).unwrap_or(egui::vec2(size * 4.0, size * 1.5));
            let scale = (ui.available_width() / natural.x).min(1.0);
            if display {
                let width = ui.available_width();
                return egui::ScrollArea::horizontal()
                    .id_salt(("display-formula", key))
                    .show(ui, |ui| {
                        ui.set_min_width(width);
                        ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                            ui.add(image.fit_to_exact_size(natural).sense(egui::Sense::click()))
                        })
                        .inner
                    })
                    .inner;
            }
            ui.add(
                image
                    .fit_to_exact_size(natural * scale)
                    .sense(egui::Sense::click()),
            )
        }
        Err(error) => {
            if display {
                egui::Frame::new()
                    .fill(ui.visuals().code_bg_color)
                    .corner_radius(6)
                    .inner_margin(12)
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new("Couldn’t render formula").strong());
                        ui.label(egui::RichText::new(source).monospace());
                        ui.label(egui::RichText::new(error).small().weak());
                    })
                    .response
                    .interact(egui::Sense::click())
            } else {
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(source)
                            .monospace()
                            .color(ui.visuals().error_fg_color),
                    )
                    .sense(egui::Sense::click()),
                )
                .on_hover_text(format!("{error}\nClick to edit the formula"))
            }
        }
    }
}
fn natural_size(bytes: &[u8]) -> Option<egui::Vec2> {
    let svg = std::str::from_utf8(bytes).ok()?;
    let values: Vec<f32> = svg
        .split("viewBox=\"")
        .nth(1)?
        .split('"')
        .next()?
        .split_whitespace()
        .filter_map(|n| n.parse().ok())
        .collect();
    match values.as_slice() {
        [_, _, w, h] if *w > 0.0 && *h > 0.0 && w.is_finite() && h.is_finite() => {
            Some(egui::vec2(*w, *h) * (4.0 / 3.0))
        }
        _ => None,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn matrices_aligned_equations_and_operators_keep_natural_size() {
        for source in [
            r"\begin{pmatrix}1&2\\3&4\end{pmatrix}",
            r"\begin{aligned}a&=b+c\\d&=e-f\end{aligned}",
            r"\sum_{i=1}^{n}i^2",
            r"\int_0^1 x^2\,dx",
            r"\dfrac{1}{1+x}",
            r"\begin{align*}a&=b+c\\d&=e-f\end{align*}",
            r"\begin{equation}E=mc^2\tag{A}\end{equation}",
            r"\begin{equation*}E=mc^2\end{equation*}",
            r"\begin{gather*}a=b\\c=d\end{gather*}",
            r"\begin{multline*}a+b\\+c=d\end{multline*}",
            r"\begin{displaymath}x^2\end{displaymath}",
            r"\begin{cases}x^2 & x>0\\0 & x\leq0\end{cases}",
        ] {
            let image = svg(source, 18.0, egui::Color32::WHITE, true)
                .unwrap_or_else(|e| panic!("{source}: {e}"));
            let size = natural_size(&image).unwrap();
            assert!(size.is_finite() && size.x > 0.0 && size.y > 0.0);
        }
    }
    #[test]
    fn renders_real_fractions_roots_and_rejects_invalid_source() {
        let image = svg(r"\frac{1}{2}+\sqrt{x^2}", 18.0, egui::Color32::WHITE, true).unwrap();
        let text = std::str::from_utf8(&image).unwrap();
        assert!(text.contains("<path"));
        assert!(svg(r"\notarealcommand", 18.0, egui::Color32::WHITE, false).is_err());
    }
}
