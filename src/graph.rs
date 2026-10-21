//! Minimal Microsoft Graph device-code sign-in and read-only chat access.
//!
//! # Client ID and risk
//!
//! This CLI authenticates using Microsoft's own first-party public client ID
//! (`d3590ed6-52b3-4102-aeff-aad2292ab01c`, Microsoft's "Microsoft Office"
//! application — the same fallback client ID used by several open-source
//! Microsoft Graph CLI tools, e.g. `teams-tui-go`). It is **not** the real
//! Microsoft Teams application's own client ID, but it is still a trusted
//! first-party Microsoft app.
//!
//! Using any first-party client ID (rather than an app your organization
//! registered and reviewed) means sign-in may be treated by your tenant's
//! Conditional Access / sign-in risk policies as coming from a trusted
//! Microsoft app, bypassing whatever governance your organization's own
//! app-registration process is meant to enforce. If your organization
//! requires explicit app registration for third-party tools, confirm this
//! approach with your security team.
//!
//! Override the client ID (and tenant) with `TEAMS_CLI_CLIENT_ID` /
//! `TEAMS_CLI_TENANT` once your organization provides its own registration.
//!
//! # Known failure: AADSTS65002 (consent not preauthorized)
//!
//! In at least one tenant, completing sign-in with this default client ID
//! fails at the consent step with:
//!
//! ```text
//! AADSTS65002: Consent between first party application
//! 'd3590ed6-52b3-4102-aeff-aad2292ab01c' and first party resource
//! '00000003-0000-0000-c000-000000000000' must be configured via
//! preauthorization - applications owned and operated by Microsoft must
//! get approval from the API owner before requesting tokens for that API.
//! ```
//!
//! This was reproduced with both `User.Read Chat.Read` and (as a
//! diagnostic) `User.Read Chat.ReadWrite offline_access`, so it is not
//! specific to the `Chat.Read` permission — Microsoft Graph
//! (`00000003-0000-0000-c000-000000000000`) is not preauthorized for this
//! client ID's chat scopes in that tenant at all. The device-code request
//! itself succeeds (a real code is issued); the failure only surfaces when
//! the user completes sign-in and Entra evaluates consent.
//!
//! If you hit this error, the default client ID cannot be made to work for
//! `teams chat` in your tenant; set `TEAMS_CLI_CLIENT_ID` /
//! `TEAMS_CLI_TENANT` to your own app registration (see the README) instead
//! of retrying with a different scope string.
//!
//! # Scope and token handling
//!
//! The only *functional* scope requested is the read-only `Chat.Read`
//! delegated permission. `User.Read` is also requested because Entra's
//! `/devicecode` endpoint rejects `Chat.Read` on its own with
//! `AADSTS70011: scope ... is not valid` (confirmed against the live
//! endpoint); `User.Read` is the minimal addition that is accepted and does
//! not expand access beyond the caller's own basic profile. The access
//! token is held in memory for the lifetime of the process only; it is
//! never written to disk, so every invocation requires a fresh device-code
//! sign-in.

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::env;
use std::thread;
use std::time::Duration;

/// Microsoft's first-party "Microsoft Office" public client ID, reused here as
/// a device-code fallback. See the module documentation for the trust and
/// governance implications of using a first-party client ID.
pub const DEFAULT_CLIENT_ID: &str = "d3590ed6-52b3-4102-aeff-aad2292ab01c";

/// The delegated scopes this CLI requests: read-only chat access, plus the
/// baseline `User.Read` Entra requires alongside it (see module docs).
///
/// Note: in this tenant, sign-in with this client ID fails at the consent
/// step with `AADSTS65002` regardless of which Graph chat scope is
/// requested (confirmed with both `Chat.Read` and `Chat.ReadWrite
/// offline_access`) — see the module documentation's "Known failure" section.
const SCOPES: &str = "User.Read Chat.Read";

fn client_id() -> String {
    env::var("TEAMS_CLI_CLIENT_ID").unwrap_or_else(|_| DEFAULT_CLIENT_ID.to_string())
}

fn tenant() -> String {
    env::var("TEAMS_CLI_TENANT").unwrap_or_else(|_| "common".to_string())
}

