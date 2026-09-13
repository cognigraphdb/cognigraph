//! Executable.

use super::*;

pub(crate) async fn current_executable_digest() -> Result<String, CogniGraphError> {
    let path = std::env::current_exe().map_err(|error| {
        CogniGraphError::BackendError(format!("cannot resolve running executable: {error}"))
    })?;
    let metadata = std::fs::symlink_metadata(&path).map_err(|error| {
        CogniGraphError::BackendError(format!("cannot inspect running executable: {error}"))
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(validation(
            "running executable path must be a regular non-symlink file",
        ));
    }
    let mut file = tokio::fs::File::open(&path).await.map_err(|error| {
        CogniGraphError::BackendError(format!("cannot open running executable: {error}"))
    })?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; HASH_BUFFER_BYTES];
    loop {
        let read = file.read(&mut buffer).await.map_err(|error| {
            CogniGraphError::BackendError(format!("cannot hash running executable: {error}"))
        })?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let mut hex = String::with_capacity(64);
    for byte in hasher.finalize() {
        let _ = write!(hex, "{byte:02x}");
    }
    Ok(format!("sha256:{hex}"))
}
