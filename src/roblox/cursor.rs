use std::path::{Path, PathBuf};

use crate::config::CursorPreset;
use crate::error::{Error, Result};
use crate::util::fs;

fn cursor_dir(mods_root: &Path) -> PathBuf {
    mods_root
        .join("content")
        .join("textures")
        .join("Cursors")
        .join("KeyboardMouse")
}

fn shiftlock_path(mods_root: &Path) -> PathBuf {
    mods_root
        .join("content")
        .join("textures")
        .join("MouseLockedCursor.png")
}

fn encode_png(width: u32, height: u32, rgba_pixels: &[u8]) -> Vec<u8> {
    let mut raw_scanlines = Vec::with_capacity((1 + width as usize * 4) * height as usize);
    for y in 0..height as usize {
        raw_scanlines.push(0);
        let start = y * width as usize * 4;
        let end = start + width as usize * 4;
        raw_scanlines.extend_from_slice(&rgba_pixels[start..end]);
    }

    let compressed = miniz_oxide::deflate::compress_to_vec_zlib(&raw_scanlines, 6);

    let mut png = Vec::new();
    png.extend_from_slice(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);

    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.push(8);
    ihdr.push(6);
    ihdr.push(0);
    ihdr.push(0);
    ihdr.push(0);
    write_chunk(&mut png, b"IHDR", &ihdr);
    write_chunk(&mut png, b"IDAT", &compressed);
    write_chunk(&mut png, b"IEND", &[]);

    png
}

fn write_chunk(png: &mut Vec<u8>, chunk_type: &[u8; 4], data: &[u8]) {
    png.extend_from_slice(&(data.len() as u32).to_be_bytes());
    png.extend_from_slice(chunk_type);
    png.extend_from_slice(data);
    let mut hasher = crc32fast::Hasher::new();
    hasher.update(chunk_type);
    hasher.update(data);
    png.extend_from_slice(&hasher.finalize().to_be_bytes());
}

fn set_pixel(buf: &mut [u8], x: usize, y: usize, r: u8, g: u8, b: u8, a: u8) {
    if x < 32 && y < 32 {
        let idx = (y * 32 + x) * 4;
        buf[idx] = r;
        buf[idx + 1] = g;
        buf[idx + 2] = b;
        buf[idx + 3] = a;
    }
}