#[derive(Debug, Deserialize)]
struct DeviceCodeResponse {
    device_code: String,
    /// Human-readable sign-in instructions, shown to the user as-is.
    message: String,
    #[serde(default = "default_interval")]
    interval: u64,
    expires_in: u64,
}

fn default_interval() -> u64 {
    5
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
}

#[derive(Debug, Deserialize)]
struct TokenErrorResponse {
    error: String,
}

/// Run the OAuth2 device-code flow and return an access token held only in
/// memory for this process. Prints the Microsoft-provided sign-in
/// instructions to stdout and blocks until the user completes sign-in, the
/// code expires, or the user declines.
pub fn sign_in() -> Result<String> {
    let tenant = tenant();
    let client_id = client_id();
    let http = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .context("building HTTP client")?;

    let device_code_url =
        format!("https://login.microsoftonline.com/{tenant}/oauth2/v2.0/devicecode");
    let response = http
        .post(&device_code_url)
        .form(&[("client_id", client_id.as_str()), ("scope", SCOPES)])
        .send()
        .context("requesting device code")?;
    let status = response.status();
    let body = response
        .text()
        .context("reading device code response body")?;
    if !status.is_success() {
        bail!("device code endpoint rejected the request (HTTP {status}): {body}");
    }
    let dc: DeviceCodeResponse =
        serde_json::from_str(&body).context("parsing device code response")?;

    println!("{}", dc.message);

    let token_url = format!("https://login.microsoftonline.com/{tenant}/oauth2/v2.0/token");
    let deadline = std::time::Instant::now() + Duration::from_secs(dc.expires_in);
    let mut interval = Duration::from_secs(dc.interval.max(1));

    loop {
        if std::time::Instant::now() >= deadline {
            bail!("device code expired before sign-in completed; please try again");
        }
        thread::sleep(interval);

        let response = http
            .post(&token_url)
            .form(&[
                ("client_id", client_id.as_str()),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ("device_code", dc.device_code.as_str()),
            ])
            .send()
            .context("polling for token")?;

        if response.status().is_success() {
            let token: TokenResponse = response.json().context("parsing token response")?;
            return Ok(token.access_token);
        }

        let status = response.status();
        let error: TokenErrorResponse = response
            .json()
            .with_context(|| format!("parsing token error response (HTTP {status})"))?;
        match error.error.as_str() {
            "authorization_pending" => continue,
            "slow_down" => {
                interval += Duration::from_secs(5);
                continue;
            }
            "authorization_declined" => bail!("sign-in was declined"),
            "expired_token" => {
                bail!("device code expired before sign-in completed; please try again")
            }
            other => bail!("sign-in failed: {other}"),
        }
    }
}

#[derive(Debug, Deserialize)]
struct ChatsPage {
    value: Vec<RawChat>,
}

#[derive(Debug, Deserialize)]
struct RawChat {
    id: String,
    topic: Option<String>,
    #[serde(rename = "chatType")]
    chat_type: Option<String>,
    members: Option<Vec<RawMember>>,
}

#[derive(Debug, Deserialize)]
struct RawMember {
    #[serde(rename = "displayName")]
    display_name: Option<String>,
}

/// A Teams chat, as seen through `GET /me/chats`.
#[derive(Debug, Clone)]
pub struct Chat {
    pub id: String,
    pub display_name: String,
    pub chat_type: String,
}

/// List the signed-in user's chats. Only the first page is fetched (Graph's
/// default page size); very large chat lists are truncated rather than
/// paginated, matching this CLI's minimal initial scope.
pub fn list_chats(token: &str) -> Result<Vec<Chat>> {
    let http = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .context("building HTTP client")?;

    let response = http
        .get("https://graph.microsoft.com/v1.0/me/chats")
        .query(&[("$expand", "members"), ("$top", "50")])
        .bearer_auth(token)
        .send()
        .context("listing chats")?;
    let status = response.status();
    let body = response.text().context("reading chat list response body")?;
    if !status.is_success() {
        bail!("Graph rejected the chat list request (HTTP {status}): {body}");
    }
    let page: ChatsPage = serde_json::from_str(&body).context("parsing chat list response")?;

    Ok(page
        .value
        .into_iter()
        .map(|chat| {
            let chat_type = chat.chat_type.unwrap_or_else(|| "unknown".to_string());
            let display_name = chat.topic.unwrap_or_else(|| {
                chat.members
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(|member| member.display_name)
                    .collect::<Vec<_>>()
                    .join(", ")
            });
            Chat {
                id: chat.id,
                display_name,
                chat_type,
            }
        })
        .collect())
}

