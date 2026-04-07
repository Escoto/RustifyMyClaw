use std::collections::HashSet;
use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::Json;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

use crate::channel::{ChannelProvider, ChannelProviderFactory};
use crate::config::{self, ChannelConfig, OutputConfig};
use crate::security::SecurityGate;
use crate::types::{
    AllowedUser, ChannelKind, ChatId, FormattedResponse, InboundMessage, MessageContext,
    ResponseChunk, WorkspaceHandle,
};

const WHATSAPP_MAX_CHARS: usize = 4096;
const TRUNCATION_SUFFIX: &str = "\n[truncated]";
const DEFAULT_WEBHOOK_PORT: u16 = 8080;
const GRAPH_API_BASE: &str = "https://graph.facebook.com/v21.0";

/// WhatsApp channel provider using the Meta WhatsApp Business Cloud API.
///
/// Inbound messages arrive via a webhook receiver (axum HTTP server on `webhook_port`).
/// Outbound messages are sent via the Graph API using `reqwest`.
///
/// **Webhook setup**: Meta requires verifying the webhook URL once in the Meta developer
/// console before messages will flow. The GET `/webhook` handler answers the verification
/// challenge automatically.
pub struct WhatsAppProvider {
    api_token: String,
    phone_number_id: String,
    webhook_port: u16,
    verify_token: String,
    security_gate: SecurityGate,
    workspace: Arc<RwLock<WorkspaceHandle>>,
    output_config: Arc<OutputConfig>,
    http_client: reqwest::Client,
}

