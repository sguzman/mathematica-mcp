use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{
  AtomicI64,
  AtomicUsize,
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
struct ActiveEvaluationGuard {
  in_flight: Arc<AtomicUsize>,
  last_accessed: Arc<AtomicI64>
}

impl Drop for ActiveEvaluationGuard {
  fn drop(&mut self) {
    self.last_accessed.store(Utc::now().timestamp(), Ordering::SeqCst);
    self.in_flight.fetch_sub(1, Ordering::SeqCst);
  }
}

pub struct SessionHandle {
  pub created_at:    DateTime<Utc>,
  pub last_accessed: Arc<AtomicI64>,
  pub in_flight:     Arc<AtomicUsize>,
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
    let signer = SessionIdSigner::new();
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
            if now - last > 1800 && handle.in_flight.load(Ordering::SeqCst) == 0 {
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
                && h.in_flight.load(Ordering::SeqCst) == 0
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

    let (tx, rx) = flume::bounded::<SessionRequest>(32);
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
            // The requester may have disconnected while this command was queued.
            // Never execute cancelled commands that may have side effects.
            if reply.is_closed() {
              tracing::debug!("discarding cancelled kernel evaluation");
              continue;
            }
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
          in_flight: Arc::new(AtomicUsize::new(0)),
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

    let (handle, _guard) = {
      let map = self.inner.lock().await;
      let h =
        map.get(session_id).ok_or_else(|| anyhow!("session not found or closed: {session_id}"))?;
      h.last_accessed.store(Utc::now().timestamp(), Ordering::SeqCst);
      // Each WSTP kernel is serial. Do not accept concurrent requests into
      // a queue where their deadlines would expire before execution begins.
      h.in_flight.compare_exchange(0, 1, Ordering::SeqCst, Ordering::SeqCst)
        .map_err(|_| anyhow!("session busy: wait for the current evaluation to finish"))?;
      (h.tx.clone(), ActiveEvaluationGuard {
        in_flight: h.in_flight.clone(),
        last_accessed: h.last_accessed.clone()
      })
    };

    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    if let Err(error) = handle.try_send(SessionRequest::Eval {
      code: code.to_string(),
      timeout,
      reply: reply_tx
    }) {
      return Err(anyhow!("session request queue unavailable or full: {error:?}"));
    }

    // Allow the kernel timer time to return structured results over WSTP.
    let response = tokio::time::timeout(timeout + Duration::from_secs(10), reply_rx).await;
    match response {
      | Ok(Ok(res)) => res,
      | Ok(Err(e)) => Err(anyhow!("eval reply canceled: {e:?}")),
      | Err(_) => {
        // A native WSTP operation or AbortProtect may ignore TimeConstrained.
        // Retire this session to prevent subsequent evaluations piling up.
        tracing::warn!(session_id = %session_id, "unresponsive kernel; retiring session");
        if let Some(handle) = self.inner.lock().await.remove(session_id) {
          let (reply, _response) = tokio::sync::oneshot::channel();
          let _ = handle.tx.try_send(SessionRequest::Shutdown { reply });
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
      let busy = map.get(session_id)
        .ok_or_else(|| anyhow!("session not found or already closed: {session_id}"))?
        .in_flight.load(Ordering::SeqCst);
      if busy > 0 {
        return Err(anyhow!("session has {busy} active evaluations; close after they finish"));
      }
      map.remove(session_id).expect("checked session exists")
    };

    Self::shutdown_handle(handle).await?;

    Ok(())
  }

  async fn shutdown_handle(handle: SessionHandle) -> anyhow::Result<()> {
    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    handle
      .tx
      .try_send(SessionRequest::Shutdown { reply: reply_tx })
      .map_err(|e| anyhow!("worker busy or shutdown queue full: {e:?}"))?;

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

#[cfg(test)]
mod lifecycle_tests {
  use super::*;

  #[test]
  fn evaluation_guard_releases_active_counter() {
    let in_flight = Arc::new(AtomicUsize::new(1));
    let last_accessed = Arc::new(AtomicI64::new(0));
    {
      let _guard = ActiveEvaluationGuard {
        in_flight: in_flight.clone(),
        last_accessed: last_accessed.clone()
      };
      assert_eq!(in_flight.load(Ordering::SeqCst), 1);
    }
    assert_eq!(in_flight.load(Ordering::SeqCst), 0);
    assert!(last_accessed.load(Ordering::SeqCst) > 0);
  }
}
