/// A video format offered by the camera.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VideoFormat {
    pub index: u32,
    pub width: u32,
    pub height: u32,
    pub fps: f32,
}

/// Picks the format to preview: the largest one not taller than `max_height`, preferring frame rates up
/// to 30 fps and then the order the driver lists them in; when every format is taller, the smallest one.
#[must_use]
pub fn pick_preview_format(formats: &[VideoFormat], max_height: u32) -> Option<u32> {
    let smooth = |format: &VideoFormat| format.fps.min(30.0);
    let fitting = formats.iter().filter(|format| format.height <= max_height).max_by(|a, b| {
        (a.width * a.height)
            .cmp(&(b.width * b.height))
            .then(smooth(a).total_cmp(&smooth(b)))
            .then(b.index.cmp(&a.index))
    });
    fitting
        .or_else(|| formats.iter().min_by_key(|format| format.width * format.height))
        .map(|format| format.index)
}

/// One preview frame as tightly packed, opaque BGRA rows from top to bottom.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewFrame {
    pub width: u32,
    pub height: u32,
    pub bgra: Vec<u8>,
}

/// Packs an RGB32 buffer into a [`PreviewFrame`]; a negative `stride` marks a bottom-up image.
#[must_use]
pub fn pack_bgra(data: &[u8], width: u32, height: u32, stride: i32) -> Option<PreviewFrame> {
    let row_bytes = u128::from(width) * 4;
    let rows = u128::from(height);
    let pitch = u128::from(stride.unsigned_abs());
    if row_bytes == 0 || rows == 0 || pitch < row_bytes || pitch * (rows - 1) + row_bytes > data.len() as u128
    {
        return None;
    }
    #[allow(clippy::cast_possible_truncation)]
    let (row_bytes, rows, pitch) = (row_bytes as usize, rows as usize, pitch as usize);
    let mut bgra = Vec::with_capacity(row_bytes * rows);
    for row in 0..rows {
        let source_row = if stride < 0 { rows - 1 - row } else { row };
        let start = source_row * pitch;
        bgra.extend_from_slice(&data[start..start + row_bytes]);
    }
    for pixel in bgra.as_chunks_mut::<4>().0 {
        pixel[3] = u8::MAX;
    }
    Some(PreviewFrame { width, height, bgra })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn format(index: u32, width: u32, height: u32, fps: f32) -> VideoFormat {
        VideoFormat { index, width, height, fps }
    }

    #[test]
    fn picks_largest_format_within_height() {
        let formats = [
            format(0, 1920, 1080, 30.0),
            format(1, 1280, 720, 30.0),
            format(2, 640, 480, 30.0),
            format(3, 1280, 720, 10.0),
        ];
        assert_eq!(pick_preview_format(&formats, 720), Some(1));
    }

    #[test]
    fn prefers_smooth_frame_rates_but_not_above_thirty() {
        let formats = [format(0, 1280, 720, 60.0), format(1, 1280, 720, 30.0), format(2, 1280, 720, 15.0)];
        assert_eq!(pick_preview_format(&formats, 720), Some(0));
        let capped = [format(0, 1280, 720, 30.0), format(1, 1280, 720, 60.0)];
        assert_eq!(pick_preview_format(&capped, 720), Some(0));
    }

    #[test]
    fn falls_back_to_smallest_format() {
        let formats = [format(0, 3840, 2160, 30.0), format(1, 1920, 1080, 30.0)];
        assert_eq!(pick_preview_format(&formats, 720), Some(1));
        assert_eq!(pick_preview_format(&[], 720), None);
    }

    #[test]
    fn packs_top_down_rows_and_drops_padding() {
        let data = [1, 2, 3, 0, 4, 5, 6, 0, 9, 9, 7, 8, 9, 0, 10, 11, 12, 0, 9, 9];
        let frame = pack_bgra(&data, 2, 2, 10).unwrap();
        assert_eq!(frame.bgra, [1, 2, 3, 255, 4, 5, 6, 255, 7, 8, 9, 255, 10, 11, 12, 255]);
        assert_eq!((frame.width, frame.height), (2, 2));
    }

    #[test]
    fn flips_bottom_up_images() {
        let data = [1, 1, 1, 0, 2, 2, 2, 0];
        let frame = pack_bgra(&data, 1, 2, -4).unwrap();
        assert_eq!(frame.bgra, [2, 2, 2, 255, 1, 1, 1, 255]);
    }

    #[test]
    fn rejects_inconsistent_buffers() {
        assert_eq!(pack_bgra(&[0; 16], 0, 2, 4), None);
        assert_eq!(pack_bgra(&[0; 16], 2, 0, 8), None);
        assert_eq!(pack_bgra(&[0; 16], 2, 2, 4), None);
        assert_eq!(pack_bgra(&[0; 15], 2, 2, 8), None);
    }
}
