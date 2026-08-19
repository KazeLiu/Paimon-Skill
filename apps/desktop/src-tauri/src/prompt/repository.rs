use std::path::{Path, PathBuf};

use super::models::PromptSettings;

pub struct PromptRepository {
    path: PathBuf,
}

impl PromptRepository {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn load(&self) -> Result<PromptSettings, String> {
        if !self.path.exists() {
            return Ok(PromptSettings::default());
        }

        let bytes =
            std::fs::read(&self.path).map_err(|error| format!("无法读取提示词设置：{error}"))?;
        serde_json::from_slice(&bytes).map_err(|error| format!("提示词设置格式无效：{error}"))
    }

    pub fn save(&self, settings: &PromptSettings) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("无法创建提示词设置目录：{error}"))?;
        }

        let temporary = self.path.with_extension("json.tmp");
        let bytes = serde_json::to_vec_pretty(settings)
            .map_err(|error| format!("无法序列化提示词设置：{error}"))?;
        std::fs::write(&temporary, bytes)
            .map_err(|error| format!("无法写入临时提示词设置：{error}"))?;

        replace_file(&temporary, &self.path)
    }

    pub fn reset(&self) -> Result<(), String> {
        match std::fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(format!("无法恢复默认提示词设置：{error}")),
        }
    }
}

#[cfg(windows)]
fn replace_file(source: &Path, destination: &Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };

    let source: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let destination: Vec<u16> = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    let result = unsafe {
        MoveFileExW(
            source.as_ptr(),
            destination.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };

    if result == 0 {
        Err(format!(
            "无法保存提示词设置：{}",
            std::io::Error::last_os_error()
        ))
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
fn replace_file(source: &Path, destination: &Path) -> Result<(), String> {
    if destination.exists() {
        std::fs::remove_file(destination)
            .map_err(|error| format!("无法替换提示词设置：{error}"))?;
    }
    std::fs::rename(source, destination).map_err(|error| format!("无法保存提示词设置：{error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saves_loads_and_resets_settings() {
        let dir = tempfile::tempdir().unwrap();
        let repository = PromptRepository::new(dir.path().join("prompt-settings.json"));
        let settings = PromptSettings {
            custom_suffix: "回答保持简洁".into(),
            ..PromptSettings::default()
        };

        repository.save(&settings).unwrap();
        assert_eq!(repository.load().unwrap(), settings);

        repository.reset().unwrap();
        assert_eq!(repository.load().unwrap(), PromptSettings::default());
    }

    #[test]
    fn rejects_corrupt_json_without_overwriting_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("prompt-settings.json");
        std::fs::write(&path, b"{not-json").unwrap();
        let repository = PromptRepository::new(&path);

        let error = repository.load().unwrap_err();

        assert!(error.contains("提示词设置"));
        assert_eq!(std::fs::read_to_string(path).unwrap(), "{not-json");
    }
}