pub fn classic_2015_arrow_png() -> Vec<u8> {
    let mut pixels = vec![0u8; 32 * 32 * 4];
    let black = (0, 0, 0, 255);
    let white = (255, 255, 255, 255);

    let shape: &[&str] = &[
        "X...............................",
        "XX..............................",
        "X.X.............................",
        "X..X............................",
        "X...X...........................",
        "X....X..........................",
        "X.....X.........................",
        "X......X........................",
        "X.......X.......................",
        "X........X......................",
        "X.........X.....................",
        "X..........X....................",
        "X...........X...................",
        "X............X..................",
        "X.............X.................",
        "X......XXXXXXX..................",
        "X...X..X........................",
        "X..X.X..X.......................",
        "X.X..X..X.......................",
        "XX....X..X......................",
        "X.....X..X......................",
        ".......X..X.....................",
        ".......X..X.....................",
        "........XX......................",
    ];

    for (y, row) in shape.iter().enumerate() {
        for (x, ch) in row.chars().enumerate() {
            if ch == 'X' {
                set_pixel(&mut pixels, x, y, black.0, black.1, black.2, black.3);
            }
        }
    }

    let fills: &[(usize, usize)] = &[
        (1, 2),
        (1, 3),
        (2, 3),
        (1, 4),
        (2, 4),
        (3, 4),
        (1, 5),
        (2, 5),
        (3, 5),
        (4, 5),
        (1, 6),
        (2, 6),
        (3, 6),
        (4, 6),
        (5, 6),
        (1, 7),
        (2, 7),
        (3, 7),
        (4, 7),
        (5, 7),
        (6, 7),
        (1, 8),
        (2, 8),
        (3, 8),
        (4, 8),
        (5, 8),
        (6, 8),
        (7, 8),
        (1, 9),
        (2, 9),
        (3, 9),
        (4, 9),
        (5, 9),
        (6, 9),
        (7, 9),
        (8, 9),
        (1, 10),
        (2, 10),
        (3, 10),
        (4, 10),
        (5, 10),
        (6, 10),
        (7, 10),
        (8, 10),
        (9, 10),
        (1, 11),
        (2, 11),
        (3, 11),
        (4, 11),
        (5, 11),
        (6, 11),
        (7, 11),
        (8, 11),
        (9, 11),
        (10, 11),
        (1, 12),
        (2, 12),
        (3, 12),
        (4, 12),
        (5, 12),
        (6, 12),
        (7, 12),
        (8, 12),
        (9, 12),
        (10, 12),
        (11, 12),
        (1, 13),
        (2, 13),
        (3, 13),
        (4, 13),
        (5, 13),
        (6, 13),
        (7, 13),
        (8, 13),
        (9, 13),
        (10, 13),
        (11, 13),
        (12, 13),
        (1, 14),
        (2, 14),
        (3, 14),
        (4, 14),
        (5, 14),
        (6, 14),
        (7, 14),
        (8, 14),
        (9, 14),
        (10, 14),
        (11, 14),
        (12, 14),
        (13, 14),
        (1, 15),
        (2, 15),
        (3, 15),
        (4, 15),
        (5, 15),
        (6, 15),
        (1, 16),
        (2, 16),
        (1, 17),
        (7, 17),
        (8, 17),
        (7, 18),
        (8, 18),
        (8, 19),
        (9, 19),
        (8, 20),
        (9, 20),
        (9, 21),
        (10, 21),
        (9, 22),
        (10, 22),
    ];

    for &(x, y) in fills {
        set_pixel(&mut pixels, x, y, white.0, white.1, white.2, white.3);
    }

    encode_png(32, 32, &pixels)
}

pub fn classic_2015_shiftlock_png() -> Vec<u8> {
    let mut pixels = vec![0u8; 32 * 32 * 4];
    let black = (0, 0, 0, 255);
    let white = (255, 255, 255, 255);

    let cx = 15.5_f32;
    let cy = 15.5_f32;

    for y in 0..32 {
        for x in 0..32 {
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            let dist = (dx * dx + dy * dy).sqrt();

            if (dist - 8.0).abs() <= 0.8 {
                set_pixel(&mut pixels, x, y, white.0, white.1, white.2, white.3);
            } else if (dist - 8.0).abs() <= 1.6 {
                set_pixel(&mut pixels, x, y, black.0, black.1, black.2, black.3);
            } else if dist <= 1.5 {
                set_pixel(&mut pixels, x, y, white.0, white.1, white.2, white.3);
            } else if dist <= 2.5 {
                set_pixel(&mut pixels, x, y, black.0, black.1, black.2, black.3);
            }
        }
    }

    encode_png(32, 32, &pixels)
}

pub fn clean_dot_arrow_png() -> Vec<u8> {
    let mut pixels = vec![0u8; 32 * 32 * 4];
    let black = (0, 0, 0, 255);
    let cyan = (0, 255, 255, 255);

    let cx = 15;
    let cy = 15;

    for x in cx - 1..=cx + 2 {
        for y in cy - 1..=cy + 2 {
            set_pixel(&mut pixels, x, y, black.0, black.1, black.2, black.3);
        }
    }
    for x in cx..=cx + 1 {
        for y in cy..=cy + 1 {
            set_pixel(&mut pixels, x, y, cyan.0, cyan.1, cyan.2, cyan.3);
        }
    }

    for offset in 4..=8 {
        set_pixel(&mut pixels, cx - offset, cy, cyan.0, cyan.1, cyan.2, cyan.3);
        set_pixel(&mut pixels, cx + 1 + offset, cy, cyan.0, cyan.1, cyan.2, cyan.3);
        set_pixel(&mut pixels, cx, cy - offset, cyan.0, cyan.1, cyan.2, cyan.3);
        set_pixel(
            &mut pixels,
            cx + 1 + offset,
            cy,
            cyan.0,
            cyan.1,
            cyan.2,
            cyan.3,
        );
    }

    encode_png(32, 32, &pixels)
}

