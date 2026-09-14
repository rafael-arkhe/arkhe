//! The bus: fail-closed `publish`, typed `subscribe`, and the enforcement gates.
//!
//! §5.2 of the canonical paper (DOI 10.5281/zenodo.21383201) fixes this
//! contract, quoted verbatim because it is short and every clause is
//! implemented here:
//!
//! > `EventBus / Publisher / Subscriber` — typed topics (attestation
//! > completed/failed, storage stored, amendments proposed/applied, kill switch
//! > activated, credentials verified/rejected, alerts); mandatory `EventMeta`
//! > with identifiers and schema version on every event; malformed events are
//! > unpublishable.
//!
//! §4.2 states the crate's one-line invariant (Table 1): *"No crate knows
//! another directly; all domain communication is typed pub/sub."* And the
//! roadmap's Phase 1 (2026–2027) delivers *"storage, identity, event bus, and
//! observability with local IPFS and **in-memory pub/sub**"* — which is the
//! scope of this crate.
//!
//! # `publish` is the only way in, and it is fail-closed
//!
//! An event reaches the log and the subscribers only if it passes three gates,
//! in this order, with no mutation before the last one:
//!
//! 1. **Structural** — [`Event::validate`]: mandatory meta, non-empty
//!    identifiers, a legal topic, a schema version with `major >= 1`, and
//!    derived values (canonical payload, signing bytes, digest) that agree with
//!    the fields.
//! 2. **Signature** — `EventBus::check_signature`, the `T-12` policy gate.
//! 3. **Uniqueness and append** — under the log lock, so the check and the
//!    append cannot interleave: `event_id`s are unique per bus instance, and the
//!    sequence number is assigned by the log.
//!
//! A refusal returns a [`crate::error::BusError`] that
//! [`crate::error::BusError::is_rejection`] classifies as such, and it leaves the
//! log length, the root and every subscriber untouched. The tests assert exactly
//! that, rather than asserting only that an `Err` was returned.
//!
//! # Why `tokio::sync::broadcast`
//!
//! Recorded as a decision, with the alternatives it beat:
//!
//! * **Fan-out is the requirement.** §5.2's `Subscriber` is plural: attestation,
//!   storage, governance, security and observability all consume the same
//!   stream. `mpsc` allows one consumer per channel and would need one channel
//!   per subscriber per topic, hand-rolled. `broadcast` gives each subscriber an
//!   independent cursor over the same sequence.
//! * **A bounded buffer with an explicit lag report.** `broadcast` never blocks
//!   `publish` and never drops silently: a slow subscriber gets
//!   [`BusError::SubscriberLagged`] naming how many events it missed, and the
//!   log still holds them for replay. `watch` keeps only the newest value, which
//!   would lose exactly the history §4.2 promises to replay.
//! * **`send` is synchronous**, so [`EventBus::publish`] keeps §5.2's
//!   `publish(event) -> Result<SequenceNumber, _>` shape and needs no runtime;
//!   only the consuming side is `async`.
//! * **Every subscriber sees every event in log order** for events published by
//!   a single thread — which is what makes a `A → B → C` causal chain arrive as
//!   a chain.
//!
//! ## Delivery semantics, stated rather than implied
//!
//! * **No history for late subscribers.** `broadcast` delivers from subscription
//!   time forward. To catch up, replay the log; to avoid double-handling, dedupe
//!   on `event_id` (which is unique per bus, see [`crate::log::EventLog`]).
//!   There is no at-least-once or exactly-once guarantee in Phase 1 — there is
//!   "delivered while subscribed, and always in the log".
//! * **Ordering is the log's, not the channel's, when publishers race.** Two
//!   threads publishing concurrently may be appended in one order and delivered
//!   in the other, because the broadcast happens after the log lock is released.
//!   Every delivery carries its [`SequenceNumber`], so a consumer that needs a
//!   total order sorts by it; the log never disagrees with itself.
//! * **In memory only.** A dropped `EventBus` closes its subscriptions
//!   ([`BusError::SubscriptionClosed`]) and loses the log.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use tokio::sync::broadcast;

use crate::error::{BusError, BusResult};
use crate::event::Event;
use crate::log::{Checkpoint, EventLog, MerkleRoot, SequenceNumber};
use crate::meta::{now_unix_us, EventId, IdGenerator, SchemaVersion};
use crate::signature::{SignaturePolicy, SignatureVerifier};
use crate::topic::Topic;

/// Bounded per-subscription buffer size used by [`BusConfig::default`].
///
/// Chosen so that a burst of a thousand events is absorbed without blocking the
/// publisher; beyond it, a lagging subscriber is *told* it lagged
/// ([`BusError::SubscriberLagged`]) instead of the bus stalling.
pub const DEFAULT_CHANNEL_CAPACITY: usize = 1024;

