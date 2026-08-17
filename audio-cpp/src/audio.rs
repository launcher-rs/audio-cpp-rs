//! 音频辅助：WAV 读取等便捷封装。

use std::ffi::c_char;
use std::os::raw::c_int;
use std::ptr;

use audio_cpp_sys::*;

use crate::error::Error;
use crate::ffi;

/// 从 WAV 文件读取的音频数据。
#[derive(Debug, Clone)]
pub struct WavAudio {
    /// 采样率（Hz）。
    pub sample_rate: i32,
    /// 声道数。
    pub channels: i32,
    /// 采样数据（交错存放，值域 -1..1）。
    pub samples: Vec<f32>,
}

/// 将 RIFF/WAVE 文件读取为 `float` 采样。
///
/// 底层调用 `audiocpp_audio_load_wav`；返回的缓冲由本函数负责用
/// `audiocpp_audio_free` 释放（拷贝进 Rust `Vec` 之后）。
///
/// # Errors
///
/// 路径含 NUL / 非 UTF-8，或文件无法解析为 WAV 时返回对应 [`Error`] 变体。
pub fn load_wav(path: &str) -> Result<WavAudio, Error> {
    let path_c = ffi::cstring(path)?;
    let mut sample_rate: c_int = 0;
    let mut channels: c_int = 0;
    let mut count: usize = 0;
    let mut samples: *mut f32 = ptr::null_mut();
    let rc = unsafe {
        audiocpp_audio_load_wav(
            path_c.as_ptr() as *const c_char,
            &mut sample_rate,
            &mut channels,
            &mut count,
            &mut samples,
        )
    };
    if rc != 0 {
        return Err(Error::Ffi(ffi::last_error()));
    }
    if samples.is_null() {
        return Err(Error::Ffi("WAV 读取返回空缓冲".to_string()));
    }
    let n = count;
    let data = unsafe { std::slice::from_raw_parts(samples, n) }.to_vec();
    unsafe { audiocpp_audio_free(samples) };
    Ok(WavAudio {
        sample_rate,
        channels,
        samples: data,
    })
}
