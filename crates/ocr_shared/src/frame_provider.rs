use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

/// One tightly packed RGB24 frame borrowed from its provider.
#[derive(Debug, Clone, Copy)]
pub struct Rgb24Frame<'a> {
    pub width: usize,
    pub height: usize,
    pub timestamp_sec: f64,
    pub pixels: &'a [u8],
}

/// Pull-based source for decoded frames. Implementations need only retain the current frame.
pub trait FrameProvider {
    fn next_frame(&mut self) -> anyhow::Result<Option<Rgb24Frame<'_>>>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameProgressPhase {
    Processing,
}

/// Structured progress emitted after each successfully processed frame.
#[derive(Debug, Clone, PartialEq)]
pub struct FrameProgress {
    pub phase: FrameProgressPhase,
    pub completed_frames: usize,
    pub total_frames: Option<usize>,
    pub timestamp_sec: f64,
}

#[derive(Debug, Clone, Default)]
pub struct CancellationToken(Arc<AtomicBool>);

impl CancellationToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

#[derive(Clone, Default)]
pub struct FrameProcessingOptions {
    pub cancellation: CancellationToken,
    pub progress: Option<Arc<dyn Fn(FrameProgress) + Send + Sync>>,
}

impl FrameProcessingOptions {
    pub fn with_progress<F>(mut self, callback: F) -> Self
    where
        F: Fn(FrameProgress) + Send + Sync + 'static,
    {
        self.progress = Some(Arc::new(callback));
        self
    }

    pub fn check_cancelled(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            !self.cancellation.is_cancelled(),
            "OCR frame processing cancelled"
        );
        Ok(())
    }

    pub fn report_progress(&self, completed_frames: usize, timestamp_sec: f64) {
        if let Some(callback) = &self.progress {
            callback(FrameProgress {
                phase: FrameProgressPhase::Processing,
                completed_frames,
                total_frames: None,
                timestamp_sec,
            });
        }
    }
}

/// Checks RGB24 shape and timestamp ordering before a frame enters OCR.
pub fn validate_frame(
    frame: &Rgb24Frame<'_>,
    previous_timestamp_sec: Option<f64>,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        frame.width > 0 && frame.height > 0,
        "frame dimensions must be non-zero"
    );
    anyhow::ensure!(
        frame.timestamp_sec.is_finite() && frame.timestamp_sec >= 0.0,
        "frame timestamp must be finite and non-negative"
    );
    if let Some(previous) = previous_timestamp_sec {
        anyhow::ensure!(
            frame.timestamp_sec > previous,
            "frame timestamps must be strictly increasing"
        );
    }
    let expected_len = frame
        .width
        .checked_mul(frame.height)
        .and_then(|pixels| pixels.checked_mul(3))
        .ok_or_else(|| anyhow::anyhow!("frame dimensions overflow RGB24 buffer size"))?;
    anyhow::ensure!(
        frame.pixels.len() == expected_len,
        "invalid RGB24 frame shape: expected {expected_len} bytes, got {}",
        frame.pixels.len()
    );
    Ok(())
}

/// Validates a normalized ROI and returns its integer pixel bounds.
pub fn roi_bounds(
    width: usize,
    height: usize,
    x_min: f32,
    y_min: f32,
    x_max: f32,
    y_max: f32,
) -> anyhow::Result<(usize, usize, usize, usize)> {
    anyhow::ensure!(width > 0 && height > 0, "frame dimensions must be non-zero");
    let values = [x_min, y_min, x_max, y_max];
    anyhow::ensure!(
        values
            .iter()
            .all(|value| value.is_finite() && (0.0..=1.0).contains(value)),
        "ROI coordinates must be finite normalized values in [0, 1]"
    );
    anyhow::ensure!(
        x_min < x_max && y_min < y_max,
        "ROI bounds must have positive area"
    );

    let left = (width as f64 * x_min as f64).floor() as usize;
    let top = (height as f64 * y_min as f64).floor() as usize;
    let right = (width as f64 * x_max as f64).ceil() as usize;
    let bottom = (height as f64 * y_max as f64).ceil() as usize;
    anyhow::ensure!(
        right > left && bottom > top,
        "ROI must cover at least one pixel"
    );
    Ok((left, top, right, bottom))
}