impl WhatsAppProvider {
    pub fn new(
        api_token: String,
        phone_number_id: String,
        webhook_port: Option<u16>,
        verify_token: String,
        security_gate: SecurityGate,
        workspace: Arc<RwLock<WorkspaceHandle>>,
        output_config: Arc<OutputConfig>,
    ) -> Self {
        Self {
            api_token,
            phone_number_id,
            webhook_port: webhook_port.unwrap_or(DEFAULT_WEBHOOK_PORT),
            verify_token,
            security_gate,
            workspace,
            output_config,
            http_client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl ChannelProviderFactory for WhatsAppProvider {
    async fn create(
        ch_config: &ChannelConfig,
        workspace: Arc<RwLock<WorkspaceHandle>>,
        global_output: &Arc<OutputConfig>,
    ) -> Result<Arc<dyn ChannelProvider>> {
        let phone_number_id = ch_config
            .phone_number_id
            .clone()
            .context("whatsapp channel requires `phone_number_id`")?;
        let verify_token = ch_config.verify_token.clone().unwrap_or_default();

        let tmp = Self::new(
            ch_config.token.clone(),
            phone_number_id.clone(),
            ch_config.webhook_port,
            verify_token.clone(),
            SecurityGate::new(Default::default()),
            Arc::clone(&workspace),
            Arc::clone(global_output),
        );
        let resolved = tmp.resolve_users(&ch_config.allowed_users).await?;
        let gate = SecurityGate::new(resolved);
        let effective_output = Arc::new(config::effective_output_config(global_output, ch_config));
        Ok(Arc::new(Self::new(
            ch_config.token.clone(),
            phone_number_id,
            ch_config.webhook_port,
            verify_token,
            gate,
            workspace,
            effective_output,
        )))
    }
}

// ─── Axum shared state ────────────────────────────────────────────────────────

/// All state that the axum handlers need, passed via `axum::extract::State`.
#[derive(Clone)]
struct WebhookState {
    tx: mpsc::Sender<InboundMessage>,
    gate: SecurityGate,
    workspace: Arc<RwLock<WorkspaceHandle>>,
    output_config: Arc<OutputConfig>,
    provider: Arc<dyn ChannelProvider>,
    verify_token: String,
}

// ─── Webhook payload types ────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct VerifyQuery {
    #[serde(rename = "hub.mode")]
    mode: Option<String>,
    #[serde(rename = "hub.verify_token")]
    verify_token: Option<String>,
    #[serde(rename = "hub.challenge")]
    challenge: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct WebhookPayload {
    pub entry: Vec<WebhookEntry>,
}

#[derive(Debug, Deserialize)]
pub struct WebhookEntry {
    pub changes: Vec<WebhookChange>,
}

#[derive(Debug, Deserialize)]
pub struct WebhookChange {
    pub value: WebhookValue,
}

#[derive(Debug, Deserialize)]
pub struct WebhookValue {
    #[serde(default)]
    pub messages: Vec<WhatsAppMessage>,
}

#[derive(Debug, Deserialize)]
pub struct WhatsAppMessage {
    pub from: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub text: Option<WhatsAppText>,
}

#[derive(Debug, Deserialize)]
pub struct WhatsAppText {
    pub body: String,
}

// ─── Outbound API types ───────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
struct OutboundMessage {
    messaging_product: &'static str,
    to: String,
    #[serde(rename = "type")]
    kind: &'static str,
    text: OutboundText,
}

#[derive(Debug, Serialize)]
struct OutboundText {
    body: String,
}

// ─── Axum route handlers ──────────────────────────────────────────────────────

async fn handle_verify(
    Query(params): Query<VerifyQuery>,
    State(state): State<WebhookState>,
) -> impl IntoResponse {
    if params.mode.as_deref() == Some("subscribe")
        && params.verify_token.as_deref() == Some(state.verify_token.as_str())
    {
        if let Some(challenge) = params.challenge {
            return (StatusCode::OK, challenge).into_response();
        }
    }
    StatusCode::FORBIDDEN.into_response()
}

async fn handle_inbound(
    State(state): State<WebhookState>,
    Json(payload): Json<WebhookPayload>,
) -> StatusCode {
    for entry in payload.entry {
        for change in entry.changes {
            for msg in change.value.messages {
                if msg.kind != "text" {
                    continue;
                }
                let Some(text_obj) = msg.text else {
                    continue;
                };
                let user_id = msg.from.clone();
                if !state.gate.is_allowed(&user_id) {
                    tracing::trace!(user_id, "unauthorized whatsapp message — dropped");
                    continue;
                }
                let chat_id = ChatId {
                    channel: ChannelKind::WhatsApp,
                    platform_id: user_id.clone(),
                };
                let inbound = InboundMessage {
                    chat_id,
                    user_id,
                    text: text_obj.body,
                    context: MessageContext {
                        workspace: Arc::clone(&state.workspace),
                        provider: Arc::clone(&state.provider),
                        output_config: Arc::clone(&state.output_config),
                    },
                };
                if state.tx.send(inbound).await.is_err() {
                    tracing::error!("router channel closed — cannot forward whatsapp message");
                }
            }
        }
    }
    StatusCode::OK
}

// ─── ChannelProvider impl ─────────────────────────────────────────────────────

#[async_trait]
impl ChannelProvider for WhatsAppProvider {
    async fn start(
        &self,
        tx: mpsc::Sender<InboundMessage>,
        self_arc: Arc<dyn ChannelProvider>,
        shutdown: CancellationToken,
    ) -> Result<()> {
        let state = WebhookState {
            tx,
            gate: self.security_gate.clone(),
            workspace: Arc::clone(&self.workspace),
            output_config: Arc::clone(&self.output_config),
            provider: self_arc,
            verify_token: self.verify_token.clone(),
        };

        let app = axum::Router::new()
            .route("/webhook", get(handle_verify))
            .route("/webhook", post(handle_inbound))
            .with_state(state);

        let addr = SocketAddr::from(([0, 0, 0, 0], self.webhook_port));
        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .with_context(|| format!("failed to bind webhook port {}", self.webhook_port))?;
        tracing::info!(port = self.webhook_port, "whatsapp webhook server started");

        axum::serve(listener, app)
            .with_graceful_shutdown(shutdown.cancelled_owned())
            .await
            .context("whatsapp webhook server error")?;
        Ok(())
    }

    async fn send_response(&self, chat_id: &ChatId, response: FormattedResponse) -> Result<()> {
        let url = format!("{}/{}/messages", GRAPH_API_BASE, self.phone_number_id);

        for chunk in response.chunks {
            match chunk {
                ResponseChunk::Text(text) => {
                    let safe = enforce_whatsapp_limit(&text);
                    let body = OutboundMessage {
                        messaging_product: "whatsapp",
                        to: chat_id.platform_id.clone(),
                        kind: "text",
                        text: OutboundText { body: safe },
                    };
                    self.http_client
                        .post(&url)
                        .bearer_auth(&self.api_token)
                        .json(&body)
                        .send()
                        .await
                        .context("failed to send whatsapp message")?
                        .error_for_status()
                        .context("whatsapp API returned error")?;
                }
                ResponseChunk::File { name, content } => {
                    // WhatsApp media upload is a two-step process (upload then reference).
                    // For now send a text notification instead so the user is not silently dropped.
                    tracing::warn!(
                        filename = name,
                        bytes = content.len(),
                        "whatsapp file upload not yet implemented — sending text notification"
                    );
                    let notice =
                        format!("[File `{name}` ({} bytes) — see CLI output]", content.len());
                    let safe = enforce_whatsapp_limit(&notice);
                    let body = OutboundMessage {
                        messaging_product: "whatsapp",
                        to: chat_id.platform_id.clone(),
                        kind: "text",
                        text: OutboundText { body: safe },
                    };
                    self.http_client
                        .post(&url)
                        .bearer_auth(&self.api_token)
                        .json(&body)
                        .send()
                        .await
                        .context("failed to send whatsapp file notification")?
                        .error_for_status()
                        .context("whatsapp API returned error")?;
                }
            }
        }
        Ok(())
    }

    async fn resolve_users(&self, users: &[AllowedUser]) -> Result<HashSet<String>> {
        if users.is_empty() {
            bail!("whatsapp channel must have at least one allowed_user");
        }
        let mut resolved = HashSet::new();
        for user in users {
            match user {
                AllowedUser::Handle(phone) => {
                    resolved.insert(phone.clone());
                }
                AllowedUser::NumericId(id) => {
                    tracing::warn!(
                        id,
                        "numeric IDs are not valid WhatsApp identifiers; \
                         use phone numbers like +5511999999999"
                    );
                }
            }
        }
        Ok(resolved)
    }
}

/// Truncate a message that exceeds WhatsApp's practical per-message limit.
pub fn enforce_whatsapp_limit(text: &str) -> String {
    if text.len() <= WHATSAPP_MAX_CHARS {
        return text.to_string();
    }
    let cut = WHATSAPP_MAX_CHARS - TRUNCATION_SUFFIX.len();
    let mut boundary = cut;
    while boundary > 0 && !text.is_char_boundary(boundary) {
        boundary -= 1;
    }
    format!("{}{}", &text[..boundary], TRUNCATION_SUFFIX)
}

#[cfg(test)]
#[path = "../tests/channel/whatsapp_test.rs"]
mod tests;
