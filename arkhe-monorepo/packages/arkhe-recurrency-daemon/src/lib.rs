//! gRPC service implementation of the ARKHE Recurrency engine
//! (`491-AGI-CORTEX`).
//!
//! This crate is the **transport** half of the recurrency substrate: it wraps
//! the pure, deterministic core (`arkhe-recurrency`) behind a tonic service so
//! that `lobo-frontal` (TS client) and `safe-core` (policy) can drive the
//! engine over the wire without embedding the substrate in either process.
//!
//! Design invariant: the core crate stays tokio/async-free. All asynchronous
//! plumbing lives here. The service is stateless across calls except for the
//! single shared [`RecurrencyEngine`] guarded by a mutex — one substrate per
//! daemon process.

use std::sync::{Arc, Mutex};

use arkhe_recurrency::{ArousalRegime, RecurrencyEngine, RecurrencyTicket, StimulusId};
use nalgebra::DVector;
use tonic::{Request, Response, Status};

pub mod proto {
    tonic::include_proto!("arkhe.recurrency.v1");
}

use proto::recurrency_service_server::RecurrencyService;
use proto::{ProcessRequest, ProcessResponse, RegimeRequest, RegimeResponse, WatchRequest};

/// Wire a core engine into the gRPC contract.
#[derive(Debug)]
pub struct RecurrencyGrpcService {
    engine: Arc<Mutex<RecurrencyEngine>>,
    /// Ticket fan-out for `WatchTickets` subscribers (Safe-Core watchdog).
    tickets: tokio::sync::broadcast::Sender<ProcessResponse>,
}

impl RecurrencyGrpcService {
    /// Wrap an engine. The engine is shared across all RPCs.
    pub fn new(engine: RecurrencyEngine) -> Self {
        let (tickets, _) = tokio::sync::broadcast::channel(256);
        Self {
            engine: Arc::new(Mutex::new(engine)),
            tickets,
        }
    }

    /// Engine behind this service (for health probes / shutdown inspection).
    pub fn engine(&self) -> &Arc<Mutex<RecurrencyEngine>> {
        &self.engine
    }
}

fn map_regime(r: i32) -> Option<ArousalRegime> {
    match r {
        1 => Some(ArousalRegime::Coma),
        2 => Some(ArousalRegime::DeepSleep),
        3 => Some(ArousalRegime::Drowsy),
        4 => Some(ArousalRegime::Alert),
        5 => Some(ArousalRegime::Hypervigilant),
        _ => None,
    }
}

fn to_proto_regime(r: ArousalRegime) -> i32 {
    match r {
        ArousalRegime::Coma => 1,
        ArousalRegime::DeepSleep => 2,
        ArousalRegime::Drowsy => 3,
        ArousalRegime::Alert => 4,
        ArousalRegime::Hypervigilant => 5,
    }
}

/// Fill the ticket with the two policy-facing hints.
///
/// * `suggested_gain` — if access failed because closure fell short of the
///   threshold, the multiplier needed on the next cycle to clear it
///   (clamped to `[1.0, 5.0]`); `1.0` when access already cleared.
/// * `burnt_fuel` — abstract cost of the cycle, proportional to integrator
///   work (`steps × sites`), normalized so it stays a small float.
fn policy_hints(ticket: &RecurrencyTicket, threshold: f64, steps: usize, sites: usize) -> (f64, f64) {
    let suggested_gain = if ticket.access_granted {
        1.0
    } else if ticket.local_depth > 1e-12 && ticket.local_depth < threshold {
        (threshold / ticket.local_depth).clamp(1.0, 5.0)
    } else {
        1.0
    };
    let burnt_fuel = (steps * sites) as f64 / 1000.0;
    (suggested_gain, burnt_fuel)
}

#[tonic::async_trait]
impl RecurrencyService for RecurrencyGrpcService {
    async fn process_stimulus(
        &self,
        request: Request<ProcessRequest>,
    ) -> Result<Response<ProcessResponse>, Status> {
        let req = request.into_inner();
        let mut engine = self
            .engine
            .lock()
            .map_err(|_| Status::internal("recurrency engine lock poisoned"))?;

        // Optional Safe-Core override applied before the frame is processed.
        if let Some(regime) = map_regime(req.regime) {
            let _previous = engine.daemon.set_regime(regime);
        }

        let embedding = DVector::from_vec(req.embedding);
        if embedding.len() != engine.perception.n {
            return Err(Status::invalid_argument(format!(
                "embedding dimension {} does not match substrate n={}",
                embedding.len(),
                engine.perception.n
            )));
        }

        let ticket: RecurrencyTicket = engine.process_stimulus(StimulusId(req.stimulus_id), &embedding);

        let (suggested_gain, burnt_fuel) =
            policy_hints(&ticket, engine.workspace.closure_threshold, engine.perception.steps, engine.perception.n);

        let response = ProcessResponse {
            stimulus_id: ticket.stimulus_id.0,
            local_depth: ticket.local_depth,
            error_reduction: ticket.error_reduction,
            local_loop_closed: ticket.local_loop_closed,
            access_granted: ticket.access_granted,
            tick: ticket.tick,
            suggested_gain,
            burnt_fuel,
        };
        // Fan the ticket out to WatchTickets subscribers (Safe-Core watchdog).
        let _ = self.tickets.send(response.clone());
        Ok(Response::new(response))
    }

