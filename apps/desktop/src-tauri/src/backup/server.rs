// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! The cloud server the player set up. The page reaches it only through the
//! core, so its content security policy names no outside address.

use log::warn;
use reqwest::blocking::Client;
use reqwest::redirect::Policy;
use reqwest::{Method, Url};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::remote::{USER_AGENT, checked_base_url};
use crate::constants::CLOUD_REQUEST_TIMEOUT;
use crate::error::{Result, VaultimeError};

/// A Vaultime server, as its health check describes it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ServerInfo {
    /// The address requests go to, without a trailing slash.
    pub address: String,
    /// Whether the server takes new accounts.
    pub signups_open: bool,
}

#[derive(Deserialize)]
struct Health {
    service: String,
    #[serde(default)]
    signups_open: bool,
}

/// What came back for a request of the page.
#[derive(Debug, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Reply {
    Answer { status: u16, body: Value },
    TimedOut,
    Unreachable,
}

/// Checks that a Vaultime server answers at `address`.
pub fn check(address: &str) -> Result<ServerInfo> {
    let address = checked_base_url(&typed_address(address))?;
    let not_vaultime =
        || VaultimeError::Cloud("no Vaultime server answers at that address.".into());
    let response = client()?
        .get(format!("{address}/healthz"))
        .send()
        .map_err(|error| {
            warn!("no answer to the health check of {address}: {error}");
            if error.is_timeout() {
                VaultimeError::Cloud("the server took too long to answer. Try again later.".into())
            } else {
                VaultimeError::Offline(
                    "could not reach the server. Check the address and your connection.".into(),
                )
            }
        })?;
    if !response.status().is_success() {
        return Err(not_vaultime());
    }
    let health: Health = response.json().map_err(|_| not_vaultime())?;
    // The name the server gives in its health check.
    if health.service != "vaultime-api" {
        return Err(not_vaultime());
    }
    Ok(ServerInfo {
        address,
        signups_open: health.signups_open,
    })
}

/// Sends one JSON request of the page to the server at `address`.
pub fn request(
    address: &str,
    method: &str,
    path: &str,
    body: Option<&Value>,
    access_token: Option<&str>,
) -> Result<Reply> {
    let url = api_url(&checked_base_url(address)?, path)?;
    let method = match method {
        "GET" => Method::GET,
        "POST" => Method::POST,
        "DELETE" => Method::DELETE,
        other => {
            return Err(VaultimeError::Cloud(format!(
                "not a cloud API method: {other}"
            )));
        }
    };
    let mut request = client()?.request(method, url);
    if let Some(token) = access_token {
        request = request.bearer_auth(token);
    }
    if let Some(body) = body {
        request = request.json(body);
    }
    let text = request.send().and_then(|response| {
        let status = response.status().as_u16();
        response.text().map(|text| (status, text))
    });
    Ok(match text {
        Ok((status, text)) => Reply::Answer {
            status,
            body: serde_json::from_str(&text).unwrap_or(Value::String(text)),
        },
        Err(error) if error.is_timeout() => Reply::TimedOut,
        Err(error) => {
            warn!("no answer from the cloud server: {error}");
            Reply::Unreachable
        }
    })
}

/// The address of `path` on the server, as long as it stays in the server's
/// API once its dot segments are resolved, encoded ones included.
fn api_url(address: &str, path: &str) -> Result<Url> {
    let refused = || VaultimeError::Cloud(format!("not a cloud API path: {path}"));
    let base = Url::parse(address).map_err(|_| refused())?;
    let url = Url::parse(&format!("{address}{path}")).map_err(|_| refused())?;
    let api = format!("{}/v1/", base.path().trim_end_matches('/'));
    if !path.starts_with('/') || url.origin() != base.origin() || !url.path().starts_with(&api) {
        return Err(refused());
    }
    Ok(url)
}

/// An address typed without a scheme means https.
fn typed_address(address: &str) -> String {
    let address = address.trim();
    if address.contains("://") {
        address.to_string()
    } else {
        format!("https://{address}")
    }
}

