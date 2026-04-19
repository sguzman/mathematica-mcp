use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{
  AtomicI64,
  Ordering
};
use std::thread;
use std::time::Duration;

use anyhow::anyhow;
use chrono::{
  DateTime,
  Utc
};
use flume::Sender;
use schemars::JsonSchema;
use serde::Serialize;
use tokio::sync::Mutex;

use crate::session_id::SessionIdSigner;
use crate::wolfram::{
  self,
  EvalResult
};

#[derive(Debug)]
pub enum SessionRequest {
  Eval {
    code:  String,
    reply: tokio::sync::oneshot::Sender<anyhow::Result<EvalResult>>
  },
  Shutdown {
    reply: tokio::sync::oneshot::Sender<()>
  }
}

#[derive(Debug)]
pub struct SessionHandle {
  pub created_at:    DateTime<Utc>,
  pub last_accessed: Arc<AtomicI64>,
  pub tx:            Sender<SessionRequest>,
  join:              thread::JoinHandle<()>
}

#[derive(Clone)]
pub struct SessionManager {
  signer: SessionIdSigner,
  inner:  Arc<Mutex<HashMap<String, SessionHandle>>>
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct SessionInfo {
  pub session_id:     String,
  pub created_at_utc: String,
  pub idle_seconds:   i64
}

impl SessionManager {
  pub fn new() -> Self {
    let signer = SessionIdSigner::from_env();
    let inner = Arc::new(Mutex::new(HashMap::new()));
    let manager = Self {
      signer,
      inner
    };

    // Background task for idle cleanup
    let cleanup_inner = manager.inner.clone();
    tokio::spawn(async move {
      let mut interval = tokio::time::interval(Duration::from_secs(60));
      loop {
        interval.tick().await;
        let now = Utc::now().timestamp();
        let mut to_remove = Vec::new();

        {
          let map = cleanup_inner.lock().await;
          for (id, handle) in map.iter() {
            let last = handle.last_accessed.load(Ordering::SeqCst);
            if now - last > 1800 {
              // 30 minutes
              to_remove.push(id.clone());
            }
          }
        }

        for id in to_remove {
          tracing::info!(session_id = %id, "closing idle session");
          // We don't want to hold the lock while joining threads,
          // but close_session handles its own locking.
          // Wait, we need a way to close without double locking or deadlocking.
          // Let's just remove and shutdown here.
          let handle = {
            let mut map = cleanup_inner.lock().await;
            map.remove(&id)
          };

          if let Some(h) = handle {
            let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
            if h
              .tx
              .send(SessionRequest::Shutdown {
                reply: reply_tx
              })
              .is_ok()
            {
              let _ = reply_rx.await;
            }
            let _ = tokio::task::spawn_blocking(move || h.join.join()).await;
          }
        }
      }
    });

    manager
  }

  pub fn verify(
    &self,
    session_id: &str
  ) -> bool {
    self.signer.verify(session_id)
  }

  pub async fn create_session(&self) -> anyhow::Result<String> {
    let session_id = self.signer.generate();
    let kernel_cmd = wolfram::resolve_kernel_cmd()?;

    let (tx, rx) = flume::unbounded::<SessionRequest>();
    let (ready_tx, ready_rx) = std::sync::mpsc::channel::<anyhow::Result<()>>();

    let join = thread::spawn(move || {
      let mut kernel = match wolfram::launch_link(&kernel_cmd) {
        | Ok(k) => {
          let _ = ready_tx.send(Ok(()));
          k
        }
        | Err(e) => {
          let _ = ready_tx.send(Err(e));
          return;
        }
      };
      let link = kernel.link();

      tracing::info!("session thread started");

      while let Ok(req) = rx.recv() {
        match req {
          | SessionRequest::Eval {
            code,
            reply
          } => {
            let res = wolfram::evaluate(link, &code);
            let _ = reply.send(res);
          }
          | SessionRequest::Shutdown {
            reply
          } => {
            tracing::info!("session thread shutting down");
            let _ = reply.send(());
            break;
          }
        }
      }

      tracing::info!("session thread exited");
    });

    match ready_rx.recv() {
      | Ok(Ok(())) => {
        let mut map = self.inner.lock().await;
        map.insert(session_id.clone(), SessionHandle {
          created_at: Utc::now(),
          last_accessed: Arc::new(AtomicI64::new(Utc::now().timestamp())),
          tx,
          join
        });
        Ok(session_id)
      }
      | Ok(Err(e)) => Err(e),
      | Err(e) => Err(anyhow!("session startup channel failed: {e:?}"))
    }
  }

  pub async fn eval(
    &self,
    session_id: &str,
    code: &str,
    timeout: Duration
  ) -> anyhow::Result<EvalResult> {
    let handle = {
      let map = self.inner.lock().await;
      let h =
        map.get(session_id).ok_or_else(|| anyhow!("session not found or closed: {session_id}"))?;
      h.last_accessed.store(Utc::now().timestamp(), Ordering::SeqCst);
      h.tx.clone()
    };

    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    handle
      .send(SessionRequest::Eval {
        code:  code.to_string(),
        reply: reply_tx
      })
      .map_err(|e| anyhow!("failed to send eval request: {e:?}"))?;

    match tokio::time::timeout(timeout, reply_rx).await {
      | Ok(Ok(res)) => res,
      | Ok(Err(e)) => Err(anyhow!("eval reply canceled: {e:?}")),
      | Err(_) => Err(anyhow!("evaluation timed out after {timeout:?}"))
    }
  }

  pub async fn close_session(
    &self,
    session_id: &str
  ) -> anyhow::Result<()> {
    let handle = {
      let mut map = self.inner.lock().await;
      map
        .remove(session_id)
        .ok_or_else(|| anyhow!("session not found or already closed: {session_id}"))?
    };

    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    handle
      .tx
      .send(SessionRequest::Shutdown {
        reply: reply_tx
      })
      .map_err(|e| anyhow!("failed to send shutdown: {e:?}"))?;

    let _ = reply_rx.await;

    tokio::task::spawn_blocking(move || {
      let _ = handle.join.join();
    })
    .await
    .map_err(|e| anyhow!("failed to join session thread: {e:?}"))?;

    Ok(())
  }

  pub async fn list_sessions(&self) -> Vec<SessionInfo> {
    let now = Utc::now().timestamp();
    let map = self.inner.lock().await;
    map
      .iter()
      .map(|(id, h)| {
        let last = h.last_accessed.load(Ordering::SeqCst);
        SessionInfo {
          session_id:     id.clone(),
          created_at_utc: h.created_at.to_rfc3339(),
          idle_seconds:   now - last
        }
      })
      .collect()
  }
}