    async fn set_regime(
        &self,
        request: Request<RegimeRequest>,
    ) -> Result<Response<RegimeResponse>, Status> {
        let req = request.into_inner();
        let regime = map_regime(req.regime)
            .ok_or_else(|| Status::invalid_argument("regime is AROUSAL_UNSPECIFIED"))?;
        let mut engine = self
            .engine
            .lock()
            .map_err(|_| Status::internal("recurrency engine lock poisoned"))?;
        let previous = engine.daemon.set_regime(regime);
        Ok(Response::new(RegimeResponse {
            previous: to_proto_regime(previous),
            current: to_proto_regime(regime),
        }))
    }

    type WatchTicketsStream = std::pin::Pin<
        Box<dyn futures_core::Stream<Item = Result<ProcessResponse, Status>> + Send + 'static>,
    >;

    async fn watch_tickets(
        &self,
        _request: Request<WatchRequest>,
    ) -> Result<Response<Self::WatchTicketsStream>, Status> {
        let mut rx = self.tickets.subscribe();
        let stream = async_stream::stream! {
            loop {
                match rx.recv().await {
                    Ok(resp) => yield Ok(resp),
                    // Slow subscriber: drop the lagged frames, keep streaming.
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        };
        Ok(Response::new(Box::pin(stream)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arkhe_recurrency::{GlobalWorkspace, PerceptionLoop, StateDaemon};
    use proto::recurrency_service_server::RecurrencyServiceServer;
    use proto::ArousalRegime as ProtoRegime;

    fn engine() -> RecurrencyEngine {
        RecurrencyEngine::new(
            PerceptionLoop::default(),
            StateDaemon::new(ArousalRegime::Alert),
            GlobalWorkspace::new(0.3),
        )
    }

    fn embedding(dim: usize) -> Vec<f64> {
        (0..dim).map(|i| 0.5 + (i as f64) * 0.1).collect()
    }

    #[tokio::test]
    async fn process_stimulus_round_trips_through_the_engine() {
        let svc = RecurrencyGrpcService::new(engine());
        let resp = svc
            .process_stimulus(Request::new(ProcessRequest {
                stimulus_id: "s0".into(),
                embedding: embedding(8),
                regime: ProtoRegime::ArousalUnspecified.into(),
            }))
            .await
            .expect("process must succeed")
            .into_inner();
        assert_eq!(resp.stimulus_id, "s0");
        assert!(resp.local_loop_closed, "default loop must close");
        assert!(resp.error_reduction > 0.0);
        assert!(resp.suggested_gain >= 1.0);
        assert!(resp.burnt_fuel > 0.0);
    }

    #[tokio::test]
    async fn wrong_dimension_is_rejected() {
        let svc = RecurrencyGrpcService::new(engine());
        let err = svc
            .process_stimulus(Request::new(ProcessRequest {
                stimulus_id: "bad".into(),
                embedding: vec![1.0, 2.0], // n = 8 expected
                regime: ProtoRegime::ArousalUnspecified.into(),
            }))
            .await
            .expect_err("dimension mismatch must fail");
        assert_eq!(err.code(), tonic::Code::InvalidArgument);
    }

    #[tokio::test]
    async fn deep_sleep_override_suppresses_access() {
        let svc = RecurrencyGrpcService::new(engine());
        let set = svc
            .set_regime(Request::new(RegimeRequest {
                regime: ProtoRegime::ArousalDeepSleep.into(),
            }))
            .await
            .expect("regime switch must succeed")
            .into_inner();
        assert_eq!(set.previous, ProtoRegime::ArousalAlert as i32);
        assert_eq!(set.current, ProtoRegime::ArousalDeepSleep as i32);

        let resp = svc
            .process_stimulus(Request::new(ProcessRequest {
                stimulus_id: "s1".into(),
                embedding: embedding(8),
                regime: ProtoRegime::ArousalUnspecified.into(),
            }))
            .await
            .expect("process must succeed")
            .into_inner();
        assert!(
            !resp.access_granted,
            "DeepSleep must suppress broadcast even when content is processed"
        );
    }

    #[test]
    fn server_type_is_constructible() {
        // Compile-time check that the generated server adapter type exists.
        let _: Option<RecurrencyServiceServer<RecurrencyGrpcService>> = None;
    }
}