fn client() -> Result<Client> {
    Client::builder()
        .timeout(CLOUD_REQUEST_TIMEOUT)
        // The API never redirects, and a redirect would take the body along.
        .redirect(Policy::none())
        .user_agent(USER_AGENT)
        .build()
        .map_err(|error| VaultimeError::Cloud(format!("failed to build HTTP client: {error}")))
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    use super::*;

    /// A server on this PC that answers one request with `status`, any extra
    /// `headers` and `body`, and hands back the request it got.
    fn answer_once(
        status: &str,
        headers: &'static str,
        body: &'static str,
    ) -> (String, thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = format!("http://{}", listener.local_addr().unwrap());
        let status = status.to_string();
        let served = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            let mut buffer = [0; 4096];
            while !request.windows(4).any(|end| end == b"\r\n\r\n") {
                let read = stream.read(&mut buffer).unwrap();
                request.extend_from_slice(&buffer[..read]);
            }
            write!(
                stream,
                "HTTP/1.1 {status}\r\n{headers}Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
            String::from_utf8_lossy(&request).into_owned()
        });
        (address, served)
    }

    #[test]
    fn a_vaultime_server_says_whether_it_takes_new_accounts() {
        let (address, served) = answer_once(
            "200 OK",
            "",
            r#"{"status":"ok","service":"vaultime-api","signups_open":true}"#,
        );
        let info = check(&format!("{address}/")).unwrap();
        assert_eq!(info.address, address, "the trailing slash goes");
        assert!(info.signups_open);
        assert!(served.join().unwrap().starts_with("GET /healthz "));
    }

    #[test]
    fn another_server_is_not_taken_for_one() {
        let (address, _served) = answer_once("200 OK", "", r#"{"status":"ok","service":"nginx"}"#);
        let error = check(&address).unwrap_err().to_string();
        assert!(error.contains("no Vaultime server"), "{error}");
    }

    #[test]
    fn plain_http_works_only_on_this_pc() {
        let error = check("http://nas.local:8080").unwrap_err().to_string();
        assert!(error.contains("https"), "{error}");
        assert_eq!(
            typed_address(" nas.example.com/vaultime "),
            "https://nas.example.com/vaultime"
        );
        assert_eq!(
            typed_address("http://localhost:8080"),
            "http://localhost:8080"
        );
    }

    #[test]
    fn requests_carry_the_token_and_pass_on_any_answer() {
        let (address, served) = answer_once(
            "403 Forbidden",
            "",
            r#"{"error":{"code":"forbidden","message":"this server takes no new accounts"}}"#,
        );
        let reply = request(
            &address,
            "POST",
            "/v1/auth/signup",
            Some(&serde_json::json!({ "email": "a@b.c" })),
            Some("token"),
        )
        .unwrap();
        let Reply::Answer { status, body } = reply else {
            panic!("no answer: {reply:?}");
        };
        assert_eq!(status, 403);
        assert_eq!(body["error"]["code"], "forbidden");
        let sent = served.join().unwrap();
        assert!(sent.starts_with("POST /v1/auth/signup "), "{sent}");
        let sent = sent.to_ascii_lowercase();
        assert!(sent.contains("authorization: bearer token"), "{sent}");
        assert!(sent.contains("user-agent: vaultime/"), "{sent}");
    }

    #[test]
    fn redirects_are_not_followed() {
        // Followed, the request would end at a closed port instead.
        let (address, _served) = answer_once(
            "308 Permanent Redirect",
            "Location: http://127.0.0.1:9/v1/auth/login\r\n",
            "",
        );
        let reply = request(
            &address,
            "POST",
            "/v1/auth/login",
            Some(&serde_json::json!({})),
            None,
        )
        .unwrap();
        assert!(
            matches!(reply, Reply::Answer { status: 308, .. }),
            "{reply:?}"
        );
    }

    #[test]
    fn only_api_paths_and_methods_go_out() {
        for path in [
            "/healthz",
            "/v1/../admin",
            "/v1/%2e%2e/%2E%2e/admin",
            "https://elsewhere/v1/x",
        ] {
            assert!(
                request("https://example.com", "GET", path, None, None).is_err(),
                "{path}"
            );
        }
        let below = "https://nas.example.com/vaultime";
        assert!(request(below, "GET", "/v1/%2e%2e/%2e%2e/other-app", None, None).is_err());
        assert_eq!(
            api_url(below, "/v1/backups").unwrap().as_str(),
            "https://nas.example.com/vaultime/v1/backups"
        );
        assert!(request("https://example.com", "PATCH", "/v1/backups", None, None).is_err());
    }

    #[test]
    fn a_server_that_hangs_up_is_unreachable() {
        // Hanging up at once, as a closed port on Windows takes seconds to refuse.
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = format!("http://{}", listener.local_addr().unwrap());
        let served = thread::spawn(move || drop(listener.accept().unwrap()));
        let reply = request(&address, "GET", "/v1/storage", None, None).unwrap();
        served.join().unwrap();
        assert_eq!(reply, Reply::Unreachable);
    }
}
