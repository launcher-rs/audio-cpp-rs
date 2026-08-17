//! 音频辅助：WAV 读取等便捷封装。
//!
//! # 关于非 WAV 音频格式（mp3 / flac / ogg / m4a …）
//!
//! 引擎只接受 **f32 采样**（采样率 + 声道数 + 交错采样数组），不解析容器格式；
//! [`load_wav`] 是 C 侧内置的 RIFF/WAVE 读取器，仅支持 WAV。要喂给引擎其他
//! 格式的音频，本 crate **不内置解码库**（避免替用户锁定生态 / 增加依赖），
//! 而是留出 [`WavAudio`] + `Request::asr` / `vad` / `diar` 等的 `Buffer` 路径——
//! 用户用任意 Rust 解码库（如 [symphonia]、[rodio]、[hound]、[claxon]）解码出
//! f32 采样后，直接构造 [`WavAudio`] 交给请求即可。
//!
//! 完整的可运行示例见 `load_any_audio`（在 `audio-cpp/examples/` 下），
//! 它用 symphonia 解码 mp3/flac/ogg/m4a 后经 `AudioInput::Buffer` 跑通 VAD 链路。
//!
//! [symphonia]: https://crates.io/crates/symphonia
//! [rodio]: https://crates.io/crates/rodio
//! [hound]: https://crates.io/crates/hound
//! [claxon]: https://crates.io/crates/claxon

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