/// Bus configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BusConfig {
    signature_policy: SignaturePolicy,
    channel_capacity: usize,
}

impl BusConfig {
    /// Default configuration: [`SignaturePolicy::Optional`], capacity
    /// [`DEFAULT_CHANNEL_CAPACITY`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Require (or stop requiring) a verified signature on every event.
    pub fn with_signature_policy(mut self, signature_policy: SignaturePolicy) -> Self {
        self.signature_policy = signature_policy;
        self
    }

    /// Set the per-subscription buffer size.
    ///
    /// Clamped to at least `1`: `tokio`'s `broadcast::channel(0)` would panic,
    /// and a library that can be made to panic by configuration is not
    /// fail-closed. A capacity of `0` therefore means `1`, not "unbounded".
    pub fn with_channel_capacity(mut self, channel_capacity: usize) -> Self {
        self.channel_capacity = channel_capacity.max(1);
        self
    }

    /// The configured signature policy.
    pub fn signature_policy(&self) -> SignaturePolicy {
        self.signature_policy
    }

    /// The configured per-subscription buffer size.
    pub fn channel_capacity(&self) -> usize {
        self.channel_capacity
    }
}

impl Default for BusConfig {
    fn default() -> Self {
        Self {
            signature_policy: SignaturePolicy::default(),
            channel_capacity: DEFAULT_CHANNEL_CAPACITY,
        }
    }
}

/// An event as delivered: the event, plus where it sits in the log.
///
/// The sequence number travels with the delivery so that a consumer can order,
/// deduplicate, checkpoint and cite history without asking the bus again.
///
/// `PartialEq` but not `Eq`: an event's payload is a `serde_json::Value`, and
/// `Value` is not `Eq` (its numbers are floats).
#[derive(Debug, Clone, PartialEq)]
pub struct Delivery {
    sequence: SequenceNumber,
    event: Arc<Event>,
}

impl Delivery {
    /// Position of this event in the log.
    pub fn sequence(&self) -> SequenceNumber {
        self.sequence
    }

    /// The event.
    pub fn event(&self) -> &Arc<Event> {
        &self.event
    }

    /// Identity of the event.
    pub fn event_id(&self) -> &EventId {
        self.event.event_id()
    }

    /// Topic of the event.
    pub fn topic(&self) -> &Topic {
        self.event.topic()
    }

    /// Schema version of the event's payload.
    pub fn schema_version(&self) -> SchemaVersion {
        self.event.schema_version()
    }

    /// Whether a consumer at `consumer` can read this event. See `T-E3`.
    pub fn is_readable_by(&self, consumer: SchemaVersion) -> bool {
        self.schema_version().is_readable_by(consumer)
    }
}

/// What a [`Subscription`] is listening to.
#[derive(Debug, Clone, PartialEq, Eq)]
enum SubscriptionScope {
    /// One topic.
    Topic(Topic),
    /// Every topic, present and future.
    All,
}

/// A subscriber's handle.
///
/// Dropping it unsubscribes; the bus keeps publishing (the event is in the log
/// regardless). A `Subscription` that lags or that outlives its bus says so —
/// [`BusError::SubscriberLagged`] / [`BusError::SubscriptionClosed`] — rather
/// than going quiet.
#[derive(Debug)]
pub struct Subscription {
    scope: SubscriptionScope,
    receiver: broadcast::Receiver<Arc<Delivery>>,
}

impl Subscription {
    /// The subscribed topic, or `None` for a subscription to every topic.
    pub fn topic(&self) -> Option<&Topic> {
        match &self.scope {
            SubscriptionScope::Topic(topic) => Some(topic),
            SubscriptionScope::All => None,
        }
    }

    /// True when this subscription receives every topic.
    pub fn covers_all_topics(&self) -> bool {
        matches!(self.scope, SubscriptionScope::All)
    }

    /// True when the bus has been dropped and no further events can arrive.
    pub fn is_closed(&self) -> bool {
        self.receiver.is_closed()
    }

    /// Await the next delivery.
    ///
    /// # Errors
    ///
    /// [`BusError::SubscriberLagged`] if the bounded buffer overflowed — the
    /// count of missed events is reported and the subscription stays usable;
    /// [`BusError::SubscriptionClosed`] once the bus is gone.
    pub async fn recv(&mut self) -> BusResult<Arc<Delivery>> {
        match self.receiver.recv().await {
            Ok(delivery) => Ok(delivery),
            Err(error) => Err(from_recv_error(error)),
        }
    }

