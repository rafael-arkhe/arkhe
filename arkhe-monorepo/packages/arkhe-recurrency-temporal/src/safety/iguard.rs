//! MissionSafetyKernel (iGuard) — the v0.4.3 safety interlock.
//!
//! The kernel owns a shared [`crate::daemon::TemporalStateDaemon`] and
//! grades every perception by a *confidence* measure. The v0.4.3 patch fixed
//! the regression where an iGuard-forced `Coma` never recorded the failing
//! cycle, leaving the temporal EWMA at `success_rate == 1.0`. The fix is
//! structural: the kernel exposes [`MissionSafetyKernel::record_cycle`]
//! *before* [`MissionSafetyKernel::set_regime`], and
//! [`MissionSafetyKernel::enforce_coma`] always records the failing cycle
//! before forcing `Coma`.

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use arkhe_recurrency::ArousalRegime;
use axum::{routing::post, Json, Router};

use crate::daemon::{TemporalRecord, TemporalStateDaemon};
use crate::error::RecurrencyError;
use crate::perception::Perception;

/// Alert raised by the iGuard webhook protocol.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct IguardAlert {
    /// Alert severity level.
    pub level: String,
    /// Human-readable message.
    pub message: String,
    /// iGuard confidence in the underlying perception.
    pub confidence: f64,
    /// Identifier of the offending action.
    pub action_id: String,
    /// Regime the kernel is moving the substrate to.
    pub regime: ArousalRegime,
}

/// Response of an iGuard webhook endpoint.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct IguardResponse {
    /// Whether the alert was accepted.
    pub accepted: bool,
    /// Confidence at the time of the response.
    pub confidence: f64,
    /// Regime the kernel decided on.
    pub regime: ArousalRegime,
}

/// The iGuard safety interlock.
#[derive(Clone)]
pub struct MissionSafetyKernel {
    daemon: Arc<TemporalStateDaemon>,
    /// Confidence at/above which a perception is fully trusted.
    confidence_high: f64,
    /// Confidence below which a perception forces `Coma`.
    confidence_low: f64,
}

impl MissionSafetyKernel {
    /// Kernel over the shared daemon with iGuard bounds `0.8` / `0.5`.
    pub fn new(daemon: Arc<TemporalStateDaemon>) -> Self {
        Self::with_confidence(daemon, 0.8, 0.5)
    }

    /// Kernel with explicit confidence bounds.
    pub fn with_confidence(
        daemon: Arc<TemporalStateDaemon>,
        confidence_high: f64,
        confidence_low: f64,
    ) -> Self {
        Self {
            daemon,
            confidence_high,
            confidence_low,
        }
    }

    /// The shared temporal daemon.
    pub fn daemon(&self) -> &TemporalStateDaemon {
        &self.daemon
    }

    /// **v0.4.3 fix:** record the cycle before any regime change.
    pub fn record_cycle(&self, perception: &Perception) -> TemporalRecord {
        self.daemon.record_cycle(perception)
    }

    /// Switch regime on the shared daemon.
    pub fn set_regime(&self, regime: ArousalRegime) -> ArousalRegime {
        self.daemon.set_regime(regime)
    }

    /// Confidence iGuard places in a perception, `[0, 1]`.
    ///
    /// Weighted from perceived integrity and latency pressure:
    /// `0.7·integrity + 0.3·(1 − latency/2s)`.
    pub fn confidence(&self, perception: &Perception) -> f64 {
        let integrity = perception.integrity.clamp(0.0, 1.0);
        let latency_penalty = (perception.latency_ms as f64 / 2000.0).min(1.0);
        (0.7 * integrity + 0.3 * (1.0 - latency_penalty)).clamp(0.0, 1.0)
    }

}

/// Grade of a perception under the iGuard confidence bounds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IguardGrade {
    /// `confidence >= high` — fully trusted.
    Trusted,
    /// `high > confidence >= low` — partial detection.
    Partial,
    /// `confidence < low` — hostile, must force `Coma`.
    Hostile,
}

impl MissionSafetyKernel {
    /// Grade a perception under the iGuard confidence bounds.
    pub fn grade(&self, perception: &Perception) -> IguardGrade {
        let confidence = self.confidence(perception);
        if confidence >= self.confidence_high {
            IguardGrade::Trusted
        } else if confidence >= self.confidence_low {
            IguardGrade::Partial
        } else {
            IguardGrade::Hostile
        }
    }

    /// Grade a perception.
    ///
    /// * `Trusted` — fully trusted, current regime kept;
    /// * `Partial` — partial detection, `Alert` kept;
    /// * `Hostile` — record the cycle, force `Coma`.
    pub fn assess(&self, perception: &Perception) -> IguardResponse {
        let confidence = self.confidence(perception);
        match self.grade(perception) {
            IguardGrade::Hostile => {
                self.enforce_coma(perception);
                IguardResponse {
                    accepted: true,
                    confidence,
                    regime: ArousalRegime::Coma,
                }
            }
            _ => IguardResponse {
                accepted: true,
                confidence,
                regime: self.daemon.regime(),
            },
        }
    }

