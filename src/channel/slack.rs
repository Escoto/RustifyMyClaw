use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, RwLock};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message as WsMessage;
use tokio_util::sync::CancellationToken;

use crate::channel::{ChannelProvider, ChannelProviderFactory};
use crate::config::{self, ChannelConfig, OutputConfig};
use crate::security::SecurityGate;
use crate::types::{
    AllowedUser, ChannelKind, ChatId, FormattedResponse, InboundMessage, MessageContext,
    ResponseChunk, WorkspaceHandle,
};

const SLACK_API_BASE: &str = "https://slack.com/api";

// ─── Slack API response types ──────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct SlackApiOk {
    ok: bool,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SocketModeConnectResponse {
    ok: bool,
    url: Option<String>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UsersListResponse {
    ok: bool,
    members: Option<Vec<SlackUser>>,
    #[serde(rename = "response_metadata")]
    metadata: Option<UsersListMetadata>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SlackUser {
    id: String,
    name: String,
    #[serde(default)]
    deleted: bool,
}

#[derive(Debug, Deserialize)]
struct UsersListMetadata {
    next_cursor: Option<String>,
}

// ─── Socket Mode event types ──────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct SocketModeEnvelope {
    #[serde(rename = "type")]
    pub kind: String,
    pub envelope_id: Option<String>,
    pub payload: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
struct SocketModeAck {
    envelope_id: String,
}

// ─── Outbound API types ───────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
struct PostMessageRequest {
    channel: String,
    text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    thread_ts: Option<String>,
}

// ─── Provider ─────────────────────────────────────────────────────────────────

/// Slack channel provider using Socket Mode (WebSocket, no public URL required).
///
/// Requires two tokens:
/// - `bot_token` (`xoxb-…`): used for Web API calls (posting messages, resolving users).
/// - `app_token` (`xapp-…`): used to open a Socket Mode WebSocket connection for inbound events.
pub struct SlackProvider {
    bot_token: String,
    app_token: String,
    use_threads: bool,
    security_gate: SecurityGate,
    workspace: Arc<RwLock<WorkspaceHandle>>,
    output_config: Arc<OutputConfig>,
    http_client: reqwest::Client,
    /// Maps Slack channel/DM platform_id → last seen message `ts` for threading.
    thread_map: RwLock<HashMap<String, String>>,
}

impl SlackProvider {
    pub fn new(
        bot_token: String,
        app_token: String,
        use_threads: bool,
        security_gate: SecurityGate,
        workspace: Arc<RwLock<WorkspaceHandle>>,
        output_config: Arc<OutputConfig>,
        http_client: reqwest::Client,
    ) -> Self {
        Self {
            bot_token,
            app_token,
            use_threads,
            security_gate,
            workspace,
            output_config,
            http_client,
            thread_map: RwLock::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl ChannelProviderFactory for SlackProvider {
    async fn create(
        ch_config: &ChannelConfig,
        workspace: Arc<RwLock<WorkspaceHandle>>,
        global_output: &Arc<OutputConfig>,
    ) -> Result<Arc<dyn ChannelProvider>> {
        let app_token = ch_config
            .app_token
            .clone()
            .context("slack channel requires `app_token` (xapp-…)")?;
        let use_threads = ch_config.use_threads.unwrap_or(false);
        let http_client = reqwest::Client::new();

        let resolved =
            resolve_users(&ch_config.allowed_users, &ch_config.token, &http_client).await?;
        let gate = SecurityGate::new(resolved);
        let effective_output = Arc::new(config::effective_output_config(global_output, ch_config));
        Ok(Arc::new(Self::new(
            ch_config.token.clone(),
            app_token,
            use_threads,
            gate,
            workspace,
            effective_output,
            http_client,
        )))
    }
}

impl SlackProvider {
    /// Open a Socket Mode WebSocket connection and return its URL.
    async fn open_socket_connection(&self) -> Result<String> {
        let resp: SocketModeConnectResponse = self
            .http_client
            .post(format!("{SLACK_API_BASE}/apps.connections.open"))
            .bearer_auth(&self.app_token)
            .send()
            .await
            .context("failed to call apps.connections.open")?
            .json()
            .await
            .context("failed to parse apps.connections.open response")?;

        if !resp.ok {
            bail!(
                "apps.connections.open failed: {}",
                resp.error.as_deref().unwrap_or("unknown error")
            );
        }
        resp.url.context("apps.connections.open returned no URL")
    }
}

// ─── ChannelProvider impl ─────────────────────────────────────────────────────

#[async_trait]
impl ChannelProvider for SlackProvider {
    async fn start(
        &self,
        tx: mpsc::Sender<InboundMessage>,
        self_arc: Arc<dyn ChannelProvider>,
        shutdown: CancellationToken,
    ) -> Result<()> {
        loop {
            if shutdown.is_cancelled() {
                break;
            }

            let ws_url = match self.open_socket_connection().await {
                Ok(url) => url,
                Err(e) => {
                    tracing::error!(error = ?e, "slack socket connection failed — retrying in 5s");
                    tokio::select! {
                        _ = tokio::time::sleep(std::time::Duration::from_secs(5)) => {}
                        _ = shutdown.cancelled() => break,
                    }
                    continue;
                }
            };

            tracing::info!("slack socket mode connected");

            let (ws_stream, _) = match connect_async(&ws_url).await {
                Ok(pair) => pair,
                Err(e) => {
                    tracing::error!(error = ?e, "slack websocket connect failed — retrying in 5s");
                    tokio::select! {
                        _ = tokio::time::sleep(std::time::Duration::from_secs(5)) => {}
                        _ = shutdown.cancelled() => break,
                    }
                    continue;
                }
            };

            let (mut write, mut read) = ws_stream.split();

            'inner: loop {
                let msg_result = tokio::select! {
                    msg = read.next() => {
                        match msg {
                            Some(r) => r,
                            None => {
                                tracing::warn!("slack websocket stream ended — reconnecting");
                                break 'inner;
                            }
                        }
                    }
                    _ = shutdown.cancelled() => {
                        tracing::info!("slack socket mode shutting down");
                        return Ok(());
                    }
                };

                let raw = match msg_result {
                    Ok(WsMessage::Text(t)) => t,
                    Ok(WsMessage::Close(_)) => {
                        tracing::warn!("slack websocket closed by server — reconnecting");
                        break 'inner;
                    }
                    Ok(_) => continue, // ping/pong/binary — ignore
                    Err(e) => {
                        tracing::error!(error = ?e, "slack websocket read error — reconnecting");
                        break 'inner;
                    }
                };

                let envelope: SocketModeEnvelope = match serde_json::from_str(&raw) {
                    Ok(e) => e,
                    Err(e) => {
                        tracing::warn!(error = ?e, "failed to parse slack envelope — skipping");
                        continue;
                    }
                };

                // Always acknowledge envelopes that have an envelope_id.
                if let Some(ref eid) = envelope.envelope_id {
                    let ack = build_ack(eid);
                    if let Err(e) = write.send(WsMessage::Text(ack)).await {
                        tracing::error!(error = ?e, "failed to send slack ack");
                    }
                }

                match envelope.kind.as_str() {
                    "hello" => {
                        tracing::debug!("slack socket mode hello received");
                    }
                    "disconnect" => {
                        tracing::warn!("slack requested disconnect — reconnecting");
                        break 'inner;
                    }
                    "events_api" => {
                        let Some(payload) = envelope.payload else {
                            continue;
                        };
                        let event = &payload["event"];
                        if event["type"].as_str() != Some("message") {
                            continue;
                        }
                        // Skip bot messages and message_changed subtypes to avoid loops.
                        if event.get("bot_id").is_some() || event.get("subtype").is_some() {
                            continue;
                        }

                        let user_id = match event["user"].as_str() {
                            Some(u) => u.to_string(),
                            None => continue,
                        };
                        let text = match event["text"].as_str() {
                            Some(t) if !t.trim().is_empty() => t.to_string(),
                            _ => continue,
                        };
                        let channel_id = match event["channel"].as_str() {
                            Some(c) => c.to_string(),
                            None => continue,
                        };
                        let msg_ts = event["ts"].as_str().unwrap_or("").to_string();

                        if !self.security_gate.is_allowed(&user_id) {
                            tracing::trace!(user_id, "unauthorized slack message — dropped");
                            continue;
                        }

                        // Store thread_ts for possible use in send_response.
                        if self.use_threads && !msg_ts.is_empty() {
                            self.thread_map
                                .write()
                                .await
                                .insert(channel_id.clone(), msg_ts);
                        }

                        let chat_id = ChatId {
                            channel: ChannelKind::Slack,
                            platform_id: channel_id,
                        };
                        let inbound = InboundMessage {
                            chat_id,
                            user_id,
                            text,
                            context: MessageContext {
                                workspace: Arc::clone(&self.workspace),
                                provider: Arc::clone(&self_arc),
                                output_config: Arc::clone(&self.output_config),
                            },
                        };
                        if tx.send(inbound).await.is_err() {
                            tracing::error!("router channel closed — cannot forward slack message");
                        }
                    }
                    other => {
                        tracing::trace!(kind = other, "unhandled slack envelope type — skipping");
                    }
                }
            }

            // Brief pause before reconnecting to avoid tight loops on persistent failures.
            tokio::select! {
                _ = tokio::time::sleep(std::time::Duration::from_secs(1)) => {}
                _ = shutdown.cancelled() => break,
            }
        }

        tracing::info!("slack socket mode stopped");
        Ok(())
    }

    async fn send_response(&self, chat_id: &ChatId, response: FormattedResponse) -> Result<()> {
        let thread_ts = if self.use_threads {
            self.thread_map
                .read()
                .await
                .get(&chat_id.platform_id)
                .cloned()
        } else {
            None
        };

        for chunk in response.chunks {
            let text = match chunk {
                ResponseChunk::Text(t) => t,
                ResponseChunk::File { name, content } => {
                    // Slack file uploads require a separate API call (files.upload).
                    // For now, send a text notification so the user is not silently dropped.
                    tracing::warn!(
                        filename = name,
                        bytes = content.len(),
                        "slack file upload not yet implemented — sending text notification"
                    );
                    format!("[File `{name}` ({} bytes) — see CLI output]", content.len())
                }
            };

            let body = PostMessageRequest {
                channel: chat_id.platform_id.clone(),
                text,
                thread_ts: thread_ts.clone(),
            };

            let resp: SlackApiOk = self
                .http_client
                .post(format!("{SLACK_API_BASE}/chat.postMessage"))
                .bearer_auth(&self.bot_token)
                .json(&body)
                .send()
                .await
                .context("failed to call Slack chat.postMessage")?
                .json()
                .await
                .context("failed to parse Slack chat.postMessage response")?;

            if !resp.ok {
                bail!(
                    "Slack chat.postMessage failed: {}",
                    resp.error.as_deref().unwrap_or("unknown error")
                );
            }
        }
        Ok(())
    }
}

/// Resolve Slack [`AllowedUser`] entries into platform-native user ID strings
/// suitable for [`SecurityGate`] comparison.
///
/// Raw Slack user IDs (`U…` / `W…`) pass through directly. Handles (`@name`)
/// are resolved against the workspace member list via the Slack Web API.
pub async fn resolve_users(
    users: &[AllowedUser],
    bot_token: &str,
    http_client: &reqwest::Client,
) -> Result<HashSet<String>> {
    if users.is_empty() {
        bail!("slack channel must have at least one allowed_user");
    }

    let mut resolved = HashSet::new();
    let mut handles_to_lookup: Vec<String> = Vec::new();

    for user in users {
        match user {
            AllowedUser::Handle(h) if h.starts_with('U') || h.starts_with('W') => {
                resolved.insert(h.clone());
            }
            AllowedUser::Handle(h) => {
                let stripped = h.trim_start_matches('@').to_string();
                handles_to_lookup.push(stripped);
            }
            AllowedUser::NumericId(id) => {
                tracing::warn!(
                    id,
                    "numeric IDs are not valid Slack identifiers; \
                     use @handles or Slack user IDs like U01ABC123"
                );
            }
        }
    }

    if handles_to_lookup.is_empty() {
        return Ok(resolved);
    }

    let name_to_id = fetch_all_slack_users(bot_token, http_client).await?;

    for handle in &handles_to_lookup {
        match name_to_id.get(handle.as_str()) {
            Some(id) => {
                resolved.insert(id.clone());
            }
            None => {
                bail!(
                    "Slack user `@{handle}` not found in workspace — \
                     check the handle or use the raw user ID (U…)"
                );
            }
        }
    }

    Ok(resolved)
}

/// Fetch all (non-deleted) workspace users via paginated `users.list` calls.
async fn fetch_all_slack_users(
    bot_token: &str,
    http_client: &reqwest::Client,
) -> Result<HashMap<String, String>> {
    let mut name_to_id: HashMap<String, String> = HashMap::new();
    let mut cursor: Option<String> = None;

    loop {
        let mut req = http_client
            .get(format!("{SLACK_API_BASE}/users.list"))
            .bearer_auth(bot_token)
            .query(&[("limit", "200")]);

        if let Some(ref c) = cursor {
            req = req.query(&[("cursor", c.as_str())]);
        }

        let resp: UsersListResponse = req
            .send()
            .await
            .context("failed to call Slack users.list")?
            .json()
            .await
            .context("failed to parse Slack users.list response")?;

        if !resp.ok {
            bail!(
                "Slack users.list failed: {}",
                resp.error.as_deref().unwrap_or("unknown error")
            );
        }

        for member in resp.members.unwrap_or_default() {
            if !member.deleted {
                name_to_id.insert(member.name, member.id);
            }
        }

        cursor = resp
            .metadata
            .and_then(|m| m.next_cursor)
            .filter(|c| !c.is_empty());

        if cursor.is_none() {
            break;
        }
    }

    Ok(name_to_id)
}

/// Build the ack JSON for a Socket Mode envelope.
pub fn build_ack(envelope_id: &str) -> String {
    serde_json::to_string(&SocketModeAck {
        envelope_id: envelope_id.to_string(),
    })
    .unwrap_or_default()
}

#[cfg(test)]
#[path = "../tests/channel/slack_test.rs"]
mod tests;
