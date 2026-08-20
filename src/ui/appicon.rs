const ICON: &[u8] = include_bytes!("../../assets/rustblox.ico");
const PREFERRED: u32 = 128;
const HEADER: usize = 40;

pub struct Image {
    pub rgba: Vec<u8>,
    pub side: u32,
}

fn u16_at(bytes: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(bytes.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

fn entries(icon: &[u8]) -> Vec<(u32, usize, usize)> {
    let count = u16_at(icon, 4).unwrap_or(0) as usize;
    let mut found = Vec::new();

    for index in 0..count {
        let at = 6 + index * 16;
        let Some(width) = icon.get(at).copied() else {
            continue;
        };
        let side = if width == 0 { 256 } else { width as u32 };
        let Some(size) = u32_at(icon, at + 8) else {
            continue;
        };
        let Some(offset) = u32_at(icon, at + 12) else {
            continue;
        };
        found.push((side, offset as usize, size as usize));
    }

    found
}

fn decode_dib(data: &[u8], side: u32) -> Option<Vec<u8>> {
    if u32_at(data, 0)? as usize != HEADER || u16_at(data, 14)? != 32 {
        return None;
    }

    let side = side as usize;
    let pixels = data.get(HEADER..HEADER + side * side * 4)?;
    let mut rgba = vec![0u8; side * side * 4];

    for y in 0..side {
        let source = (side - 1 - y) * side * 4;
        for x in 0..side {
            let from = source + x * 4;
            let to = (y * side + x) * 4;
            rgba[to] = pixels[from + 2];
            rgba[to + 1] = pixels[from + 1];
            rgba[to + 2] = pixels[from];
            rgba[to + 3] = pixels[from + 3];
        }
    }

    Some(rgba)
}

pub fn load() -> Option<Image> {
    let mut candidates = entries(ICON);
    candidates.sort_by_key(|(side, _, _)| {
        let side = *side as i64;
        (side - PREFERRED as i64).abs()
    });

    for (side, offset, size) in candidates {
        let Some(data) = ICON.get(offset..offset + size) else {
            continue;
        };
        if let Some(rgba) = decode_dib(data, side) {
            return Some(Image { rgba, side });
        }
    }

    None
}

pub fn window_icon() -> egui::IconData {
    match load() {
        Some(image) => egui::IconData {
            rgba: image.rgba,
            width: image.side,
            height: image.side,
        },
        None => egui::IconData {
            rgba: vec![251, 86, 6, 255],
            width: 1,
            height: 1,
        },
    }
}

pub fn texture(ctx: &egui::Context) -> Option<egui::TextureHandle> {
    let key = egui::Id::new("rustblox-logo-texture");
    if let Some(handle) = ctx.data(|data| data.get_temp::<egui::TextureHandle>(key)) {
        return Some(handle);
    }

    let image = load()?;
    let size = [image.side as usize, image.side as usize];
    let handle = ctx.load_texture(
        "rustblox-logo",
        egui::ColorImage::from_rgba_unmultiplied(size, &image.rgba),
        egui::TextureOptions::LINEAR,
    );
    ctx.data_mut(|data| data.insert_temp(key, handle.clone()));
    Some(handle)
}

pub fn paint(ui: &egui::Ui, rect: egui::Rect, rounding: egui::CornerRadius) {
    let Some(handle) = texture(ui.ctx()) else {
        ui.painter()
            .rect_filled(rect, rounding, egui::Color32::from_rgb(251, 86, 6));
        return;
    };

    ui.painter().add(
        egui::epaint::RectShape::filled(rect, rounding, egui::Color32::WHITE).with_texture(
            handle.id(),
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
        ),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_embedded_file_is_an_icon() {
        assert_eq!(u16_at(ICON, 0), Some(0));
        assert_eq!(u16_at(ICON, 2), Some(1));
        assert!(u16_at(ICON, 4).unwrap_or(0) > 0);
    }

    #[test]
    fn every_listed_entry_sits_inside_the_file() {
        for (side, offset, size) in entries(ICON) {
            assert!(side > 0 && side <= 256, "odd side {side}");
            assert!(
                offset + size <= ICON.len(),
                "entry {side} runs past the end of the file"
            );
        }
    }

    #[test]
    fn the_icon_decodes_to_square_rgba() {
        let image = load().expect("the embedded icon should decode");
        assert_eq!(image.rgba.len(), (image.side * image.side * 4) as usize);
        assert!(image.side >= 32);
    }

    #[test]
    fn the_decoded_icon_is_the_orange_logo() {
        let image = load().expect("the embedded icon should decode");
        let side = image.side as usize;
        let middle = ((side / 2) * side + side / 2) * 4;

        assert_eq!(image.rgba[middle], 251);
        assert_eq!(image.rgba[middle + 1], 86);
        assert_eq!(image.rgba[middle + 2], 6);
        assert_eq!(image.rgba[middle + 3], 255);
    }

    #[test]
    fn the_preferred_size_is_chosen_when_it_exists() {
        let image = load().expect("the embedded icon should decode");
        assert_eq!(image.side, PREFERRED);
    }
}