    /// Await the next delivery, refusing one whose schema this consumer cannot
    /// read (`T-E3`).
    ///
    /// A higher *minor* within a known major is accepted (additive); a higher
    /// *major* is refused with [`BusError::UnsupportedSchemaVersion`], which
    /// names the sequence so the caller can replay that event from the log and
    /// inspect the raw JSON rather than losing it. Nothing here panics, and
    /// nothing is coerced.
    ///
    /// # Errors
    ///
    /// [`BusError::SubscriberLagged`], [`BusError::SubscriptionClosed`], or
    /// [`BusError::UnsupportedSchemaVersion`].
    pub async fn recv_understanding(
        &mut self,
        consumer: SchemaVersion,
    ) -> BusResult<Arc<Delivery>> {
        let delivery = self.recv().await?;
        match delivery.is_readable_by(consumer) {
            true => Ok(delivery),
            false => Err(BusError::UnsupportedSchemaVersion {
                sequence: delivery.sequence().get(),
                event_major: delivery.schema_version().major(),
                consumer_major: consumer.major(),
            }),
        }
    }

    /// Non-blocking receive: `Ok(None)` when nothing is pending.
    ///
    /// The synchronous counterpart of [`Subscription::recv`], which is what lets
    /// a test (or a single-threaded consumer) assert "the subscriber received
    /// nothing" without a runtime or a sleep.
    ///
    /// # Errors
    ///
    /// [`BusError::SubscriberLagged`], [`BusError::SubscriptionClosed`].
    pub fn try_recv(&mut self) -> BusResult<Option<Arc<Delivery>>> {
        match self.receiver.try_recv() {
            Ok(delivery) => Ok(Some(delivery)),
            Err(broadcast::error::TryRecvError::Empty) => Ok(None),
            Err(broadcast::error::TryRecvError::Lagged(skipped)) => {
                Err(BusError::SubscriberLagged { skipped })
            }
            Err(broadcast::error::TryRecvError::Closed) => Err(BusError::SubscriptionClosed),
        }
    }

    /// Everything currently pending, in delivery order.
    ///
    /// # Errors
    ///
    /// [`BusError::SubscriberLagged`] — returned *after* the events it still
    /// could hand over, because the buffer it lost is lost; a caller that needs
    /// the full history replays the log.
    pub fn drain(&mut self) -> BusResult<Vec<Arc<Delivery>>> {
        let mut pending = Vec::new();
        loop {
            match self.try_recv() {
                Ok(Some(delivery)) => pending.push(delivery),
                Ok(None) => return Ok(pending),
                Err(error) => return Err(error),
            }
        }
    }
}

/// Translate a `broadcast` receive error. Never panics, never hides a loss.
fn from_recv_error(error: broadcast::error::RecvError) -> BusError {
    match error {
        broadcast::error::RecvError::Lagged(skipped) => BusError::SubscriberLagged { skipped },
        broadcast::error::RecvError::Closed => BusError::SubscriptionClosed,
    }
}

/// Acquire a bus lock, recovering if a previous holder panicked.
///
/// Recovery is deliberate, not careless. Every critical section in this crate is
/// a `Vec`/`HashMap` operation with **no user code inside it** — signature
/// verification, canonicalisation and event validation all happen outside the
/// locks — so a panic while holding one is not a state this crate can produce,
/// and an allocation failure aborts the process rather than unwinding. The
/// alternative (propagating `PoisonError` through every accessor) would turn a
/// non-event into a permanently unusable bus. Recorded here rather than left as
/// an unexplained `unwrap_or_else`.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The typed publish/subscribe bus.
///
/// Create one, subscribe to topics, publish events. The log is the authority;
/// the channels are a convenience for consumers that are already running. See
/// the module docs for the publish gates and the delivery semantics.
///
/// ```
/// use arkhe_event_bus::bus::{BusConfig, EventBus};
/// use arkhe_event_bus::event::Event;
/// use arkhe_event_bus::meta::{CorrelationId, EventId, EventMeta, SchemaVersion, TraceId};
/// use arkhe_event_bus::topic::ATTESTATION_COMPLETED;
///
/// # fn main() -> Result<(), arkhe_event_bus::error::BusError> {
/// let bus = EventBus::new(BusConfig::default());
/// let mut subscriber = bus.subscribe(ATTESTATION_COMPLETED);
///
/// let meta = EventMeta::root(
///     ATTESTATION_COMPLETED,
///     SchemaVersion::V1,
///     bus.new_event_id()?,
///     TraceId::new("trc_demo")?,
///     CorrelationId::new("cor_demo")?,
///     1,
/// );
/// let event = Event::new(meta, serde_json::json!({"graph": "g1"}))?;
///
/// let sequence = bus.publish(event)?;
/// assert_eq!(sequence.get(), 1);
///
/// let delivery = subscriber.try_recv()?.expect("delivered synchronously");
/// assert_eq!(delivery.sequence(), sequence);
/// assert_eq!(bus.current_root(), bus.log().current_root());
/// # Ok(())
/// # }
/// ```
pub struct EventBus {
    config: BusConfig,
    verifier: Option<Arc<dyn SignatureVerifier>>,
    ids: IdGenerator,
    log: Mutex<EventLog>,
    channels: Mutex<HashMap<Topic, broadcast::Sender<Arc<Delivery>>>>,
    all_topics: Mutex<Option<broadcast::Sender<Arc<Delivery>>>>,
}

