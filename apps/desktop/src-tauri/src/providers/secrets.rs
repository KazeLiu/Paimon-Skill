const SERVICE_NAME: &str = "io.github.kazeliu.paimon-desktop";

pub fn save_secret(secret_ref: &str, secret: &str) -> Result<(), String> {
    let entry = keyring::Entry::new(SERVICE_NAME, secret_ref)
        .map_err(|error| format!("无法打开系统凭据存储：{error}"))?;
    entry
        .set_password(secret)
        .map_err(|error| format!("无法保存 API Key：{error}"))
}

pub fn load_secret(secret_ref: &str) -> Result<String, String> {
    let entry = keyring::Entry::new(SERVICE_NAME, secret_ref)
        .map_err(|error| format!("无法打开系统凭据存储：{error}"))?;
    entry
        .get_password()
        .map_err(|error| format!("无法读取 API Key：{error}"))
}

pub fn delete_secret(secret_ref: &str) -> Result<(), String> {
    let entry = keyring::Entry::new(SERVICE_NAME, secret_ref)
        .map_err(|error| format!("无法打开系统凭据存储：{error}"))?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(format!("无法删除 API Key：{error}")),
    }
}