pub fn clean_dot_shiftlock_png() -> Vec<u8> {
    let mut pixels = vec![0u8; 32 * 32 * 4];
    let black = (0, 0, 0, 255);
    let cyan = (0, 255, 255, 255);

    let cx = 15;
    let cy = 15;

    for x in cx - 2..=cx + 3 {
        for y in cy - 2..=cy + 3 {
            set_pixel(&mut pixels, x, y, black.0, black.1, black.2, black.3);
        }
    }
    for x in cx - 1..=cx + 2 {
        for y in cy - 1..=cy + 2 {
            set_pixel(&mut pixels, x, y, cyan.0, cyan.1, cyan.2, cyan.3);
        }
    }

    encode_png(32, 32, &pixels)
}

pub fn apply_cursor_preset(mods_root: &Path, preset: CursorPreset) -> Result<()> {
    match preset {
        CursorPreset::Default => {
            remove_cursor(mods_root)?;
        }
        CursorPreset::Classic2015 => {
            let arrow = classic_2015_arrow_png();
            let shiftlock = classic_2015_shiftlock_png();
            write_cursor_files(mods_root, &arrow, &shiftlock)?;
        }
        CursorPreset::CleanDot => {
            let arrow = clean_dot_arrow_png();
            let shiftlock = clean_dot_shiftlock_png();
            write_cursor_files(mods_root, &arrow, &shiftlock)?;
        }
        CursorPreset::Custom => {}
    }
    Ok(())
}

fn write_cursor_files(mods_root: &Path, arrow_png: &[u8], shiftlock_png: &[u8]) -> Result<()> {
    let dir = cursor_dir(mods_root);
    fs::ensure_dir(&dir)?;
    fs::write_atomic(&dir.join("ArrowCursor.png"), arrow_png)?;
    fs::write_atomic(&dir.join("ArrowFarCursor.png"), arrow_png)?;
    fs::write_atomic(&dir.join("ArrowFarCursorDeclined.png"), arrow_png)?;

    let shift_target = shiftlock_path(mods_root);
    if let Some(parent) = shift_target.parent() {
        fs::ensure_dir(parent)?;
    }
    fs::write_atomic(&shift_target, shiftlock_png)?;
    Ok(())
}

pub fn install_custom_cursor(mods_root: &Path, source: &Path) -> Result<()> {
    if !source.is_file() {
        return Err(Error::invalid("Please select a valid image file"));
    }
    let data = std::fs::read(source)
        .map_err(|err| Error::io(format!("Could not read {}", source.display()), err))?;

    write_cursor_files(mods_root, &data, &data)?;
    Ok(())
}

pub fn remove_cursor(mods_root: &Path) -> Result<()> {
    let dir = cursor_dir(mods_root);
    let _ = std::fs::remove_file(dir.join("ArrowCursor.png"));
    let _ = std::fs::remove_file(dir.join("ArrowFarCursor.png"));
    let _ = std::fs::remove_file(dir.join("ArrowFarCursorDeclined.png"));
    let _ = std::fs::remove_file(shiftlock_path(mods_root));

    let _ = std::fs::remove_dir(&dir);
    if let Some(parent) = dir.parent() {
        let _ = std::fs::remove_dir(parent);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_valid_png_headers() {
        let arrow = classic_2015_arrow_png();
        assert_eq!(
            &arrow[0..8],
            &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]
        );
        let dot = clean_dot_arrow_png();
        assert_eq!(&dot[0..8], &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
    }

    #[test]
    fn installs_and_removes_cursor_files() {
        let dir = tempfile::tempdir().unwrap();
        apply_cursor_preset(dir.path(), CursorPreset::Classic2015).unwrap();

        assert!(cursor_dir(dir.path()).join("ArrowCursor.png").is_file());
        assert!(shiftlock_path(dir.path()).is_file());

        remove_cursor(dir.path()).unwrap();
        assert!(!cursor_dir(dir.path()).join("ArrowCursor.png").exists());
        assert!(!shiftlock_path(dir.path()).exists());
    }
}