impl EventBus {
    /// A bus with no signature verifier.
    ///
    /// With [`SignaturePolicy::Optional`] (the default) this accepts unsigned
    /// events. With [`SignaturePolicy::Required`] this state is explicitly
    /// representable and explicitly unusable: every `publish` fails with
    /// [`BusError::SignatureVerifierUnavailable`]. It is allowed to exist so that
    /// a misconfiguration is *reported* instead of being unrepresentable or, far
    /// worse, silently tolerated.
    pub fn new(config: BusConfig) -> Self {
        Self {
            config,
            verifier: None,
            ids: IdGenerator::new(),
            log: Mutex::new(EventLog::new()),
            channels: Mutex::new(HashMap::new()),
            all_topics: Mutex::new(None),
        }
    }

    /// A bus that verifies signatures with `verifier`.
    pub fn with_verifier(
        config: BusConfig,
        verifier: Arc<dyn SignatureVerifier>,
    ) -> Self {
        Self {
            verifier: Some(verifier),
            ..Self::new(config)
        }
    }

    /// The active configuration.
    pub fn config(&self) -> BusConfig {
        self.config
    }

    /// The active signature policy.
    pub fn signature_policy(&self) -> SignaturePolicy {
        self.config.signature_policy
    }

    /// Whether a verifier is configured.
    pub fn has_signature_verifier(&self) -> bool {
        self.verifier.is_some()
    }

    /// The bus's identifier generator.
    ///
    /// Publishers need trace and correlation identifiers too, so the generator
    /// is public; it is a convenience, not a trust boundary
    /// ([`IdGenerator`] explains why).
    pub fn id_generator(&self) -> &IdGenerator {
        &self.ids
    }

    /// Mint a fresh [`EventId`].
    ///
    /// # Errors
    ///
    /// [`BusError::InvalidId`] only if the generator produced something the
    /// identifier grammar rejects — which its own construction prevents.
    pub fn new_event_id(&self) -> BusResult<EventId> {
        self.ids.next_event_id(now_unix_us())
    }

    /// Publish an event.
    ///
    /// Returns the event's [`SequenceNumber`]. See the module docs for the three
    /// gates; `§5.2`'s *"malformed events are unpublishable"* is gates 1 and 2.
    ///
    /// # Errors
    ///
    /// Every error is a refusal ([`BusError::is_rejection`] is `true`) and leaves
    /// the log and all subscribers untouched.
    pub fn publish(&self, event: Event) -> BusResult<SequenceNumber> {
        self.publish_shared(Arc::new(event))
    }

    /// Publish an already-shared event, for relays and bridges that must hand the
    /// same value on without cloning the payload.
    ///
    /// # Errors
    ///
    /// As [`EventBus::publish`].
    pub fn publish_shared(&self, event: Arc<Event>) -> BusResult<SequenceNumber> {
        // Gate 1 — structural (§5.2).
        event.validate()?;

        // Gate 2 — signature policy (T-12). Outside the lock: verification is
        // caller-supplied code and must never run while the bus is locked.
        self.check_signature(&event)?;

        // Gate 3 — uniqueness and append, atomically.
        let (sequence, delivery) = {
            let mut log = lock(&self.log);

            if log.contains_event_id(event.event_id().as_str()) {
                return Err(BusError::DuplicateEventId {
                    event_id: event.event_id().to_string(),
                });
            }

            let sequence = log.append(Arc::clone(&event));
            (
                sequence,
                Arc::new(Delivery {
                    sequence,
                    event: Arc::clone(&event),
                }),
            )
        };

        self.deliver(delivery);
        Ok(sequence)
    }

    /// Subscribe to one topic.
    ///
    /// Subscribing to a topic nobody publishes is legal and quiet; subscribing
    /// to a topic this crate has never heard of is equally legal — that is `T-E3`
    /// on the publisher/subscriber axis, and it is why `Topic` is a validated
    /// string rather than an enum.
    pub fn subscribe(&self, topic: Topic) -> Subscription {
        let receiver = {
            let mut channels = lock(&self.channels);
            let capacity = self.config.channel_capacity;
            let sender = channels
                .entry(topic.clone())
                .or_insert_with(|| broadcast::channel(capacity).0);
            sender.subscribe()
        };

        Subscription {
            scope: SubscriptionScope::Topic(topic),
            receiver,
        }
    }

    /// Subscribe to every topic, including topics that do not exist yet.
    ///
    /// This is how the observability crate of Table 1 consumes the stream.
    pub fn subscribe_all(&self) -> Subscription {
        let receiver = {
            let mut all_topics = lock(&self.all_topics);
            let capacity = self.config.channel_capacity;
            let sender = all_topics
                .get_or_insert_with(|| broadcast::channel(capacity).0);
            sender.subscribe()
        };

        Subscription {
            scope: SubscriptionScope::All,
            receiver,
        }
    }

