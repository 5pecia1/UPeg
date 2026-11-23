//! Desktop tray access to the local unread tool-result queue.

use super::boot::FrbError;

/// A retained OS notification summary.
#[derive(Debug, Clone)]
#[flutter_rust_bridge::frb(non_opaque)]
pub struct NotificationResultDto {
    pub id: String,
    pub tool_id: String,
    pub title: String,
    pub summary: String,
    pub success: bool,
    pub created_at_ms: i64,
}

/// Whether at least one tool-result notification awaits acknowledgement.
pub fn has_unread_notification_results() -> Result<bool, FrbError> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        upeg_sources::store::has_unread_notification_results().map_err(native::store_error)
    }
    #[cfg(target_arch = "wasm32")]
    {
        Ok(false)
    }
}

/// Snapshot the unread results without modifying them.
pub fn list_notification_results() -> Result<Vec<NotificationResultDto>, FrbError> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let records =
            upeg_sources::store::list_notification_results().map_err(native::store_error)?;
        Ok(records
            .into_iter()
            .map(NotificationResultDto::from)
            .collect())
    }
    #[cfg(target_arch = "wasm32")]
    {
        Ok(Vec::new())
    }
}

/// Acknowledge the ids that were displayed, preserving newer results.
pub fn acknowledge_notification_results(ids: Vec<String>) -> Result<(), FrbError> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        upeg_sources::store::acknowledge_notification_results(&ids).map_err(native::store_error)
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = ids;
        Ok(())
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use upeg_sources::store::{NotificationResultRecord, StoreError};

    use super::{FrbError, NotificationResultDto};

    pub(super) fn store_error(error: StoreError) -> FrbError {
        FrbError::Io {
            message: format!("notification results: {error}"),
        }
    }

    impl From<NotificationResultRecord> for NotificationResultDto {
        fn from(record: NotificationResultRecord) -> Self {
            Self {
                id: record.id,
                tool_id: record.tool_id,
                title: record.title,
                summary: record.summary,
                success: record.success,
                created_at_ms: record.created_at_ms,
            }
        }
    }
}
