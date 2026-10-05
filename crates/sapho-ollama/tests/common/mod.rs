// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! A loopback imitation of the Ollama endpoints that records every request it receives.
#![allow(
    dead_code,
    reason = "each test binary uses a different part of the harness"
)]
use sapho_ollama::{Limits, OllamaBackend, Server, Settings};
use serde_json::{Value, json};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::{Mutex as AsyncMutex, MutexGuard, Notify},
};

/// The in-process request permit is shared by every test in a binary; running the tests
/// one after another keeps a test that holds it from starving another.
pub async fn serial() -> MutexGuard<'static, ()> {
    static LOCK: AsyncMutex<()> = AsyncMutex::const_new(());
    LOCK.lock().await
}

/// One request as the server received it.
#[derive(Clone)]
pub struct Recorded {
    pub path: String,
    pub body: Vec<u8>,
}
impl Recorded {
    pub fn json(&self) -> Value {
        serde_json::from_slice(&self.body).unwrap()
    }
}

/// What the fake sends back.
pub enum Reply {
    Status(u16, Vec<u8>),
    /// Hold the response until the test notifies, then send it.
    Held(Arc<Notify>, Box<Reply>),
    /// Wait, then send.
    After(Duration, Box<Reply>),
    /// Never answer.
    Stall,
    /// Promise more bytes than are sent, then close.
    Torn(Vec<u8>),
    Redirect,
}
impl Reply {
    pub fn ok(value: Value) -> Self {
        Self::Status(200, serde_json::to_vec(&value).unwrap())
    }
    pub fn status(code: u16, value: Value) -> Self {
        Self::Status(code, serde_json::to_vec(&value).unwrap())
    }
}

type Handler = dyn Fn(&Recorded) -> Reply + Send + Sync;

pub struct Fake {
    pub url: String,
    pub log: Arc<Mutex<Vec<Recorded>>>,
    pub max_active: Arc<AtomicUsize>,
}
impl Fake {
    pub async fn start(handler: impl Fn(&Recorded) -> Reply + Send + Sync + 'static) -> Self {
        let handler: Arc<Handler> = Arc::new(handler);
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let log = Arc::new(Mutex::new(Vec::new()));
        let active = Arc::new(AtomicUsize::new(0));
        let max_active = Arc::new(AtomicUsize::new(0));
        let (log2, active2, max2) = (log.clone(), active.clone(), max_active.clone());
        tokio::spawn(async move {
            loop {
                let Ok((mut stream, _)) = listener.accept().await else {
                    return;
                };
                let (handler, log, active, max) =
                    (handler.clone(), log2.clone(), active2.clone(), max2.clone());
                tokio::spawn(async move {
                    let mut head = Vec::new();
                    let mut byte = [0u8; 1];
                    while !head.ends_with(b"\r\n\r\n") {
                        if stream.read_exact(&mut byte).await.is_err() {
                            return;
                        }
                        head.push(byte[0]);
                    }
                    let text = String::from_utf8_lossy(&head).into_owned();
                    let path = text.split_whitespace().nth(1).unwrap_or("").to_string();
                    let length = text
                        .lines()
                        .find_map(|l| {
                            l.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .map(|v| v.trim().parse().unwrap_or(0))
                        })
                        .unwrap_or(0usize);
                    let mut body = vec![0u8; length];
                    if stream.read_exact(&mut body).await.is_err() {
                        return;
                    }
                    let request = Recorded { path, body };
                    log.lock().unwrap().push(request.clone());
                    let now = active.fetch_add(1, Ordering::SeqCst) + 1;
                    max.fetch_max(now, Ordering::SeqCst);
                    let mut reply = handler(&request);
                    loop {
                        match reply {
                            Reply::Held(gate, inner) => {
                                gate.notified().await;
                                reply = *inner;
                            }
                            Reply::After(wait, inner) => {
                                tokio::time::sleep(wait).await;
                                reply = *inner;
                            }
                            other => {
                                reply = other;
                                break;
                            }
                        }
                    }
                    let wire: Option<Vec<u8>> = match reply {
                        Reply::Status(code, body) => {
                            let mut out = format!(
                                "HTTP/1.1 {code} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                                body.len()
                            )
                            .into_bytes();
                            out.extend(body);
                            Some(out)
                        }
                        Reply::Torn(part) => {
                            let mut out = format!(
                                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                                part.len() + 1000
                            )
                            .into_bytes();
                            out.extend(part);
                            Some(out)
                        }
                        Reply::Redirect => Some(
                            b"HTTP/1.1 302 Found\r\nLocation: /elsewhere\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                                .to_vec(),
                        ),
                        Reply::Stall => {
                            tokio::time::sleep(Duration::from_secs(30)).await;
                            None
                        }
                        Reply::Held(..) | Reply::After(..) => None,
                    };
                    if let Some(wire) = wire {
                        let _ = stream.write_all(&wire).await;
                        let _ = stream.shutdown().await;
                    }
                    active.fetch_sub(1, Ordering::SeqCst);
                });
            }
        });
        Self {
            url,
            log,
            max_active,
        }
    }
    pub fn requests(&self, path: &str) -> Vec<Recorded> {
        self.log
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r.path == path)
            .cloned()
            .collect()
    }
    pub fn generates(&self) -> Vec<Recorded> {
        self.requests("/api/generate")
    }
    pub fn total(&self) -> usize {
        self.log.lock().unwrap().len()
    }
}

pub const BLOB: &str = "58574f2e94b99fb9e4391408b57e5aeaaaec10f6384e9a699fc2cb43a5c8eabf";

/// The `/api/show` answer for weights `blob`.
pub fn description(blob: &str) -> Reply {
    Reply::ok(json!({
        "modelfile": format!("# Modelfile\n\nFROM /home/x/.ollama/models/blobs/sha256-{blob}\nTEMPLATE y\n"),
        "details": {}
    }))
}
/// A complete generate answer for `model` with the given response text.
pub fn generated(model: &str, response: &str) -> Value {
    json!({
        "model": model,
        "created_at": "2026-10-05T19:58:06Z",
        "response": response,
        "thinking": "",
        "done": true,
        "done_reason": "stop",
        "context": [1, 2, 3],
        "total_duration": 5_990_808_167u64,
        "load_duration": 5_786_564_833u64,
        "prompt_eval_count": 48,
        "prompt_eval_duration": 103_966_000u64,
        "eval_count": 8,
        "eval_duration": 95_525_000u64
    })
}
/// A server that knows `model` (weights `BLOB`) and answers every generate with `answer`.
pub async fn serving(answer: Value) -> Fake {
    Fake::start(move |r| match r.path.as_str() {
        "/api/show" => description(BLOB),
        _ => Reply::ok(answer.clone()),
    })
    .await
}

pub fn limits() -> Limits {
    Limits {
        timeout: Duration::from_secs(20),
        request_bytes: 1 << 20,
        response_bytes: 1 << 20,
    }
}
pub fn settings(model: &str, think: bool, num_ctx: u64, num_predict: u64) -> Settings {
    Settings {
        model: model.into(),
        think,
        num_ctx,
        num_predict,
    }
}
pub fn backend(fake: &Fake, model: &str) -> OllamaBackend {
    OllamaBackend::new(
        Server::new(&fake.url, limits()).unwrap(),
        settings(model, false, 4096, 512),
    )
    .unwrap()
}