    /// Live subscribers on `topic`.
    pub fn subscriber_count(&self, topic: &Topic) -> usize {
        lock(&self.channels)
            .get(topic)
            .map(|sender| sender.receiver_count())
            .unwrap_or(0)
    }

    /// Live subscribers on every topic.
    pub fn all_topics_subscriber_count(&self) -> usize {
        lock(&self.all_topics)
            .as_ref()
            .map(|sender| sender.receiver_count())
            .unwrap_or(0)
    }

    /// A copy of the log at this instant.
    ///
    /// A snapshot, not a view: events published afterwards are not in it, and
    /// events in it cannot be changed by publishing. Cloning is the Phase 1
    /// trade: it keeps the lock entirely inside this module, at the cost of
    /// copying the entry list (the events themselves are shared `Arc`s).
    pub fn log(&self) -> EventLog {
        lock(&self.log).clone()
    }

    /// Number of events in the log.
    pub fn log_len(&self) -> usize {
        lock(&self.log).len()
    }

    /// The highest assigned sequence number, or `None` for an empty log.
    pub fn sequence_head(&self) -> Option<SequenceNumber> {
        lock(&self.log).head()
    }

    /// The Merkle root of the whole log.
    pub fn current_root(&self) -> MerkleRoot {
        lock(&self.log).current_root()
    }

    /// A checkpoint at the head of the log.
    pub fn checkpoint(&self) -> Checkpoint {
        lock(&self.log).checkpoint()
    }

    /// A checkpoint at `seq`.
    ///
    /// # Errors
    ///
    /// [`BusError::ReplayOutOfRange`] when `seq` is beyond the head.
    pub fn checkpoint_at(&self, seq: SequenceNumber) -> BusResult<Checkpoint> {
        lock(&self.log).checkpoint_at(seq)
    }

    /// Replay from `from` (inclusive). See [`EventLog::replay`].
    ///
    /// # Errors
    ///
    /// [`BusError::ReplayOutOfRange`].
    pub fn replay(&self, from: SequenceNumber) -> BusResult<Vec<Arc<Event>>> {
        lock(&self.log).replay(from)
    }

    /// Check the log against a checkpoint. See [`EventLog::verify_against`].
    ///
    /// # Errors
    ///
    /// [`BusError::CheckpointMismatch`], [`BusError::ReplayOutOfRange`].
    pub fn verify_against(&self, checkpoint: &Checkpoint) -> BusResult<()> {
        lock(&self.log).verify_against(checkpoint)
    }

    /// The `T-12` / `T-13` signature gate.
    ///
    /// | Policy | Event | Outcome |
    /// |---|---|---|
    /// | `Required` | no verifier configured | [`BusError::SignatureVerifierUnavailable`] |
    /// | `Required` | unsigned | [`BusError::MissingSignature`] |
    /// | `Required` | signed, verifier says `false` | [`BusError::InvalidSignature`] |
    /// | `Required` | signed, verifier says `true` | accepted |
    /// | `Optional` | unsigned | accepted |
    /// | `Optional` | signed, no verifier | [`BusError::SignatureVerifierUnavailable`] |
    /// | `Optional` | signed, verifier says `false` | [`BusError::InvalidSignature`] |
    fn check_signature(&self, event: &Event) -> BusResult<()> {
        let required = self.config.signature_policy == SignaturePolicy::Required;

        if required && self.verifier.is_none() {
            return Err(BusError::SignatureVerifierUnavailable);
        }

        let signature = match event.signature() {
            Some(signature) => signature,
            None => {
                return match required {
                    true => Err(BusError::MissingSignature {
                        event_id: event.event_id().to_string(),
                    }),
                    false => Ok(()),
                }
            }
        };

        // A signature is present. With no verifier it cannot be checked, and an
        // unverifiable signature is never accepted on faith.
        let verifier = match self.verifier.as_ref() {
            Some(verifier) => verifier,
            None => return Err(BusError::SignatureVerifierUnavailable),
        };

        match verifier.verify(event.signing_bytes(), signature.as_bytes()) {
            true => Ok(()),
            false => Err(BusError::InvalidSignature {
                event_id: event.event_id().to_string(),
            }),
        }
    }