    /// Record the failing cycle *then* force `Coma` — the exact v0.4.3
    /// sequence. Returns the recorded cycle.
    pub fn enforce_coma(&self, perception: &Perception) -> TemporalRecord {
        let record = self.daemon.record_cycle(perception);
        self.daemon.set_regime(ArousalRegime::Coma);
        record
    }

    /// Deliver an alert to an iGuard webhook endpoint over HTTP.
    pub async fn notify_webhook(
        &self,
        url: &str,
        alert: IguardAlert,
    ) -> Result<IguardResponse, RecurrencyError> {
        let client = reqwest::Client::new();
        let response = client
            .post(url)
            .json(&alert)
            .send()
            .await
            .map_err(|e| RecurrencyError::Transport(e.to_string()))?;
        response
            .json::<IguardResponse>()
            .await
            .map_err(|e| RecurrencyError::Transport(e.to_string()))
    }
}

/// Axum-based iGuard webhook endpoint.
///
/// Serves `POST /iguard`, storing the most recent alert and answering with an
/// [`IguardResponse`]. The listener itself never changes the regime — the
/// decision stays in the kernel.
pub struct IguardWebhookListener {
    addr: SocketAddr,
    last: Arc<Mutex<Option<IguardAlert>>>,
    handle: tokio::task::JoinHandle<()>,
}

impl IguardWebhookListener {
    /// Bind and spawn the endpoint on the given address.
    pub async fn bind(addr: SocketAddr) -> Result<Self, RecurrencyError> {
        let last: Arc<Mutex<Option<IguardAlert>>> = Arc::new(Mutex::new(None));
        let state = last.clone();
        let app = Router::new().route(
            "/iguard",
            post(move |Json(alert): Json<IguardAlert>| {
                let state = state.clone();
                async move {
                    *state.lock().expect("iguard mutex") = Some(alert);
                    Json(IguardResponse {
                        accepted: true,
                        confidence: 1.0,
                        regime: ArousalRegime::Coma,
                    })
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind(addr).await?;
        let addr = listener.local_addr()?;
        let handle = tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        Ok(Self {
            addr,
            last,
            handle,
        })
    }

    /// Address the endpoint is bound to.
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// The most recently received alert, if any.
    pub fn last_alert(&self) -> Option<IguardAlert> {
        self.last.lock().expect("iguard mutex").clone()
    }
}

impl Drop for IguardWebhookListener {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::perception::ActionId;

    fn perception(integrity: f64, latency_ms: u64) -> Perception {
        Perception {
            action_id: ActionId("probe".into()),
            delta: vec![0.5; 8],
            latency_ms,
            integrity,
        }
    }

    #[test]
    fn healthy_perception_is_fully_trusted() {
        let d = Arc::new(TemporalStateDaemon::new(ArousalRegime::Alert));
        let k = MissionSafetyKernel::new(d);
        let r = k.assess(&perception(1.0, 120));
        assert!(r.confidence >= k.confidence_high);
        assert_eq!(r.regime, ArousalRegime::Alert);
        assert_eq!(k.daemon().regime(), ArousalRegime::Alert);
    }

    #[test]
    fn partial_detection_keeps_alert() {
        let d = Arc::new(TemporalStateDaemon::new(ArousalRegime::Alert));
        let k = MissionSafetyKernel::new(d);
        // confidence ≈ 0.775 → in the (0.5, 0.8) band.
        let r = k.assess(&perception(0.7, 100));
        assert!(r.confidence < k.confidence_high);
        assert!(r.confidence >= k.confidence_low);
        assert_eq!(r.regime, ArousalRegime::Alert);
        assert_eq!(k.daemon().regime(), ArousalRegime::Alert);
    }

    #[test]
    fn hostile_perception_forces_coma_and_records_the_cycle() {
        let d = Arc::new(TemporalStateDaemon::new(ArousalRegime::Alert));
        let k = MissionSafetyKernel::new(d);
        let r = k.assess(&perception(0.0, 900));
        assert!(r.confidence < k.confidence_low);
        assert_eq!(r.regime, ArousalRegime::Coma);
        assert_eq!(k.daemon().regime(), ArousalRegime::Coma);
        let integrity = k.daemon().current_integrity();
        assert!(integrity.success_rate < 1.0, "v0.4.3: cycle must be recorded");
        assert!(integrity.open_loops >= 1);
    }

    #[tokio::test]
    async fn webhook_receives_the_alert() {
        let listener = IguardWebhookListener::bind("127.0.0.1:0".parse().unwrap())
            .await
            .expect("bind");
        let d = Arc::new(TemporalStateDaemon::new(ArousalRegime::Alert));
        let k = MissionSafetyKernel::new(d);
        let alert = IguardAlert {
            level: "IMMEDIATE".into(),
            message: "hostile perception".into(),
            confidence: 0.1,
            action_id: "probe".into(),
            regime: ArousalRegime::Coma,
        };
        let url = format!("http://{}/iguard", listener.addr());
        let resp = k.notify_webhook(&url, alert.clone()).await.expect("notify");
        assert!(resp.accepted);
        assert_eq!(
            listener.last_alert().expect("received").action_id,
            "probe"
        );
    }
}
