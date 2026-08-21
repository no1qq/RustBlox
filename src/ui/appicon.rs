const ICON: &[u8] = include_bytes!("../../assets/rustblox.ico");
const WINDOW_ICON_SIDE: u32 = 128;
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

fn decode_entry(side: u32, offset: usize, size: usize) -> Option<Vec<u8>> {
    decode_dib(ICON.get(offset..offset + size)?, side)
}

fn nearest_at_least(wanted: u32) -> Option<Image> {
    let mut candidates = entries(ICON);
    candidates.sort_by_key(|(side, _, _)| *side);

    let usable =
        |(side, offset, size): &&(u32, usize, usize)| decode_entry(*side, *offset, *size).is_some();

    let chosen = candidates
        .iter()
        .filter(usable)
        .find(|(side, _, _)| *side >= wanted)
        .or_else(|| candidates.iter().rfind(usable))?;

    Some(Image {
        rgba: decode_entry(chosen.0, chosen.1, chosen.2)?,
        side: chosen.0,
    })
}

fn premultiply(rgba: &[u8]) -> Vec<f32> {
    rgba.chunks_exact(4)
        .flat_map(|pixel| {
            let alpha = pixel[3] as f32 / 255.0;
            [
                pixel[0] as f32 * alpha,
                pixel[1] as f32 * alpha,
                pixel[2] as f32 * alpha,
                pixel[3] as f32,
            ]
        })
        .collect()
}

fn box_resample(source: &Image, wanted: u32) -> Image {
    let from = source.side as usize;
    let to = wanted as usize;
    let premultiplied = premultiply(&source.rgba);
    let mut out = vec![0u8; to * to * 4];
    let ratio = from as f32 / to as f32;

    for y in 0..to {
        let top = (y as f32 * ratio).floor() as usize;
        let bottom = (((y + 1) as f32 * ratio).ceil() as usize)
            .min(from)
            .max(top + 1);
        for x in 0..to {
            let left = (x as f32 * ratio).floor() as usize;
            let right = (((x + 1) as f32 * ratio).ceil() as usize)
                .min(from)
                .max(left + 1);

            let mut sum = [0.0f32; 4];
            let mut count = 0.0f32;
            for sy in top..bottom {
                for sx in left..right {
                    let at = (sy * from + sx) * 4;
                    for (channel, total) in sum.iter_mut().enumerate() {
                        *total += premultiplied[at + channel];
                    }
                    count += 1.0;
                }
            }

            let at = (y * to + x) * 4;
            for (channel, total) in sum.iter().enumerate() {
                out[at + channel] = (total / count).round().clamp(0.0, 255.0) as u8;
            }
        }
    }

    Image {
        rgba: out,
        side: wanted,
    }
}

fn premultiplied_bytes(source: &Image) -> Vec<u8> {
    premultiply(&source.rgba)
        .into_iter()
        .map(|value| value.round().clamp(0.0, 255.0) as u8)
        .collect()
}

pub fn load() -> Option<Image> {
    nearest_at_least(WINDOW_ICON_SIDE)
}

pub fn load_at(wanted: u32) -> Option<Image> {
    let wanted = wanted.clamp(8, 256);
    let source = nearest_at_least(wanted)?;
    if source.side <= wanted {
        return Some(Image {
            rgba: premultiplied_bytes(&source),
            side: source.side,
        });
    }
    Some(box_resample(&source, wanted))
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

fn texture_at(ctx: &egui::Context, side: u32) -> Option<egui::TextureHandle> {
    let key = egui::Id::new(("rustblox-logo-texture", side));
    if let Some(handle) = ctx.data(|data| data.get_temp::<egui::TextureHandle>(key)) {
        return Some(handle);
    }

    let image = load_at(side)?;
    let size = [image.side as usize, image.side as usize];
    let handle = ctx.load_texture(
        format!("rustblox-logo-{side}"),
        egui::ColorImage::from_rgba_premultiplied(size, &image.rgba),
        egui::TextureOptions::LINEAR.with_mipmap_mode(Some(egui::TextureFilter::Linear)),
    );
    ctx.data_mut(|data| data.insert_temp(key, handle.clone()));
    Some(handle)
}

fn cache_step(value: f32) -> u32 {
    const STEP: f32 = 8.0;
    ((value / STEP).ceil() * STEP).clamp(8.0, 256.0) as u32
}

pub fn texture(ctx: &egui::Context, points: f32) -> Option<egui::TextureHandle> {
    texture_at(ctx, cache_step(points * ctx.pixels_per_point()))
}

pub fn paint(ui: &egui::Ui, rect: egui::Rect, rounding: egui::CornerRadius) {
    let Some(handle) = texture(ui.ctx(), rect.width().max(rect.height())) else {
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
    fn the_window_icon_uses_a_large_entry() {
        let image = load().expect("the embedded icon should decode");
        assert_eq!(image.side, WINDOW_ICON_SIDE);
    }

    #[test]
    fn a_small_request_is_resampled_rather_than_stretched() {
        let image = load_at(16).expect("the embedded icon should decode");
        assert_eq!(image.side, 16);
        assert_eq!(image.rgba.len(), 16 * 16 * 4);
    }

    #[test]
    fn resampling_keeps_the_middle_of_the_logo_orange() {
        let image = load_at(24).expect("the embedded icon should decode");
        let side = image.side as usize;
        let middle = ((side / 2) * side + side / 2) * 4;

        assert_eq!(image.rgba[middle + 3], 255);
        assert!(image.rgba[middle] > image.rgba[middle + 1]);
        assert!(image.rgba[middle + 1] > image.rgba[middle + 2]);
    }

    #[test]
    fn a_request_larger_than_every_usable_entry_is_never_stretched_on_the_cpu() {
        let image = load_at(256).expect("the embedded icon should decode");
        assert_eq!(image.side, WINDOW_ICON_SIDE);
    }

    #[test]
    fn sizes_are_rounded_up_to_a_cache_step() {
        assert_eq!(cache_step(15.0), 16);
        assert_eq!(cache_step(16.0), 16);
        assert_eq!(cache_step(17.0), 24);
        assert_eq!(cache_step(1.0), 8);
    }
}
