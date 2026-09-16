use can_dbc::ByteOrder;
use dear_imgui_rs::Ui;

use crate::editable_dbc::{get_signal_bit_positions, EditableMessage};

/// Compute the absolute bit indices occupied by a signal.
/// Bit index N means byte N/8, bit N%8 within that byte.
pub fn get_bit_positions(start_bit: u64, size: u64, byte_order: &ByteOrder) -> Vec<usize> {
    get_signal_bit_positions(start_bit, size, byte_order)
}

const PALETTE: [[f32; 4]; 8] = [
    [0.25, 0.55, 0.95, 0.6],
    [0.95, 0.55, 0.25, 0.6],
    [0.35, 0.8, 0.45, 0.6],
    [0.85, 0.35, 0.75, 0.6],
    [0.95, 0.85, 0.3, 0.6],
    [0.4, 0.85, 0.85, 0.6],
    [0.65, 0.5, 0.95, 0.6],
    [0.9, 0.4, 0.4, 0.6],
];

const CELL_W: f32 = 46.0;
const CELL_H: f32 = 22.0;
const LABEL_W: f32 = 36.0;

/// Render the signal bit-layout diagram. Must be called inside a window.
pub fn render_bit_layout(ui: &Ui, message: &EditableMessage, selected_names: &[String]) {
    let signals = message.signals();
    let num_bytes = message.message_size().max(1) as usize;

    let origin = ui.cursor_screen_pos();
    let total_w = LABEL_W + 8.0 * CELL_W;
    let total_h = 18.0 + num_bytes as f32 * CELL_H + 4.0;
    ui.dummy([total_w, total_h]);

    let dl = ui.get_window_draw_list();

    // Bit number header (bit 7 on the left, DBC matrix style)
    for bit in 0..8 {
        let x = origin[0] + LABEL_W + (7 - bit) as f32 * CELL_W;
        dl.add_text(
            [x + CELL_W / 2.0 - 4.0, origin[1]],
            [0.7, 0.7, 0.7, 1.0],
            format!("{}", bit),
        );
    }

    let grid_top = origin[1] + 18.0;

    // Byte labels and grid cells
    for byte in 0..num_bytes {
        let y = grid_top + byte as f32 * CELL_H;
        dl.add_text(
            [origin[0], y + CELL_H / 2.0 - 6.0],
            [0.7, 0.7, 0.7, 1.0],
            format!("B{}", byte),
        );
        for col in 0..8 {
            let x = origin[0] + LABEL_W + col as f32 * CELL_W;
            dl.add_rect([x, y], [x + CELL_W, y + CELL_H], [0.35, 0.35, 0.35, 1.0])
                .build();
        }
    }

    // Signal cells
    for (i, sig) in signals.iter().enumerate() {
        let color = PALETTE[i % PALETTE.len()];
        let is_selected = selected_names.iter().any(|n| n == sig.name());
        let positions = get_bit_positions(sig.start_bit(), sig.signal_size(), sig.byte_order());

        let mut label_pos: Option<[f32; 2]> = None;
        for &bit in &positions {
            let byte = bit / 8;
            if byte >= num_bytes {
                continue;
            }
            let col = 7 - (bit % 8);
            let x = origin[0] + LABEL_W + col as f32 * CELL_W;
            let y = grid_top + byte as f32 * CELL_H;
            dl.add_rect([x + 1.0, y + 1.0], [x + CELL_W - 1.0, y + CELL_H - 1.0], color)
                .filled(true)
                .build();
            if label_pos.is_none() {
                label_pos = Some([x, y]);
            }
        }

        if let Some([x, y]) = label_pos {
            let border_color = if is_selected {
                [1.0, 1.0, 1.0, 1.0]
            } else {
                [color[0], color[1], color[2], 1.0]
            };
            dl.add_rect([x + 0.5, y + 0.5], [x + CELL_W - 0.5, y + CELL_H - 0.5], border_color)
                .thickness(if is_selected { 2.0 } else { 1.0 })
                .build();
            dl.add_text(
                [x + 3.0, y + CELL_H / 2.0 - 6.0],
                [1.0, 1.0, 1.0, 1.0],
                sig.name(),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::get_bit_positions;
    use can_dbc::ByteOrder;

    #[test]
    fn intel_layout_is_linear() {
        assert_eq!(
            get_bit_positions(8, 4, &ByteOrder::LittleEndian),
            vec![8, 9, 10, 11]
        );
    }

    #[test]
    fn motorola_wraps_to_next_byte() {
        assert_eq!(
            get_bit_positions(7, 16, &ByteOrder::BigEndian),
            vec![7, 6, 5, 4, 3, 2, 1, 0, 15, 14, 13, 12, 11, 10, 9, 8]
        );
    }

    #[test]
    fn motorola_single_byte() {
        assert_eq!(
            get_bit_positions(3, 4, &ByteOrder::BigEndian),
            vec![3, 2, 1, 0]
        );
    }
}
