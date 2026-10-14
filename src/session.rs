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
    timeout: Duration,
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
          // Do not retire a session that was used since the idle scan.
          let handle = {
            let mut map = cleanup_inner.lock().await;
            let still_idle = map.get(&id).is_some_and(|h| {
              now - h.last_accessed.load(Ordering::SeqCst) > 1800
            });
            if still_idle { map.remove(&id) } else { None }
          };
          if let Some(handle) = handle {
            tracing::info!(session_id = %id, "closing idle session");
            if let Err(error) = SessionManager::shutdown_handle(handle).await {
              tracing::warn!(session_id = %id, %error, "idle shutdown incomplete");
            }
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
    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel::<anyhow::Result<()>>();

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
            timeout,
            reply
          } => {
            let res = wolfram::evaluate(link, &code, timeout);
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

    match tokio::time::timeout(Duration::from_secs(60), ready_rx).await {
      | Ok(Ok(Ok(()))) => {
        let mut map = self.inner.lock().await;
        map.insert(session_id.clone(), SessionHandle {
          created_at: Utc::now(),
          last_accessed: Arc::new(AtomicI64::new(Utc::now().timestamp())),
          tx,
          join
        });
        Ok(session_id)
      }
      | Ok(Ok(Err(e))) => Err(e),
      | Ok(Err(e)) => Err(anyhow!("session startup channel failed: {e:?}")),
      | Err(_) => Err(anyhow!("kernel startup timed out after 60 seconds"))
    }
  }

  pub async fn eval(
    &self,
    session_id: &str,
    code: &str,
    timeout: Duration
  ) -> anyhow::Result<EvalResult> {
    if timeout.is_zero() || timeout > Duration::from_secs(3600) {
      return Err(anyhow!("timeout_seconds must be between 1 and 3600"));
    }

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
        timeout,
        reply: reply_tx
      })
      .map_err(|e| anyhow!("failed to send eval request: {e:?}"))?;

    // Allow the kernel timer time to return structured results over WSTP.
    match tokio::time::timeout(timeout + Duration::from_secs(10), reply_rx).await {
      | Ok(Ok(res)) => res,
      | Ok(Err(e)) => Err(anyhow!("eval reply canceled: {e:?}")),
      | Err(_) => {
        // A native WSTP operation or AbortProtect may ignore TimeConstrained.
        // Retire this session to prevent subsequent evaluations piling up.
        tracing::warn!(session_id = %session_id, "unresponsive kernel; retiring session");
        if let Some(handle) = self.inner.lock().await.remove(session_id) {
          let (reply, _response) = tokio::sync::oneshot::channel();
          let _ = handle.tx.send(SessionRequest::Shutdown { reply });
          // A blocked native call cannot safely be joined; detach its worker.
          drop(handle);
        }
        Err(anyhow!("kernel failed to respond within timeout plus 10s; session retired"))
      }
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

    Self::shutdown_handle(handle).await?;

    Ok(())
  }

  async fn shutdown_handle(handle: SessionHandle) -> anyhow::Result<()> {
    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    handle
      .tx
      .send(SessionRequest::Shutdown { reply: reply_tx })
      .map_err(|e| anyhow!("failed to request worker shutdown: {e:?}"))?;

    // Never let a blocked kernel hang the close tool or the cleanup loop.
    tokio::time::timeout(Duration::from_secs(10), reply_rx)
      .await
      .map_err(|_| anyhow!("worker shutdown acknowledgement timed out"))?
      .map_err(|e| anyhow!("worker shutdown channel closed: {e:?}"))?;

    let joined = tokio::task::spawn_blocking(move || handle.join.join());
    tokio::time::timeout(Duration::from_secs(5), joined)
      .await
      .map_err(|_| anyhow!("worker exit timed out"))?
      .map_err(|e| anyhow!("worker join task failed: {e:?}"))?
      .map_err(|e| anyhow!("worker thread panicked: {e:?}"))?;
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
