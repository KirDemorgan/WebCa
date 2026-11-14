use windows::Win32::Media::MediaFoundation::{
    IMFAttributes, IMFMediaType, IMFSample, IMFSourceReader, MF_E_NO_MORE_TYPES, MF_MT_DEFAULT_STRIDE,
    MF_MT_FRAME_RATE, MF_MT_FRAME_SIZE, MF_MT_MAJOR_TYPE, MF_MT_SUBTYPE,
    MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING, MF_SOURCE_READER_FIRST_VIDEO_STREAM,
    MF_SOURCE_READERF_ENDOFSTREAM, MF_SOURCE_READERF_ERROR, MFCreateAttributes, MFCreateMediaType,
    MFCreateSourceReaderFromMediaSource, MFMediaType_Video, MFVideoFormat_RGB32,
};

use webca_core::DeviceError;

use crate::{
    camera::WindowsCamera,
    error::platform_error,
    frame::{PreviewFrame, VideoFormat, pack_bgra, pick_preview_format},
};

const PREVIEW_MAX_HEIGHT: u32 = 540;
#[allow(clippy::cast_sign_loss)]
const VIDEO_STREAM: u32 = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;

/// Reads frames from an opened camera, converted to RGB32 at a preview-friendly size.
#[derive(Debug)]
pub struct PreviewReader {
    reader: IMFSourceReader,
    width: u32,
    height: u32,
    stride: i32,
}

impl PreviewReader {
    pub fn start(camera: &WindowsCamera) -> Result<Self, DeviceError> {
        Self::create(camera).map_err(|error| platform_error(&error))
    }

    fn create(camera: &WindowsCamera) -> windows::core::Result<Self> {
        let mut attributes: Option<IMFAttributes> = None;
        // SAFETY: valid out-pointer; the interface is owned by `attributes`.
        unsafe { MFCreateAttributes(&raw mut attributes, 1)? };
        let attributes = attributes.ok_or_else(windows::core::Error::empty)?;
        // SAFETY: the key is a static GUID.
        unsafe { attributes.SetUINT32(&MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING, 1)? };
        // SAFETY: the source belongs to `camera`, which outlives the reader in every caller.
        let reader = unsafe { MFCreateSourceReaderFromMediaSource(camera.source(), &attributes)? };

        let formats = native_formats(&reader)?;
        if let Some(index) = pick_preview_format(&formats, PREVIEW_MAX_HEIGHT) {
            // SAFETY: `index` was returned by GetNativeMediaType for this stream.
            let native = unsafe { reader.GetNativeMediaType(VIDEO_STREAM, index)? };
            // SAFETY: setting a native type of the same stream.
            unsafe { reader.SetCurrentMediaType(VIDEO_STREAM, None, &native)? };
        }
        // SAFETY: plain media type construction.
        let output = unsafe { MFCreateMediaType()? };
        // SAFETY: static GUID keys and values.
        unsafe {
            output.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
            output.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32)?;
            reader.SetCurrentMediaType(VIDEO_STREAM, None, &output)?;
        }
        // SAFETY: the stream has a current type after the call above.
        let current = unsafe { reader.GetCurrentMediaType(VIDEO_STREAM)? };
        let (width, height) = split(unsafe { current.GetUINT64(&MF_MT_FRAME_SIZE)? });
        // SAFETY: reading an optional attribute; absence means a top-down image.
        let stride = unsafe { current.GetUINT32(&MF_MT_DEFAULT_STRIDE) }
            .map_or_else(|_| i32::try_from(width * 4).unwrap_or(i32::MAX), u32::cast_signed);
        Ok(Self { reader, width, height, stride })
    }

    /// Blocks until the next frame; `Ok(None)` when the camera produced no image this time.
    pub fn next_frame(&self) -> Result<Option<PreviewFrame>, DeviceError> {
        let mut flags = 0u32;
        let mut sample: Option<IMFSample> = None;
        // SAFETY: out-pointers reference locals.
        unsafe {
            self.reader.ReadSample(VIDEO_STREAM, 0, None, Some(&raw mut flags), None, Some(&raw mut sample))
        }
        .map_err(|error| platform_error(&error))?;
        #[allow(clippy::cast_sign_loss)]
        if flags & (MF_SOURCE_READERF_ERROR.0 as u32 | MF_SOURCE_READERF_ENDOFSTREAM.0 as u32) != 0 {
            return Err(DeviceError::Disconnected);
        }
        let Some(sample) = sample else { return Ok(None) };
        self.pack(&sample).map_err(|error| platform_error(&error))
    }

    fn pack(&self, sample: &IMFSample) -> windows::core::Result<Option<PreviewFrame>> {
        // SAFETY: `sample` is a live sample returned by ReadSample.
        let buffer = unsafe { sample.ConvertToContiguousBuffer()? };
        let mut data = std::ptr::null_mut();
        let mut length = 0u32;
        // SAFETY: out-pointers reference locals; the buffer stays locked until Unlock below.
        unsafe { buffer.Lock(&raw mut data, None, Some(&raw mut length))? };
        // SAFETY: Lock returned `length` readable bytes at `data`.
        let bytes = unsafe { std::slice::from_raw_parts(data, length as usize) };
        let frame = pack_bgra(bytes, self.width, self.height, self.stride);
        // SAFETY: balances the Lock above.
        unsafe { buffer.Unlock()? };
        Ok(frame)
    }
}

fn native_formats(reader: &IMFSourceReader) -> windows::core::Result<Vec<VideoFormat>> {
    let mut formats = Vec::new();
    for index in 0.. {
        // SAFETY: enumeration stops at MF_E_NO_MORE_TYPES.
        let media_type: IMFMediaType = match unsafe { reader.GetNativeMediaType(VIDEO_STREAM, index) } {
            Ok(media_type) => media_type,
            Err(error) if error.code() == MF_E_NO_MORE_TYPES => break,
            Err(error) => return Err(error),
        };
        // SAFETY: reading attributes of a native video type.
        let Ok(size) = (unsafe { media_type.GetUINT64(&MF_MT_FRAME_SIZE) }) else { continue };
        let (width, height) = split(size);
        // SAFETY: as above; frame rate may be missing on some drivers.
        let fps = unsafe { media_type.GetUINT64(&MF_MT_FRAME_RATE) }.map_or(0.0, |rate| {
            let (numerator, denominator) = split(rate);
            #[allow(clippy::cast_precision_loss)]
            if denominator == 0 { 0.0 } else { numerator as f32 / denominator as f32 }
        });
        formats.push(VideoFormat { index, width, height, fps });
    }
    Ok(formats)
}

#[allow(clippy::cast_possible_truncation)]
const fn split(packed: u64) -> (u32, u32) {
    ((packed >> 32) as u32, packed as u32)
}
