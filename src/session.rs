use crate::session_id::SessionIdSigner;
use crate::wolfram;

use anyhow::anyhow;
use chrono::{DateTime, Utc};
use flume::Sender;
use serde::Serialize;
use std::collections::HashMap;
use std::thread;

#[derive(Debug)]
pub enum SessionRequest {
    Eval {
        code: String,
        reply: tokio::sync::oneshot::Sender<anyhow::Result<String>>,
    },
    Shutdown {
        reply: tokio::sync::oneshot::Sender<()>,
    },
}

#[derive(Debug)]
pub struct SessionHandle {
    pub created_at: DateTime<Utc>,
    pub tx: Sender<SessionRequest>,
    join: thread::JoinHandle<()>,
}

#[derive(Clone)]
pub struct SessionManager {
    signer: SessionIdSigner,
    inner: std::sync::Arc<tokio::sync::Mutex<HashMap<String, SessionHandle>>>,
}

#[derive(Debug, Serialize)]
pub struct SessionInfo {
    pub session_id: String,
    pub created_at_utc: String,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            signer: SessionIdSigner::from_env(),
            inner: std::sync::Arc::new(tokio::sync::Mutex::new(HashMap::new())),
        }
    }

    pub fn verify(&self, session_id: &str) -> bool {
        self.signer.verify(session_id)
    }

    pub async fn create_session(&self) -> anyhow::Result<String> {
        let session_id = self.signer.generate();
        let kernel_cmd = wolfram::resolve_kernel_cmd()?;

        let (tx, rx) = flume::unbounded::<SessionRequest>();
        let (ready_tx, ready_rx) = std::sync::mpsc::channel::<anyhow::Result<()>>();

        let join = thread::spawn(move || {
            // Create link *inside* the thread.
            let mut link = match wolfram::launch_link(&kernel_cmd) {
                Ok(l) => {
                    let _ = ready_tx.send(Ok(()));
                    l
                }
                Err(e) => {
                    let _ = ready_tx.send(Err(e));
                    return;
                }
            };

            tracing::info!("session thread started");

            while let Ok(req) = rx.recv() {
                match req {
                    SessionRequest::Eval { code, reply } => {
                        let res = wolfram::eval_to_string(&mut link, &code);
                        let _ = reply.send(res);
                    }
                    SessionRequest::Shutdown { reply } => {
                        tracing::info!("session thread shutting down");
                        let _ = reply.send(());
                        break;
                    }
                }
            }

            // When thread exits, Link drops; kernel should terminate.
            tracing::info!("session thread exited");
        });

        // Wait for startup
        match ready_rx.recv() {
            Ok(Ok(())) => {
                let mut map = self.inner.lock().await;
                map.insert(
                    session_id.clone(),
                    SessionHandle {
                        created_at: Utc::now(),
                        tx,
                        join,
                    },
                );
                Ok(session_id)
            }
            Ok(Err(e)) => Err(e),
            Err(e) => Err(anyhow!("session startup channel failed: {e:?}")),
        }
    }

    pub async fn eval(&self, session_id: &str, code: &str) -> anyhow::Result<String> {
        let handle = {
            let map = self.inner.lock().await;
            map.get(session_id)
                .ok_or_else(|| anyhow!("session not found or closed: {session_id}"))?
                .tx
                .clone()
        };

        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        handle
            .send(SessionRequest::Eval {
                code: code.to_string(),
                reply: reply_tx,
            })
            .map_err(|e| anyhow!("failed to send eval request: {e:?}"))?;

        reply_rx
            .await
            .map_err(|e| anyhow!("eval reply canceled: {e:?}"))?
    }

    pub async fn close_session(&self, session_id: &str) -> anyhow::Result<()> {
        let handle = {
            let mut map = self.inner.lock().await;
            map.remove(session_id)
                .ok_or_else(|| anyhow!("session not found or already closed: {session_id}"))?
        };

        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        handle
            .tx
            .send(SessionRequest::Shutdown { reply: reply_tx })
            .map_err(|e| anyhow!("failed to send shutdown: {e:?}"))?;

        let _ = reply_rx.await;

        // Joining is blocking, so do it off-thread.
        tokio::task::spawn_blocking(move || {
            let _ = handle.join.join();
        })
        .await
        .map_err(|e| anyhow!("failed to join session thread: {e:?}"))?;

        Ok(())
    }

    pub async fn list_sessions(&self) -> Vec<SessionInfo> {
        let map = self.inner.lock().await;
        map.iter()
            .map(|(id, h)| SessionInfo {
                session_id: id.clone(),
                created_at_utc: h.created_at.to_rfc3339(),
            })
            .collect()
    }
}
