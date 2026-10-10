//! 窗口图标：把 `roxy-dbc.ico` 里的一条位图解成 winit 要的 RGBA。
//!
//! exe 在资源管理器里的图标是另一回事，那是 `build.rs` 用 winresource 把同一个
//! .ico 嵌进 PE 资源，运行时不经过这里。

use winit::window::Icon;

const SOURCE: &[u8] = include_bytes!("../roxy-dbc.ico");

/// 标题栏与任务栏要的尺寸：太小会糊，太大由系统缩小反而丢细节
const PREFERRED_WIDTH: usize = 32;

struct Frame {
    width: usize,
    height: usize,
    rgba: Vec<u8>,
}

fn u16_at(bytes: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(bytes.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

/// 供界面用的窗口图标；图标文件读不出可用的条目时返回 None，窗口保持系统默认图标
pub fn window_icon() -> Option<Icon> {
    let frames = frames(SOURCE);
    let frame = choose(&frames)?;
    Icon::from_rgba(frame.rgba.clone(), frame.width as u32, frame.height as u32).ok()
}

fn choose(frames: &[Frame]) -> Option<&Frame> {
    frames
        .iter()
        .filter(|frame| frame.width >= PREFERRED_WIDTH)
        .min_by_key(|frame| frame.width)
        .or_else(|| frames.iter().max_by_key(|frame| frame.width))
}

/// 解出 ICO 里所有 32 位 BMP 条目；PNG 压缩条目要额外解码器，跳过
fn frames(bytes: &[u8]) -> Vec<Frame> {
    let mut out = Vec::new();
    let (Some(kind), Some(count)) = (u16_at(bytes, 2), u16_at(bytes, 4)) else {
        return out;
    };
    if kind != 1 {
        return out;
    }
    for index in 0..count as usize {
        let entry = 6 + index * 16;
        let (Some(raw_width), Some(raw_height), Some(bits), Some(size), Some(offset)) = (
            bytes.get(entry).copied(),
            bytes.get(entry + 1).copied(),
            u16_at(bytes, entry + 6),
            u32_at(bytes, entry + 8),
            u32_at(bytes, entry + 12),
        ) else {
            continue;
        };
        // 0 在 ICO 里表示 256
        let width = if raw_width == 0 {
            256
        } else {
            usize::from(raw_width)
        };
        let height = if raw_height == 0 {
            256
        } else {
            usize::from(raw_height)
        };
        let start = offset as usize;
        if size < 40 || bytes.len() < start + size as usize {
            continue;
        }
        if let Some(frame) = decode_bmp(bytes, start, width, height, bits) {
            out.push(frame);
        }
    }
    out
}

/// 条目里是一张没有文件头的 BMP：像素自下而上、BGRA，后面跟 1 位 AND 掩码
fn decode_bmp(bytes: &[u8], start: usize, width: usize, height: usize, bits: u16) -> Option<Frame> {
    if bits != 32 || width == 0 || height == 0 {
        return None;
    }
    let header = u32_at(bytes, start)? as usize;
    let pixels_at = start + header;
    let plane = width * height * 4;
    let pixels = bytes.get(pixels_at..pixels_at + plane)?;

    let mut rgba = vec![0u8; plane];
    for y in 0..height {
        let source = (height - 1 - y) * width * 4;
        let target = y * width * 4;
        for x in 0..width {
            let s = source + x * 4;
            let t = target + x * 4;
            rgba[t] = pixels[s + 2];
            rgba[t + 1] = pixels[s + 1];
            rgba[t + 2] = pixels[s];
            rgba[t + 3] = pixels[s + 3];
        }
    }

    // 有的工具画图标时 alpha 全写 0，形状只记在 AND 掩码里
    if rgba.chunks(4).all(|pixel| pixel[3] == 0) {
        let mask_row = width.div_ceil(32) * 4;
        let mask_at = pixels_at + plane;
        if let Some(mask) = bytes.get(mask_at..mask_at + mask_row * height) {
            for y in 0..height {
                for x in 0..width {
                    let byte = mask[(height - 1 - y) * mask_row + x / 8];
                    let hole = (byte >> (7 - x % 8)) & 1;
                    rgba[(y * width + x) * 4 + 3] = if hole == 0 { 255 } else { 0 };
                }
            }
        }
    }

    Some(Frame {
        width,
        height,
        rgba,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shipped_icon_yields_a_usable_frame() {
        let frames = frames(SOURCE);
        assert!(!frames.is_empty(), "no 32-bit BMP entry in roxy-dbc.ico");
        let frame = choose(&frames).expect("a frame");
        assert!(frame.width >= PREFERRED_WIDTH, "picked {} px", frame.width);
        assert_eq!(frame.rgba.len(), frame.width * frame.height * 4);
        assert!(
            frame.rgba.chunks(4).any(|pixel| pixel[3] != 0),
            "every pixel is transparent: the icon would be invisible"
        );
    }

    #[test]
    fn it_picks_the_entry_near_the_requested_size() {
        let sizes: Vec<usize> = frames(SOURCE).iter().map(|f| f.width).collect();
        assert!(sizes.contains(&32), "sizes: {sizes:?}");
        assert_eq!(choose(&frames(SOURCE)).unwrap().width, 32);
    }

    #[test]
    fn broken_input_gives_no_frames_instead_of_a_panic() {
        assert!(frames(&[]).is_empty());
        assert!(frames(&[0; 32]).is_empty());
        let mut cursor_like = vec![0, 0, 2, 0, 1, 0];
        cursor_like.extend_from_slice(&[0; 32]);
        assert!(frames(&cursor_like).is_empty());
    }

    #[test]
    fn a_window_icon_can_be_built_from_the_shipped_file() {
        assert!(window_icon().is_some());
    }
}
