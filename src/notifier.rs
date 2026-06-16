use std::sync::Arc;
use std::time::SystemTime;

use reqwest::Client;
use serde::Serialize;
use time::OffsetDateTime;

use crate::config::{WebhookNotificationConfig, WebhookNotificationEvent};
use crate::state::CheckStatus;

#[derive(Clone)]
pub struct WebhookNotifier {
    client: Client,
    config: Arc<WebhookNotificationConfig>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationEventType {
    StatusChange,
}

#[derive(Debug, Clone, Serialize)]
pub struct StatusChangeEvent {
    pub event: NotificationEventType,
    pub check_name: String,
    pub critical: bool,
    pub old_status: CheckStatus,
    pub new_status: CheckStatus,
    pub timestamp: String,
    pub error: Option<String>,
    pub groups: Vec<String>,
}

impl WebhookNotifier {
    pub fn new(config: WebhookNotificationConfig) -> anyhow::Result<Self> {
        let client = Client::builder().timeout(config.timeout).build()?;
        Ok(Self {
            client,
            config: Arc::new(config),
        })
    }

    pub fn notify(&self, event: StatusChangeEvent) {
        if !self.should_send(&event) {
            return;
        }

        let client = self.client.clone();
        let config = self.config.clone();
        tokio::spawn(async move {
            let response = client.post(&config.url).json(&event).send().await;
            match response {
                Ok(resp) if resp.status().is_success() => {}
                Ok(resp) => {
                    tracing::warn!(
                        url = %config.url,
                        status = %resp.status(),
                        check = %event.check_name,
                        old_status = ?event.old_status,
                        new_status = ?event.new_status,
                        "webhook notification failed"
                    );
                }
                Err(err) => {
                    tracing::warn!(
                        url = %config.url,
                        check = %event.check_name,
                        old_status = ?event.old_status,
                        new_status = ?event.new_status,
                        error = %err,
                        "webhook notification request failed"
                    );
                }
            }
        });
    }

    fn should_send(&self, event: &StatusChangeEvent) -> bool {
        self.config.on.iter().any(|kind| match kind {
            WebhookNotificationEvent::Down => event.new_status == CheckStatus::Down,
            WebhookNotificationEvent::Warn => event.new_status == CheckStatus::Warn,
            WebhookNotificationEvent::Recovery => {
                event.new_status == CheckStatus::Up && event.old_status != CheckStatus::Up
            }
        })
    }
}

pub fn now_rfc3339(st: SystemTime) -> String {
    match st.duration_since(SystemTime::UNIX_EPOCH) {
        Ok(duration) => OffsetDateTime::from_unix_timestamp(duration.as_secs() as i64)
            .ok()
            .and_then(|dt| {
                dt.format(&time::format_description::well_known::Rfc3339)
                    .ok()
            })
            .unwrap_or_else(|| "-".to_string()),
        Err(_) => "-".to_string(),
    }
}