/// Copies only the requested rows and columns; no other frames are retained.
pub fn crop_rgb24(
    frame: &Rgb24Frame<'_>,
    bounds: (usize, usize, usize, usize),
) -> anyhow::Result<Vec<u8>> {
    validate_frame(frame, None)?;
    let (left, top, right, bottom) = bounds;
    anyhow::ensure!(
        left < right && top < bottom && right <= frame.width && bottom <= frame.height,
        "ROI pixel bounds are outside the frame"
    );
    let row_bytes = frame
        .width
        .checked_mul(3)
        .ok_or_else(|| anyhow::anyhow!("frame row size overflow"))?;
    let roi_width = right - left;
    let roi_row_bytes = roi_width * 3;
    let mut cropped = Vec::with_capacity(roi_row_bytes * (bottom - top));
    for y in top..bottom {
        let start = y * row_bytes + left * 3;
        cropped.extend_from_slice(&frame.pixels[start..start + roi_row_bytes]);
    }
    Ok(cropped)
}

/// Returns a segment's timestamp range using source timestamps, not frame indices.
pub fn timestamp_range(
    timestamps: &[f64],
    start_index: usize,
    end_index: usize,
    fallback_fps: u32,
) -> anyhow::Result<(f64, f64)> {
    anyhow::ensure!(
        start_index <= end_index && end_index < timestamps.len(),
        "invalid frame range"
    );
    anyhow::ensure!(
        timestamps
            .iter()
            .all(|timestamp| timestamp.is_finite() && *timestamp >= 0.0)
            && timestamps.windows(2).all(|pair| pair[1] > pair[0]),
        "timestamps must be finite, non-negative, and strictly increasing"
    );
    let start = timestamps[start_index];
    let end = timestamps
        .get(end_index + 1)
        .copied()
        .unwrap_or(start_or_fallback_end(timestamps[end_index], fallback_fps));
    anyhow::ensure!(end > start, "frame range has no positive duration");
    Ok((start, end))
}

fn start_or_fallback_end(timestamp: f64, fallback_fps: u32) -> f64 {
    timestamp + 1.0 / fallback_fps.max(1) as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_rgb24_shape_and_timestamps() {
        let pixels = [0; 18];
        let frame = Rgb24Frame {
            width: 3,
            height: 2,
            timestamp_sec: 1.0,
            pixels: &pixels,
        };
        assert!(validate_frame(&frame, None).is_ok());
        assert!(validate_frame(&frame, Some(1.0)).is_err());
        assert!(validate_frame(&Rgb24Frame { width: 0, ..frame }, None).is_err());
        assert!(
            validate_frame(
                &Rgb24Frame {
                    pixels: &pixels[..17],
                    ..frame
                },
                None
            )
            .is_err()
        );
        assert!(
            validate_frame(
                &Rgb24Frame {
                    timestamp_sec: f64::NAN,
                    ..frame
                },
                None
            )
            .is_err()
        );
    }

    #[test]
    fn normalized_roi_crops_expected_rgb24_pixels() {
        let pixels: Vec<u8> = (0..4 * 3 * 3).collect();
        let frame = Rgb24Frame {
            width: 4,
            height: 3,
            timestamp_sec: 0.0,
            pixels: &pixels,
        };
        let bounds = roi_bounds(4, 3, 0.25, 1.0 / 3.0, 0.75, 1.0).unwrap();
        assert_eq!(bounds, (1, 1, 3, 3));
        assert_eq!(
            crop_rgb24(&frame, bounds).unwrap(),
            [15, 16, 17, 18, 19, 20, 27, 28, 29, 30, 31, 32]
        );
        assert!(roi_bounds(4, 3, 0.8, 0.0, 0.2, 1.0).is_err());
    }

    #[test]
    fn preserves_provider_timestamps_and_cancellation() {
        assert_eq!(
            timestamp_range(&[0.2, 0.7, 1.4], 1, 1, 10).unwrap(),
            (0.7, 1.4)
        );
        assert!(timestamp_range(&[0.7, 0.6], 0, 0, 10).is_err());
        let token = CancellationToken::new();
        let options = FrameProcessingOptions {
            cancellation: token.clone(),
            progress: None,
        };
        assert!(options.check_cancelled().is_ok());
        token.cancel();
        assert!(options.check_cancelled().is_err());
    }
}
