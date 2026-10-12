use serde::{Serialize, Serializer};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SiftError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Path not found: {0}")]
    PathNotFound(String),

    #[error("Path is not a valid directory: {0}")]
    NotADirectory(String),

    #[error("Invalid root folder: {0}")]
    InvalidRoot(String),

    #[error("Excluded path: {0}")]
    ExcludedPath(String),

    #[error("Resource limit exceeded: {0}")]
    ResourceLimitExceeded(String),

    #[error("Operation cancelled: {0}")]
    Cancelled(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Index error: {0}")]
    Index(String),

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

// Ensure SiftError serializes to typed JSON object for Tauri IPC boundaries
impl Serialize for SiftError {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("AppError", 3)?;
        let (code, message) = match self {
            SiftError::Database(e) => ("DATABASE_ERROR", e.to_string()),
            SiftError::Io(e) => ("IO_ERROR", e.to_string()),
            SiftError::Serialization(e) => ("SERIALIZATION_ERROR", e.to_string()),
            SiftError::PathNotFound(p) => ("PATH_NOT_FOUND", format!("Path does not exist: {}", p)),
            SiftError::NotADirectory(p) => {
                ("NOT_A_DIRECTORY", format!("Path is not a directory: {}", p))
            }
            SiftError::InvalidRoot(msg) => ("INVALID_ROOT", msg.clone()),
            SiftError::ExcludedPath(msg) => ("EXCLUDED_PATH", msg.clone()),
            SiftError::ResourceLimitExceeded(msg) => ("RESOURCE_LIMIT", msg.clone()),
            SiftError::Cancelled(msg) => ("CANCELLED", msg.clone()),
            SiftError::Config(msg) => ("CONFIG_ERROR", msg.clone()),
            SiftError::Index(msg) => ("INDEX_ERROR", msg.clone()),
            SiftError::Storage(msg) => ("STORAGE_ERROR", msg.clone()),
            SiftError::Internal(msg) => ("INTERNAL_ERROR", msg.clone()),
        };
        state.serialize_field("code", code)?;
        state.serialize_field("message", &message)?;
        state.serialize_field("details", &None::<String>)?;
        state.end()
    }
}

pub type Result<T> = std::result::Result<T, SiftError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_serialization() {
        let err = SiftError::PathNotFound("/does/not/exist".to_string());
        let json_val = serde_json::to_value(&err).expect("serialize error");

        assert_eq!(json_val["code"], "PATH_NOT_FOUND");
        assert!(json_val["message"]
            .as_str()
            .unwrap()
            .contains("/does/not/exist"));
    }

    #[test]
    fn test_not_a_directory_error() {
        let err = SiftError::NotADirectory("/tmp/file.txt".to_string());
        let json_val = serde_json::to_value(&err).expect("serialize error");

        assert_eq!(json_val["code"], "NOT_A_DIRECTORY");
    }
}
