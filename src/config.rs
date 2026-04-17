use directories::ProjectDirs;
use std::path::PathBuf;

pub fn get_app_data_dir() -> PathBuf {
    let proj_dirs = ProjectDirs::from("com", "Tim5500", "rust-yt-dlp")
        .expect("无法获取应用数据目录");
    let dir = proj_dirs.data_dir().to_path_buf();
    std::fs::create_dir_all(&dir).ok();
    dir
}

pub fn get_bin_dir() -> PathBuf {
    get_app_data_dir().join("bin")
}

pub fn get_default_output_dir() -> PathBuf {
    get_app_data_dir().join("Downloads")
}