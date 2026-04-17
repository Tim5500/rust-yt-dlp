use std::process::Stdio;   // ← 新增这一行

// 原有的其他 use 语句保持不变
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::process::Command;
use tokio::io::{AsyncBufReadExt, BufReader};
use anyhow::{Result, Context};


#[derive(Default, PartialEq, Clone, Copy)]
pub enum Quality {
    #[default]
    Best,
    BestVideo,
    Medium,
    Low,
}

#[derive(Clone)]
pub struct DownloadProgress {
    pub percentage: f32,
}

pub struct YtDlpBackend {
    bin_dir: PathBuf,
    pub yt_dlp_path: PathBuf,
    _ffmpeg_path: PathBuf,
}

impl YtDlpBackend {
    pub fn new() -> Self {
        let bin_dir = crate::config::get_bin_dir();
        let yt_dlp_path = bin_dir.join(if cfg!(windows) { "yt-dlp.exe" } else { "yt-dlp" });
        let ffmpeg_path = bin_dir.join(if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" });

        Self { bin_dir, yt_dlp_path, _ffmpeg_path: ffmpeg_path }
    }

    pub async fn ensure_binaries(&self) -> Result<()> {
        std::fs::create_dir_all(&self.bin_dir)?;

        if !self.yt_dlp_path.exists() {
            println!("首次运行，正在自动下载 yt-dlp...");
            let url = if cfg!(windows) {
                "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe"
            } else if cfg!(target_os = "macos") {
                "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_macos"
            } else {
                "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_linux"
            };

            self.download_file(url, &self.yt_dlp_path).await?;
        }
        Ok(())
    }

    async fn download_file(&self, url: &str, dest: &Path) -> Result<()> {
        let client = reqwest::Client::new();
        let resp = client.get(url).send().await?;
        let bytes = resp.bytes().await?;
        std::fs::write(dest, bytes)?;
        Ok(())
    }

    pub fn force_update_yt_dlp(&self) -> Result<()> {
        if self.yt_dlp_path.exists() {
            let _ = std::fs::remove_file(&self.yt_dlp_path);
        }
        Ok(())
    }

    pub async fn download(
    &self,
    url: &str,
    output_dir: &Path,
    quality: Quality,
    progress_callback: impl Fn(DownloadProgress) + Send + 'static,
    cancel_flag: Arc<AtomicBool>,
) -> Result<String> {
    self.ensure_binaries().await?;

    let format = match quality {
    Quality::Best => "bestvideo[ext=mp4]+bestaudio[ext=m4a]/bestvideo+bestaudio/best",  // 优先 mp4视频 + m4a音频
    Quality::BestVideo => "bestvideo[ext=mp4]/bestvideo", 
    Quality::Medium => "best[height<=720][ext=mp4]/bestvideo[height<=720]+bestaudio[ext=m4a]/best[height<=720]",
    Quality::Low => "best[height<=480][ext=mp4]/bestvideo[height<=480]+bestaudio[ext=m4a]/best[height<=480]",
};

    let output_template = output_dir.join("%(title)s.%(ext)s").to_string_lossy().to_string();

    let mut cmd = Command::new(&self.yt_dlp_path);
    cmd.current_dir(output_dir)
        .arg("-f").arg(format)
        .arg("-o").arg(output_template)
        .arg("--merge-output-format").arg("mp4")
        .arg("--no-playlist")
        .arg("--ignore-errors")
        .arg("--newline")
        .arg("--encoding").arg("utf8")          // 强制 yt-dlp 使用 UTF-8 输出
        .arg(url)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());   // stderr 也接管，防止阻塞

    let mut child = cmd.spawn().context("Failed to start yt-dlp")?;

    // ====================== 关键修复：使用字节读取 + lossy ======================
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let mut buffer = Vec::new();   // 存放原始字节

    loop {
        buffer.clear();
        let bytes_read = stdout.read_until(b'\n', &mut buffer).await?;

        if bytes_read == 0 {
            break; // EOF
        }

        if cancel_flag.load(Ordering::Relaxed) {
            let _ = child.kill().await;
            return Err(anyhow::anyhow!("Download was cancelled"));
        }

        // 将字节转为字符串，遇到无效 UTF-8 自动替换为 �（lossy）
        let line = String::from_utf8_lossy(&buffer);

        // 打印到控制台方便调试（你可以之后注释掉）
        print!("{}", line);

        if line.contains("[download]") && line.contains("%") {
            let progress = parse_progress(&line);
            progress_callback(progress);
        }
    }

    let status = child.wait().await?;
    if status.success() {
        Ok("✅ Download completed successfully!".to_string())
    } else {
        Err(anyhow::anyhow!("Download failed. Check console output for details."))
    }
}
}

fn parse_progress(line: &str) -> DownloadProgress {
    let percentage: f32 = if let Some(pos) = line.find('%') {
        line[..pos].split_whitespace().last()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.0)
    } else {
        0.0
    };

    DownloadProgress { percentage: percentage.clamp(0.0, 100.0) }
}