pub fn print_chats(chats: &[Chat]) {
    if chats.is_empty() {
        println!("No chats found.");
        return;
    }
    for chat in chats {
        let name = if chat.display_name.is_empty() {
            "(unnamed chat)"
        } else {
            &chat.display_name
        };
        println!("{}\t{}\t{}", chat.id, name, chat.chat_type);
    }
}

#[derive(Debug, Deserialize)]
struct MessagesPage {
    value: Vec<RawMessage>,
}

#[derive(Debug, Deserialize)]
struct RawMessage {
    #[serde(rename = "createdDateTime")]
    created_date_time: Option<String>,
    from: Option<RawFrom>,
    body: Option<RawBody>,
}

#[derive(Debug, Deserialize)]
struct RawFrom {
    user: Option<RawMember>,
}

#[derive(Debug, Deserialize)]
struct RawBody {
    #[serde(rename = "contentType")]
    content_type: Option<String>,
    content: Option<String>,
}

/// A single chat message, with HTML bodies reduced to plain text.
#[derive(Debug, Clone)]
pub struct Message {
    pub created_date_time: String,
    pub sender: String,
    pub text: String,
}

/// List the most recent messages in one chat (newest first, Graph's default
/// order), up to `top` (callers should keep this within Graph's own limits).
pub fn list_messages(token: &str, chat_id: &str, top: u32) -> Result<Vec<Message>> {
    let http = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .context("building HTTP client")?;

    let url = format!("https://graph.microsoft.com/v1.0/me/chats/{chat_id}/messages");
    let response = http
        .get(&url)
        .query(&[("$top", top.to_string())])
        .bearer_auth(token)
        .send()
        .context("listing messages")?;
    let status = response.status();
    let body = response
        .text()
        .context("reading message list response body")?;
    if !status.is_success() {
        bail!("Graph rejected the message list request (HTTP {status}): {body}");
    }
    let page: MessagesPage =
        serde_json::from_str(&body).context("parsing message list response")?;

    Ok(page
        .value
        .into_iter()
        .map(|message| {
            let sender = message
                .from
                .and_then(|from| from.user)
                .and_then(|user| user.display_name)
                .unwrap_or_else(|| "(unknown sender)".to_string());
            let text = message
                .body
                .map(|body| match body.content_type.as_deref() {
                    Some("html") => strip_html(&body.content.unwrap_or_default()),
                    _ => body.content.unwrap_or_default(),
                })
                .unwrap_or_default();
            Message {
                created_date_time: message.created_date_time.unwrap_or_default(),
                sender,
                text,
            }
        })
        .collect())
}

pub fn print_messages(messages: &[Message]) {
    if messages.is_empty() {
        println!("No messages found.");
        return;
    }
    for message in messages {
        println!(
            "[{}] {}: {}",
            message.created_date_time, message.sender, message.text
        );
    }
}

/// Reduce a small, trusted HTML fragment (a Teams message body) to plain
/// text: tags are dropped and the handful of entities Teams emits are
/// decoded. This is not a general-purpose HTML sanitizer.
fn strip_html(html: &str) -> String {
    let mut text = String::with_capacity(html.len());
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => text.push(ch),
            _ => {}
        }
    }
    text.replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_tags_and_decodes_common_entities() {
        assert_eq!(
            strip_html("<p>Hi &amp; welcome &quot;friend&quot;</p>"),
            "Hi & welcome \"friend\""
        );
    }

    #[test]
    fn default_client_id_is_a_valid_guid_shape() {
        assert_eq!(DEFAULT_CLIENT_ID.len(), 36);
        assert_eq!(DEFAULT_CLIENT_ID.matches('-').count(), 4);
    }
}