    /// Hand a delivery to every interested subscriber.
    ///
    /// Senders are cloned out of the registry first and the sends happen after
    /// every lock is released, so no subscriber-visible work occurs while the bus
    /// is locked.
    fn deliver(&self, delivery: Arc<Delivery>) {
        let senders: Vec<broadcast::Sender<Arc<Delivery>>> = {
            let channels = lock(&self.channels);
            let mut senders = Vec::with_capacity(2);

            if let Some(sender) = channels.get(delivery.topic()) {
                senders.push(sender.clone());
            }

            let all_topics = lock(&self.all_topics);
            if let Some(sender) = all_topics.as_ref() {
                senders.push(sender.clone());
            }

            senders
        };

        for sender in senders {
            if sender.receiver_count() == 0 {
                // Nobody is listening to this channel. Not an error: delivery is
                // per-subscriber, and the event is in the log either way.
                continue;
            }
            // `send` fails only if every receiver was dropped between the count
            // check and the send. Nothing is silently lost: the event is in the
            // log, and the subscriptions that disappeared were dropped by their
            // owners.
            let _ = sender.send(Arc::clone(&delivery));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meta::{CorrelationId, EventMeta, TraceId};
    use crate::signature::EventSignature;
    use crate::topic::{ALERTS, ATTESTATION_FAILED, STORAGE_STORED};
    use serde_json::json;

    fn meta(topic: Topic, event_id: &str) -> EventMeta {
        EventMeta::root(
            topic,
            SchemaVersion::V1,
            EventId::new(event_id).expect("valid"),
            TraceId::new("trc_bus").expect("valid"),
            CorrelationId::new("cor_bus").expect("valid"),
            1,
        )
    }

    fn event(topic: Topic, event_id: &str) -> Event {
        Event::new(meta(topic, event_id), json!({"k": event_id})).expect("valid event")
    }

    #[test]
    fn the_bus_is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<EventBus>();
        assert_send_sync::<Subscription>();
        assert_send_sync::<Delivery>();
    }

    #[test]
    fn config_defaults_and_clamps() {
        let default = BusConfig::new();
        assert_eq!(default.signature_policy(), SignaturePolicy::Optional);
        assert_eq!(default.channel_capacity(), DEFAULT_CHANNEL_CAPACITY);
        assert_eq!(BusConfig::default(), default);

        // `tokio`'s broadcast channel panics at capacity 0; configuration must
        // not be able to panic a library.
        assert_eq!(BusConfig::new().with_channel_capacity(0).channel_capacity(), 1);
        assert_eq!(
            BusConfig::new()
                .with_signature_policy(SignaturePolicy::Required)
                .signature_policy(),
            SignaturePolicy::Required
        );
    }

    #[test]
    fn publish_assigns_monotonic_sequences_and_delivers_by_topic() {
        let bus = EventBus::new(BusConfig::default());
        let mut storage = bus.subscribe(STORAGE_STORED);
        let mut alerts = bus.subscribe(ALERTS);

        let first = bus
            .publish(event(STORAGE_STORED, "evt_1"))
            .expect("accepted");
        let second = bus
            .publish(event(STORAGE_STORED, "evt_2"))
            .expect("accepted");
        let third = bus.publish(event(ALERTS, "evt_3")).expect("accepted");

        assert_eq!(
            (first.get(), second.get(), third.get()),
            (1, 2, 3),
            "sequence numbers are monotonic from 1"
        );

        let storage_seen: Vec<String> = storage
            .drain()
            .expect("no lag")
            .iter()
            .map(|delivery| delivery.event_id().to_string())
            .collect();
        assert_eq!(storage_seen, ["evt_1", "evt_2"]);

        let alerts_seen: Vec<String> = alerts
            .drain()
            .expect("no lag")
            .iter()
            .map(|delivery| delivery.event_id().to_string())
            .collect();
        assert_eq!(alerts_seen, ["evt_3"]);

        assert_eq!(bus.log_len(), 3);
        assert_eq!(bus.sequence_head(), Some(SequenceNumber::new(3)));
    }

    #[test]
    fn subscriptions_to_unknown_topics_are_legal_and_quiet() {
        let bus = EventBus::new(BusConfig::default());
        let unknown = Topic::parse("arkhe_physics.experiment.published").expect("valid");
        let mut subscriber = bus.subscribe(unknown.clone());

        assert_eq!(bus.subscriber_count(&unknown), 1);
        assert_eq!(subscriber.topic(), Some(&unknown));
        assert!(!subscriber.covers_all_topics());
        assert!(subscriber.try_recv().expect("no lag").is_none());

        bus.publish(event(unknown.clone(), "evt_future"))
            .expect("unknown topics publish");
        let delivery = subscriber.try_recv().expect("no lag").expect("delivered");
        assert_eq!(delivery.topic(), &unknown);
    }

    #[test]
    fn subscribe_all_receives_every_topic() {
        let bus = EventBus::new(BusConfig::default());
        let mut all = bus.subscribe_all();

        assert!(all.covers_all_topics());
        assert_eq!(all.topic(), None);
        assert_eq!(bus.all_topics_subscriber_count(), 1);

        bus.publish(event(STORAGE_STORED, "evt_a")).expect("accepted");
        bus.publish(event(ALERTS, "evt_b")).expect("accepted");
        bus.publish(event(ATTESTATION_FAILED, "evt_c")).expect("accepted");

        let seen: Vec<String> = all
            .drain()
            .expect("no lag")
            .iter()
            .map(|delivery| delivery.topic().to_string())
            .collect();
        assert_eq!(seen, ["storage.stored", "alerts", "attestation.failed"]);
    }

    #[test]
    fn duplicate_event_ids_are_refused_without_touching_the_log() {
        let bus = EventBus::new(BusConfig::default());
        let mut subscriber = bus.subscribe(STORAGE_STORED);

        bus.publish(event(STORAGE_STORED, "evt_same")).expect("accepted");
        let root_after_first = bus.current_root();

        match bus.publish(event(STORAGE_STORED, "evt_same")) {
            Err(BusError::DuplicateEventId { event_id }) => assert_eq!(event_id, "evt_same"),
            other => panic!("expected DuplicateEventId, got {other:?}"),
        }

        assert_eq!(bus.log_len(), 1, "the refused event must not enter the log");
        assert_eq!(bus.current_root(), root_after_first, "root must not move");
        assert_eq!(
            subscriber.drain().expect("no lag").len(),
            1,
            "the refused event must not be delivered"
        );
    }

    #[test]
    fn required_policy_without_a_verifier_refuses_every_publish() {
        let bus = EventBus::new(
            BusConfig::default().with_signature_policy(SignaturePolicy::Required),
        );
        let mut subscriber = bus.subscribe(STORAGE_STORED);

        assert!(!bus.has_signature_verifier());
        assert_eq!(bus.signature_policy(), SignaturePolicy::Required);

        // Unsigned, signed — it makes no difference: the configuration cannot
        // validate anything, so nothing is accepted.
        let unsigned = event(STORAGE_STORED, "evt_u");
        assert_eq!(
            bus.publish(unsigned),
            Err(BusError::SignatureVerifierUnavailable)
        );

        let signed = event(STORAGE_STORED, "evt_s").with_signature(
            EventSignature::from_bytes(vec![1u8; 64]).expect("valid signature"),
        );
        assert_eq!(
            bus.publish(signed),
            Err(BusError::SignatureVerifierUnavailable)
        );

        assert!(BusError::SignatureVerifierUnavailable.is_rejection());
        assert_eq!(bus.log_len(), 0);
        assert!(subscriber.try_recv().expect("no lag").is_none());
    }

    #[test]
    fn required_policy_refuses_missing_and_invalid_signatures() {
        let verifier: Arc<dyn SignatureVerifier> =
            Arc::new(|message: &[u8], signature: &[u8]| {
                // A deliberately trivial stand-in for a real scheme: it proves
                // the *plumbing* (the bus hands the verifier exactly the signing
                // bytes), not any cryptography.
                !message.is_empty() && signature == [7u8; 64].as_slice()
            });
        let bus = EventBus::with_verifier(
            BusConfig::default().with_signature_policy(SignaturePolicy::Required),
            verifier,
        );

        assert!(bus.has_signature_verifier());

        assert_eq!(
            bus.publish(event(STORAGE_STORED, "evt_unsigned")),
            Err(BusError::MissingSignature {
                event_id: "evt_unsigned".to_string()
            })
        );

        let wrong = event(STORAGE_STORED, "evt_wrong")
            .with_signature(EventSignature::from_bytes(vec![0u8; 64]).expect("valid"));
        assert_eq!(
            bus.publish(wrong),
            Err(BusError::InvalidSignature {
                event_id: "evt_wrong".to_string()
            })
        );

        let right = event(STORAGE_STORED, "evt_right")
            .with_signature(EventSignature::from_bytes(vec![7u8; 64]).expect("valid"));
        assert_eq!(bus.publish(right).expect("accepted").get(), 1);
    }

    #[test]
    fn optional_policy_accepts_unsigned_but_never_unverifiable() {
        // With a verifier: unsigned passes, a bad signature does not.
        let verifier: Arc<dyn SignatureVerifier> =
            Arc::new(|_m: &[u8], s: &[u8]| s == [3u8; 64].as_slice());
        let bus = EventBus::with_verifier(BusConfig::default(), verifier);

        bus.publish(event(STORAGE_STORED, "evt_plain"))
            .expect("unsigned events are accepted under Optional");

        let forged = event(STORAGE_STORED, "evt_forged")
            .with_signature(EventSignature::from_bytes(vec![9u8; 64]).expect("valid"));
        assert_eq!(
            bus.publish(forged),
            Err(BusError::InvalidSignature {
                event_id: "evt_forged".to_string()
            })
        );
        assert_eq!(bus.log_len(), 1);

        // Without a verifier: a signature that cannot be checked is refused.
        let no_verifier = EventBus::new(BusConfig::default());
        let unverifiable = event(STORAGE_STORED, "evt_unverifiable")
            .with_signature(EventSignature::from_bytes(vec![1u8; 64]).expect("valid"));
        assert_eq!(
            no_verifier.publish(unverifiable),
            Err(BusError::SignatureVerifierUnavailable)
        );
        assert_eq!(no_verifier.log_len(), 0);
    }

    #[test]
    fn lagging_subscribers_are_told_rather_than_disconnected() {
        let bus = EventBus::new(BusConfig::default().with_channel_capacity(2));
        let mut slow = bus.subscribe(STORAGE_STORED);

        for n in 0..5 {
            bus.publish(event(STORAGE_STORED, &format!("evt_{n}")))
                .expect("accepted");
        }

        // The first read is told what was lost.
        match slow.drain() {
            Err(BusError::SubscriberLagged { skipped }) => {
                assert!(skipped >= 3, "expected at least 3 skipped, got {skipped}");
            }
            other => panic!("expected SubscriberLagged, got {other:?}"),
        }

        // The subscription survives the lag: the retained window still arrives,
        // in order, with no gap presented as if it were contiguous.
        let retained: Vec<String> = slow
            .drain()
            .expect("usable after lag")
            .iter()
            .map(|delivery| delivery.event_id().to_string())
            .collect();
        assert!(retained.len() <= 2, "capacity was 2, got {retained:?}");
        assert_eq!(
            retained.last().map(String::as_str),
            Some("evt_4"),
            "the newest event is still delivered: {retained:?}"
        );

        // And it keeps up with new events.
        bus.publish(event(STORAGE_STORED, "evt_after"))
            .expect("accepted");
        let after = slow.try_recv().expect("no lag").expect("delivered");
        assert_eq!(after.event_id().as_str(), "evt_after");

        // The log kept everything regardless of who was listening.
        assert_eq!(bus.log_len(), 6);
        assert_eq!(bus.replay(SequenceNumber::FIRST).expect("ok").len(), 6);
    }

    #[test]
    fn an_empty_log_has_the_empty_root_and_a_genesis_checkpoint() {
        let bus = EventBus::new(BusConfig::default());
        assert_eq!(bus.current_root(), MerkleRoot::empty());
        assert_eq!(bus.log(), EventLog::new());
        assert!(bus.checkpoint().is_genesis());
        assert!(bus.checkpoint_at(SequenceNumber::FIRST).is_err());
        assert!(bus.log().verify_integrity().is_ok());
    }

    #[test]
    fn log_snapshots_are_point_in_time() {
        let bus = EventBus::new(BusConfig::default());
        bus.publish(event(STORAGE_STORED, "evt_1")).expect("accepted");

        let snapshot = bus.log();
        bus.publish(event(STORAGE_STORED, "evt_2")).expect("accepted");

        assert_eq!(snapshot.len(), 1);
        assert_eq!(bus.log_len(), 2);
        assert_ne!(snapshot.current_root(), bus.current_root());
    }

    #[test]
    fn checkpoint_verification_round_trips_through_the_bus() {
        let bus = EventBus::new(BusConfig::default());
        for n in 0..4 {
            bus.publish(event(STORAGE_STORED, &format!("evt_{n}")))
                .expect("accepted");
        }

        let checkpoint = bus.checkpoint_at(SequenceNumber::new(2)).expect("ok");
        assert!(bus.verify_against(&checkpoint).is_ok());
        assert_eq!(checkpoint.root(), bus.log().root_at(SequenceNumber::new(2)).expect("ok"));

        // A rewritten prefix is refused.
        let rewritten = EventLog::restore(vec![
            event(STORAGE_STORED, "evt_0").into(),
            Event::new(meta(STORAGE_STORED, "evt_1"), json!({"k": "different"}))
                .expect("valid")
                .into(),
        ])
        .expect("restore");
        match rewritten.verify_against(&checkpoint) {
            Err(BusError::CheckpointMismatch { checkpoint_seq, .. }) => {
                assert_eq!(checkpoint_seq, 2);
            }
            other => panic!("expected CheckpointMismatch, got {other:?}"),
        }
    }

    #[test]
    fn id_generator_is_usable_from_the_bus() {
        let bus = EventBus::new(BusConfig::default());
        let first = bus.new_event_id().expect("valid");
        let second = bus.new_event_id().expect("valid");
        assert_ne!(first, second);
        assert!(first.as_str().starts_with("evt_"));
        assert!(bus.id_generator().next_trace_id(1).is_ok());
    }

    #[test]
    fn publish_shared_accepts_a_relayed_event() {
        let bus = EventBus::new(BusConfig::default());
        let shared = Arc::new(event(STORAGE_STORED, "evt_relay"));
        let sequence = bus.publish_shared(Arc::clone(&shared)).expect("accepted");
        assert_eq!(sequence.get(), 1);
        assert_eq!(bus.log().entries()[0].event(), &shared);
    }
}
