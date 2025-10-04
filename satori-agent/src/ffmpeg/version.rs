use regex::Regex;
use std::process::Command;
use tracing::info;

/// Gets the version of ffmpeg based on the output of the `ffmpeg -version` command.
pub(crate) fn get_ffmpeg_version() -> String {
    let ffmpeg_process = Command::new("ffmpeg")
        .arg("-version")
        .output()
        .expect("ffmpeg process should be started");
    extract_version_from_version_output(std::str::from_utf8(&ffmpeg_process.stdout).unwrap())
}

/// Parses the output of `ffmpeg -version` to extract the version.
fn extract_version_from_version_output(text: &str) -> String {
    let version_string_regex = Regex::new(r"ffmpeg version (.*) Copyright").unwrap();
    let version_string = version_string_regex.captures(text).unwrap();
    let version_string = version_string[1].to_string();
    info!("Detected ffmpeg version: {}", version_string);
    version_string
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_version_6() {
        let text = "ffmpeg version 6.0 Copyright (c) 2000-2023 the FFmpeg developers
built with gcc 12.3.0 (GCC)
configuration: --disable-static --prefix=/nix/store/eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee-ffmpeg-6.0 --target_os=linux --arch=x86_64 --pkg-config=pkg-config --enable-gpl --enable-version3 --disable-nonfree --enable-shared --enable-pic --disable-small --enable-runtime-cpudetect --disable-gray --enable-swscale-alpha --enable-hardcoded-tables --enable-safe-bitstream-reader --enable-pthreads --disable-w32threads --disable-os2threads --enable-network --enable-pixelutils --datadir=/nix/store/hj2xwsm8jigjc3ld27gcqmdv8fghs8qg-ffmpeg-6.0-data/share/ffmpeg --enable-ffmpeg --disable-ffplay --enable-ffprobe --bindir=/nix/store/eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee-ffmpeg-6.0-bin/bin --enable-avcodec --enable-avdevice --enable-avfilter --enable-avformat --enable-avutil --enable-postproc --enable-swresample --enable-swscale --libdir=/nix/store/eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee-ffmpeg-6.0-lib/lib --incdir=/nix/store/eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee-ffmpeg-6.0-dev/include --enable-doc --enable-htmlpages --enable-manpages --mandir=/nix/store/eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee-ffmpeg-6.0-man/share/man --enable-podpages --enable-txtpages --docdir=/nix/store/eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee-ffmpeg-6.0-doc/share/doc/ffmpeg --enable-alsa --enable-bzlib --disable-libcelt --disable-cuda --disable-cuda-llvm --enable-libdav1d --disable-libfdk-aac --disable-libflite --enable-fontconfig --enable-libfreetype --disable-frei0r --disable-libfribidi --disable-libgme --enable-gnutls --disable-libgsm --disable-ladspa --enable-libmp3lame --disable-libaom --enable-libass --disable-libbluray --disable-libbs2b --disable-libdc1394 --enable-libdrm --enable-iconv --disable-libjack --disable-libmfx --disable-libmodplug --disable-libmysofa --enable-libopus --disable-librsvg --enable-libsrt --enable-libssh --disable-libtensorflow --enable-libtheora --enable-libv4l2 --enable-v4l2-m2m --enable-vaapi --enable-vdpau --enable-libvorbis --disable-libvmaf --enable-libvpx --disable-libwebp --disable-xlib --disable-libxcb --disable-libxcb-shm --disable-libxcb-xfixes --disable-libxcb-shape --disable-libxml2 --enable-lzma --enable-cuvid --enable-nvdec --enable-nvenc --disable-openal --disable-opencl --disable-libopencore-amrnb --disable-opengl --disable-libopenh264 --disable-libopenjpeg --disable-libopenmpt --enable-libpulse --disable-librav1e --enable-libsvtav1 --disable-librtmp --enable-sdl2 --enable-libsoxr --enable-libspeex --disable-libvidstab --disable-libvo-amrwbenc --enable-libx264 --enable-libx265 --disable-libxavs --enable-libxvid --disable-libzmq --enable-libzimg --enable-zlib --disable-vulkan --disable-libglslang --disable-libsmbclient --disable-debug --enable-optimizations --disable-extra-warnings --disable-stripping
libavutil      58.  2.100 / 58.  2.100
libavcodec     60.  3.100 / 60.  3.100
libavformat    60.  3.100 / 60.  3.100
libavdevice    60.  1.100 / 60.  1.100
libavfilter     9.  3.100 /  9.  3.100
libswscale      7.  1.100 /  7.  1.100
libswresample   4. 10.100 /  4. 10.100
libpostproc    57.  1.100 / 57.  1.100
";

        assert_eq!(extract_version_from_version_output(text), "6.0".to_string());
    }

    #[test]
    fn test_version_5() {
        let text = "ffmpeg version n5.1.2 Copyright (c) 2000-2022 the FFmpeg developers
built with gcc 12.2.0 (GCC)
configuration: --prefix=/usr --disable-debug --disable-static --disable-stripping --enable-amf --enable-avisynth --enable-cuda-llvm --enable-lto --enable-fontconfig --enable-gmp --enable-gnutls --enable-gpl --enable-ladspa --enable-libaom --enable-libass --enable-libbluray --enable-libbs2b --enable-libdav1d --enable-libdrm --enable-libfreetype --enable-libfribidi --enable-libgsm --enable-libiec61883 --enable-libjack --enable-libmfx --enable-libmodplug --enable-libmp3lame --enable-libopencore_amrnb --enable-libopencore_amrwb --enable-libopenjpeg --enable-libopus --enable-libpulse --enable-librav1e --enable-librsvg --enable-libsoxr --enable-libspeex --enable-libsrt --enable-libssh --enable-libsvtav1 --enable-libtheora --enable-libv4l2 --enable-libvidstab --enable-libvmaf --enable-libvorbis --enable-libvpx --enable-libwebp --enable-libx264 --enable-libx265 --enable-libxcb --enable-libxml2 --enable-libxvid --enable-libzimg --enable-nvdec --enable-nvenc --enable-opencl --enable-opengl --enable-shared --enable-version3 --enable-vulkan
libavutil      57. 28.100 / 57. 28.100
libavcodec     59. 37.100 / 59. 37.100
libavformat    59. 27.100 / 59. 27.100
libavdevice    59.  7.100 / 59.  7.100
libavfilter     8. 44.100 /  8. 44.100
libswscale      6.  7.100 /  6.  7.100
libswresample   4.  7.100 /  4.  7.100
libpostproc    56.  6.100 / 56.  6.100
";

        assert_eq!(
            extract_version_from_version_output(text),
            "n5.1.2".to_string()
        );
    }

    #[test]
    fn test_version_4() {
        let text = "ffmpeg version 4.3.5-0+deb11u1 Copyright (c) 2000-2022 the FFmpeg developers
built with gcc 10 (Debian 10.2.1-6)
configuration: --prefix=/usr --extra-version=0+deb11u1 --toolchain=hardened --libdir=/usr/lib/x86_64-linux-gnu --incdir=/usr/include/x86_64-linux-gnu --arch=amd64 --enable-gpl --disable-stripping --enable-avresample --disable-filter=resample --enable-gnutls --enable-ladspa --enable-libaom --enable-libass --enable-libbluray --enable-libbs2b --enable-libcaca --enable-libcdio --enable-libcodec2 --enable-libdav1d --enable-libflite --enable-libfontconfig --enable-libfreetype --enable-libfribidi --enable-libgme --enable-libgsm --enable-libjack --enable-libmp3lame --enable-libmysofa --enable-libopenjpeg --enable-libopenmpt --enable-libopus --enable-libpulse --enable-librabbitmq --enable-librsvg --enable-librubberband --enable-libshine --enable-libsnappy --enable-libsoxr --enable-libspeex --enable-libsrt --enable-libssh --enable-libtheora --enable-libtwolame --enable-libvidstab --enable-libvorbis --enable-libvpx --enable-libwavpack --enable-libwebp --enable-libx265 --enable-libxml2 --enable-libxvid --enable-libzmq --enable-libzvbi --enable-lv2 --enable-omx --enable-openal --enable-opencl --enable-opengl --enable-sdl2 --enable-pocketsphinx --enable-libmfx --enable-libdc1394 --enable-libdrm --enable-libiec61883 --enable-chromaprint --enable-frei0r --enable-libx264 --enable-shared
libavutil      56. 51.100 / 56. 51.100
libavcodec     58. 91.100 / 58. 91.100
libavformat    58. 45.100 / 58. 45.100
libavdevice    58. 10.100 / 58. 10.100
libavfilter     7. 85.100 /  7. 85.100
libavresample   4.  0.  0 /  4.  0.  0
libswscale      5.  7.100 /  5.  7.100
libswresample   3.  7.100 /  3.  7.100
libpostproc    55.  7.100 / 55.  7.100
";

        assert_eq!(
            extract_version_from_version_output(text),
            "4.3.5-0+deb11u1".to_string()
        );
    }
}
