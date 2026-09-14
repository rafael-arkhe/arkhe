//! The append-only in-memory log: monotonic sequence numbers, a Merkle root
//! and checkpoint fencing.
//!
//! §4.2 of the canonical paper (DOI 10.5281/zenodo.21383201) promises that *"any
//! flow can be replayed or audited from the event stream alone"*; §5.2 gives
//! `ImmutableStore`/`EventSourcedLog` *"append, replay, snapshot, and
//! `current_root` over a Merkle-rooted, append-only log"*; and §6.2 anchors
//! **T-13** — *"replay against the event log"* — on *"monotonic sequence
//! numbers, Merkle roots, signed events, checkpoint fencing"*.
//!
//! This module implements the three of those four that are the log's own: the
//! sequence numbers, the Merkle root over the event digests, and the fencing.
//! *Signed events* are the publisher's contribution
//! ([`crate::signature`]).
//!
//! # What the Merkle root does and does not prove
//!
//! [`EventLog::current_root`] is a Merkle root over the *content digest* of each
//! logged event ([`crate::event::Event::digest`], i.e. `BLAKE3` over the event's
//! signing bytes), with leaves and internal nodes domain-separated. It follows
//! that:
//!
//! * changing any byte of any logged event changes the root;
//! * reordering two events changes the root (the construction is
//!   position-sensitive: the `n`-th leaf sits at a fixed place in the tree);
//! * truncating the log changes the root;
//! * the same sequence of events always yields the same root.
//!
//! It does **not** follow that the root can distinguish two events whose bytes
//! are identical — a swapped pair of byte-identical events is not a
//! distinguishable change, because there is nothing to distinguish. And the
//! root says nothing about whether an event *should* have been published; it is
//! a tamper detector for stored history, not a policy.
//!
//! # Phase 1 limits, stated here so they are not discovered later
//!
//! * In memory only. Nothing is persisted; a process exit loses the log.
//! * No proof-of-inclusion API. The root is computable, but this crate does not
//!   ship Merkle *paths* (the Phase 1 roadmap pairs this bus with
//!   `arkhe-storage` for that).
//! * `snapshot` is a clone ([`EventLog`] is `Clone`), not a persistent or
//!   incremental structure.

use std::collections::HashSet;
use std::sync::Arc;

use crate::error::{BusError, BusResult};
use crate::event::Event;
use crate::meta::EventId;
use crate::topic::Topic;

/// Domain separator for the root of an empty log.
pub const EMPTY_ROOT_DOMAIN: &[u8] = b"ARKHE-EVENT-BUS/v1/merkle/empty";

/// A position in the log.
///
/// `0` is [`SequenceNumber::GENESIS`] — "before the first event" — and is only
/// ever produced by an empty log, never assigned to an event. The first
/// appended event gets [`SequenceNumber::FIRST`] (`1`), and every later event
/// gets the previous number plus one, so the sequence is strictly increasing for
/// as long as the log lives. `Publish` returns the event's number, which is what
/// makes "monotonic sequence numbers" checkable from outside.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SequenceNumber(u64);

impl SequenceNumber {
    /// `0` — before the first event.
    pub const GENESIS: Self = Self(0);

    /// `1` — the first appended event.
    pub const FIRST: Self = Self(1);

    /// Wrap a raw position.
    ///
    /// Infallible by design: every `u64` is a legal *position*, including
    /// [`SequenceNumber::GENESIS`]. Whether a position exists in a given log is
    /// decided by that log ([`EventLog::replay`], [`EventLog::root_at`]).
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// The raw position.
    pub const fn get(&self) -> u64 {
        self.0
    }

    /// The next position, saturating at [`u64::MAX`] rather than wrapping.
    pub const fn next(&self) -> Self {
        Self(self.0.saturating_add(1))
    }

    /// True for [`SequenceNumber::GENESIS`].
    pub const fn is_genesis(&self) -> bool {
        self.0 == 0
    }
}

impl std::fmt::Display for SequenceNumber {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<u64> for SequenceNumber {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl From<SequenceNumber> for u64 {
    fn from(value: SequenceNumber) -> Self {
        value.0
    }
}

/// A BLAKE3 Merkle root over a log's event digests.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MerkleRoot([u8; 32]);

impl MerkleRoot {
    /// Byte width of a root.
    pub const LEN: usize = 32;

    /// The root of an empty log: `BLAKE3(EMPTY_ROOT_DOMAIN)`.
    ///
    /// A distinct domain, so it cannot collide with a one-leaf root.
    pub fn empty() -> Self {
        Self(*blake3::hash(EMPTY_ROOT_DOMAIN).as_bytes())
    }

    /// Borrow the raw bytes.
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Owned copy of the raw bytes.
    pub const fn to_array(&self) -> [u8; 32] {
        self.0
    }

    /// Lowercase hex rendering (64 characters).
    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }

    /// Parse from hex. Rejects wrong lengths and non-hex input, so a checkpoint
    /// read from an untrusted file cannot carry a malformed root.
    pub fn from_hex(value: &str) -> BusResult<Self> {
        let raw = hex::decode(value)
            .map_err(|e| BusError::malformed("merkle_root", format!("not hex: {e}")))?;
        let bytes: [u8; 32] = raw.try_into().map_err(|raw: Vec<u8>| {
            BusError::malformed(
                "merkle_root",
                format!("decoded {} bytes, expected 32", raw.len()),
            )
        })?;
        Ok(Self(bytes))
    }
}

impl From<[u8; 32]> for MerkleRoot {
    fn from(value: [u8; 32]) -> Self {
        Self(value)
    }
}

impl std::fmt::Debug for MerkleRoot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MerkleRoot({})", self.to_hex())
    }
}

impl std::fmt::Display for MerkleRoot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_hex())
    }
}

impl serde::Serialize for MerkleRoot {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_hex())
    }
}

impl<'de> serde::Deserialize<'de> for MerkleRoot {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = <String as serde::Deserialize>::deserialize(deserializer)?;
        MerkleRoot::from_hex(&value).map_err(<D::Error as serde::de::Error>::custom)
    }
}

/// Leaf hash: `BLAKE3(LEAF_DOMAIN ‖ digest)`.
fn hash_leaf(digest: &[u8; 32]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&[crate::event::LEAF_DOMAIN]);
    hasher.update(digest);
    *hasher.finalize().as_bytes()
}

/// Internal node hash: `BLAKE3(INTERNAL_DOMAIN ‖ left ‖ right)`.
fn hash_internal(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&[crate::event::INTERNAL_DOMAIN]);
    hasher.update(left);
    hasher.update(right);
    *hasher.finalize().as_bytes()
}

/// The Merkle root over a list of event digests.
///
/// Public because an auditor who has a set of digests from elsewhere — a
/// checkpoint file, another node, a proof bundle — must be able to compute the
/// same root this crate computes. Odd levels are completed by pairing the last
/// node with itself; an empty list yields [`MerkleRoot::empty`].
pub fn merkle_root(leaves: &[[u8; 32]]) -> MerkleRoot {
    if leaves.is_empty() {
        return MerkleRoot::empty();
    }

    let mut level: Vec<[u8; 32]> = leaves.iter().map(hash_leaf).collect();

    while level.len() > 1 {
        let mut next = Vec::with_capacity(level.len().div_ceil(2));

        let mut index = 0usize;
        while index < level.len() {
            // `index < level.len()` makes both lookups succeed; the `None` arms
            // exist so that no indexing panic can occur even if that reasoning
            // is ever broken by a later edit.
            let left = match level.get(index) {
                Some(left) => *left,
                None => break,
            };
            let right = match level.get(index + 1) {
                Some(right) => *right,
                None => left,
            };
            next.push(hash_internal(&left, &right));
            index += 2;
        }

        level = next;
    }

    match level.as_slice() {
        [only] => MerkleRoot(*only),
        _ => MerkleRoot::empty(),
    }
}

/// One recorded event.
///
/// `PartialEq` but not `Eq`: an event's payload is a `serde_json::Value`, and
/// `Value` is not `Eq` (its numbers are floats).
#[derive(Debug, Clone, PartialEq)]
pub struct LogEntry {
    seq: SequenceNumber,
    digest: [u8; 32],
    event: Arc<Event>,
}

impl LogEntry {
    /// Position in the log.
    pub fn sequence(&self) -> SequenceNumber {
        self.seq
    }

    /// The event's content digest, cached at append time and re-checkable with
    /// [`EventLog::verify_integrity`].
    pub fn digest(&self) -> [u8; 32] {
        self.digest
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
}

/// A point in history: "at sequence `seq` the root was `root`".
///
/// A checkpoint is what makes replay detectable. Once a checkpoint exists for
/// sequence `n`, sequences `1..=n` are fenced: history at or below it is no
/// longer "new" for anyone ([`EventLog::admits_as_new`]), and a log presented as
/// authoritative must reproduce the recorded root for that prefix
/// ([`EventLog::verify_against`]) or be refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Checkpoint {
    seq: SequenceNumber,
    root: MerkleRoot,
}

impl Checkpoint {
    /// Record an explicit point.
    pub fn new(seq: SequenceNumber, root: MerkleRoot) -> Self {
        Self { seq, root }
    }

    /// The sequence this checkpoint covers ([`SequenceNumber::GENESIS`] for an
    /// empty log).
    pub fn sequence(&self) -> SequenceNumber {
        self.seq
    }

    /// The Merkle root of the log's prefix up to [`Checkpoint::sequence`].
    pub fn root(&self) -> MerkleRoot {
        self.root
    }

    /// True for a checkpoint taken on an empty log.
    pub fn is_genesis(&self) -> bool {
        self.seq.is_genesis()
    }

    /// True when `seq` is at or below the fence — i.e. already history.
    pub fn fences(&self, seq: SequenceNumber) -> bool {
        !seq.is_genesis() && seq <= self.seq
    }
}

impl std::fmt::Display for Checkpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Checkpoint(seq={}, root={})", self.seq, self.root.to_hex())
    }
}

/// An append-only, in-memory, Merkle-rooted log of events.
///
/// The log assigns sequence numbers, caches each event's digest, and can
/// recompute its root, replay a range, and prove (or refuse) that it agrees with
/// a [`Checkpoint`]. It is the storage layer; [`crate::bus::EventBus`] is the
/// enforcement layer. `EventLog` never accepts an event from a caller directly —
/// [`EventLog::restore`] is the only insertion path that is public, and it
/// re-validates every event and recomputes every digest, because a log arriving
/// from elsewhere is exactly the input T-13 tells you not to trust.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EventLog {
    entries: Vec<LogEntry>,
    event_ids: HashSet<String>,
}

impl EventLog {
    /// An empty log.
    pub fn new() -> Self {
        Self::default()
    }

    /// Rebuild a log from events, in the order presented.
    ///
    /// Every event is validated and every digest recomputed from the event's own
    /// bytes, so the resulting log cannot be "pre-tampered": whatever the source
    /// claimed, the digests and the root describe what the bytes actually are.
    /// Duplicate `event_id`s are refused.
    ///
    /// # Errors
    ///
    /// [`BusError::DuplicateEventId`] if two events share an `event_id`;
    /// [`BusError::MissingMeta`] / [`BusError::InvalidId`] / … if an event fails
    /// [`Event::validate`].
    pub fn restore(events: Vec<Arc<Event>>) -> BusResult<Self> {
        let mut log = Self::new();
        for event in events {
            event.validate()?;

            let event_id = event.event_id().as_str();
            if !log.event_ids.insert(event_id.to_string()) {
                return Err(BusError::DuplicateEventId {
                    event_id: event_id.to_string(),
                });
            }

            let seq = log.next_sequence();
            log.entries.push(LogEntry {
                seq,
                digest: event.digest(),
                event,
            });
        }
        Ok(log)
    }

    /// Number of recorded events.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// True when nothing has been appended.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The highest assigned sequence number, or `None` for an empty log.
    pub fn head(&self) -> Option<SequenceNumber> {
        self.entries.last().map(|entry| entry.seq)
    }

    /// The sequence number the next appended event will receive.
    pub fn next_sequence(&self) -> SequenceNumber {
        SequenceNumber::new(self.entries.len() as u64).next()
    }

    /// Every entry, in sequence order.
    pub fn entries(&self) -> &[LogEntry] {
        &self.entries
    }

    /// The entry at `seq`, if it exists.
    pub fn entry(&self, seq: SequenceNumber) -> Option<&LogEntry> {
        if seq.is_genesis() {
            return None;
        }
        let index = (seq.get() - 1) as usize;
        self.entries.get(index)
    }

    /// True when this log has already accepted `event_id`.
    pub fn contains_event_id(&self, event_id: &str) -> bool {
        self.event_ids.contains(event_id)
    }

    /// Replay from `from` (inclusive).
    ///
    /// [`SequenceNumber::GENESIS`] is accepted and means "from the beginning",
    /// which makes `replay(GENESIS)` and `replay(FIRST)` the same call. A `from`
    /// of `head + 1` is legal and yields nothing — "anything new since the head"
    /// is a question with an empty answer, not an error. Beyond that,
    /// [`BusError::ReplayOutOfRange`].
    pub fn replay(&self, from: SequenceNumber) -> BusResult<Vec<Arc<Event>>> {
        let head = self.head().map(|head| head.get()).unwrap_or(0);
        let from_raw = from.get();

        if from_raw > head.saturating_add(1) {
            return Err(BusError::ReplayOutOfRange {
                from_seq: from_raw,
                head,
            });
        }

        let start = if from_raw == 0 { 1 } else { from_raw };
        let skip = (start - 1) as usize;

        Ok(self
            .entries
            .iter()
            .skip(skip)
            .map(|entry| Arc::clone(&entry.event))
            .collect())
    }

    /// Every event, in sequence order.
    pub fn events(&self) -> Vec<Arc<Event>> {
        self.entries
            .iter()
            .map(|entry| Arc::clone(&entry.event))
            .collect()
    }

    /// The Merkle root over the first `seq` events.
    ///
    /// # Errors
    ///
    /// [`BusError::ReplayOutOfRange`] when `seq` is beyond the head.
    pub fn root_at(&self, seq: SequenceNumber) -> BusResult<MerkleRoot> {
        let count = seq.get();
        let head = self.entries.len() as u64;

        if count > head {
            return Err(BusError::ReplayOutOfRange {
                from_seq: count,
                head,
            });
        }

        let slice = match self.entries.get(..count as usize) {
            Some(slice) => slice,
            None => {
                // Unreachable: `count <= head == entries.len()`. Returned rather
                // than indexed so that no panic exists on this path.
                return Err(BusError::ReplayOutOfRange {
                    from_seq: count,
                    head,
                });
            }
        };

        let leaves: Vec<[u8; 32]> = slice.iter().map(|entry| entry.digest).collect();
        Ok(merkle_root(&leaves))
    }

    /// The Merkle root over the whole log ([`MerkleRoot::empty`] when empty).
    pub fn current_root(&self) -> MerkleRoot {
        let leaves: Vec<[u8; 32]> = self.entries.iter().map(|entry| entry.digest).collect();
        merkle_root(&leaves)
    }

    /// A checkpoint at the head of the log.
    pub fn checkpoint(&self) -> Checkpoint {
        Checkpoint::new(
            self.head().unwrap_or(SequenceNumber::GENESIS),
            self.current_root(),
        )
    }

    /// A checkpoint at `seq`.
    ///
    /// # Errors
    ///
    /// [`BusError::ReplayOutOfRange`] when `seq` is beyond the head.
    pub fn checkpoint_at(&self, seq: SequenceNumber) -> BusResult<Checkpoint> {
        Ok(Checkpoint::new(seq, self.root_at(seq)?))
    }

    /// Check this log against a checkpoint's fenced prefix.
    ///
    /// # Errors
    ///
    /// [`BusError::ReplayOutOfRange`] when the log does not even reach the
    /// checkpointed sequence — a stream that stops before the fence says nothing
    /// about the fenced range and is refused rather than trusted;
    /// [`BusError::CheckpointMismatch`] when the prefix root differs.
    pub fn verify_against(&self, checkpoint: &Checkpoint) -> BusResult<()> {
        let fence = checkpoint.sequence().get();

        if fence == 0 {
            // A genesis checkpoint asserts "nothing had been recorded yet".
            if self.entries.is_empty() {
                return Ok(());
            }
            return Err(BusError::CheckpointMismatch {
                checkpoint_seq: 0,
                expected: checkpoint.root().to_hex(),
                observed: self.current_root().to_hex(),
            });
        }

        let observed = self.root_at(checkpoint.sequence())?;
        if observed == checkpoint.root() {
            Ok(())
        } else {
            Err(BusError::CheckpointMismatch {
                checkpoint_seq: fence,
                expected: checkpoint.root().to_hex(),
                observed: observed.to_hex(),
            })
        }
    }

    /// The T-13 gate: may `seq` be accepted as *new*?
    ///
    /// Returns [`BusError::FencedSequence`] when `seq` is at or below the
    /// checkpoint's fence. Reading one's own log ([`EventLog::replay`]) is not
    /// affected: this gate is for accepting events from an untrusted source,
    /// where re-presenting already-checkpointed history is the attack.
    pub fn admits_as_new(
        &self,
        seq: SequenceNumber,
        checkpoint: &Checkpoint,
    ) -> BusResult<()> {
        if checkpoint.fences(seq) {
            return Err(BusError::FencedSequence {
                seq: seq.get(),
                fence: checkpoint.sequence().get(),
            });
        }
        Ok(())
    }

    /// The events strictly after a checkpoint — the part that is genuinely new.
    ///
    /// # Errors
    ///
    /// [`BusError::ReplayOutOfRange`] if the checkpoint is ahead of this log.
    pub fn new_events_since(
        &self,
        checkpoint: &Checkpoint,
    ) -> BusResult<Vec<Arc<Event>>> {
        self.replay(checkpoint.sequence().next())
    }

    /// Re-derive every digest from the event bytes and compare.
    ///
    /// [`EventLog::restore`] recomputes digests, so on that path this can only
    /// confirm what restore already did. It exists for the in-process log, where
    /// digests are cached at append time, and for an audit that wants the claim
    /// re-checked after the fact rather than assumed.
    ///
    /// # Errors
    ///
    /// [`BusError::LogDiscontinuity`] on a sequence gap;
    /// [`BusError::LogIntegrity`] when a digest disagrees, an event fails
    /// [`Event::validate`], or the identifier index disagrees with the entries.
    pub fn verify_integrity(&self) -> BusResult<()> {
        if self.event_ids.len() != self.entries.len() {
            return Err(BusError::LogIntegrity {
                seq: 0,
                reason: format!(
                    "identifier index holds {} ids for {} entries",
                    self.event_ids.len(),
                    self.entries.len()
                ),
            });
        }

        for (index, entry) in self.entries.iter().enumerate() {
            let expected = SequenceNumber::new(index as u64).next();
            if entry.seq != expected {
                return Err(BusError::LogDiscontinuity {
                    seq: entry.seq.get(),
                    expected: expected.get(),
                });
            }

            entry
                .event
                .validate()
                .map_err(|e| BusError::LogIntegrity {
                    seq: entry.seq.get(),
                    reason: format!("event is no longer valid: {e}"),
                })?;

            let recomputed = *blake3::hash(entry.event.signing_bytes()).as_bytes();
            if recomputed != entry.digest {
                return Err(BusError::LogIntegrity {
                    seq: entry.seq.get(),
                    reason: format!(
                        "cached digest {} disagrees with the digest recomputed from the event {}",
                        hex::encode(entry.digest),
                        hex::encode(recomputed)
                    ),
                });
            }
        }

        Ok(())
    }

    /// Append an event. Internal to the crate: [`crate::bus::EventBus::publish`]
    /// applies the publish-time gates (meta validation, signature policy,
    /// `event_id` uniqueness) before calling this, and exposing it would let a
    /// caller bypass all three.
    pub(crate) fn append(&mut self, event: Arc<Event>) -> SequenceNumber {
        let seq = self.next_sequence();
        let digest = event.digest();

        self.event_ids.insert(event.event_id().as_str().to_string());
        self.entries.push(LogEntry {
            seq,
            digest,
            event,
        });

        seq
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meta::{CorrelationId, EventMeta, SchemaVersion, TraceId};
    use crate::topic::STORAGE_STORED;
    use serde_json::json;

    fn event(event_id: &str, payload: i64) -> Arc<Event> {
        let meta = EventMeta::root(
            STORAGE_STORED,
            SchemaVersion::V1,
            crate::meta::EventId::new(event_id).expect("valid"),
            TraceId::new("trc_log").expect("valid"),
            CorrelationId::new("cor_log").expect("valid"),
            7,
        );
        Arc::new(Event::new(meta, json!({ "n": payload })).expect("valid event"))
    }

    fn log(count: usize) -> EventLog {
        let events: Vec<Arc<Event>> = (0..count)
            .map(|n| event(&format!("evt_{n}"), n as i64))
            .collect();
        EventLog::restore(events).expect("valid log")
    }

    #[test]
    fn sequence_numbers_start_at_one_and_step_by_one() {
        let mut log = EventLog::new();
        assert!(log.is_empty());
        assert_eq!(log.head(), None);
        assert_eq!(log.next_sequence(), SequenceNumber::FIRST);
        assert!(SequenceNumber::GENESIS.is_genesis());

        for n in 1..=5u64 {
            let seq = log.append(event(&format!("evt_{n}"), 0));
            assert_eq!(seq.get(), n);
        }
        assert_eq!(log.head(), Some(SequenceNumber::new(5)));
        assert_eq!(log.next_sequence().get(), 6);
        assert_eq!(SequenceNumber::new(u64::MAX).next().get(), u64::MAX);
    }

    #[test]
    fn restore_preserves_order_and_recomputes_digests() {
        let log = log(3);
        assert_eq!(log.len(), 3);
        let ids: Vec<&str> = log
            .entries()
            .iter()
            .map(|entry| entry.event_id().as_str())
            .collect();
        assert_eq!(ids, ["evt_0", "evt_1", "evt_2"]);

        for entry in log.entries() {
            assert_eq!(
                entry.digest(),
                *blake3::hash(entry.event().signing_bytes()).as_bytes()
            );
            assert_eq!(entry.topic().as_str(), "storage.stored");
        }
        assert!(log.verify_integrity().is_ok());
    }

    #[test]
    fn restore_refuses_duplicate_identifiers() {
        // `restore` also calls `Event::validate` on every event; that arm is
        // unreachable today because `Event`'s constructors are the only way to
        // build one and they validate. The *reachable* refusal is uniqueness.
        let duplicated = vec![event("evt_same", 1), event("evt_same", 2)];
        match EventLog::restore(duplicated) {
            Err(BusError::DuplicateEventId { event_id }) => assert_eq!(event_id, "evt_same"),
            other => panic!("expected DuplicateEventId, got {other:?}"),
        }
    }

    #[test]
    fn replay_is_inclusive_and_genesis_equals_first() {
        let log = log(4);
        let from_two = log.replay(SequenceNumber::new(2)).expect("in range");
        assert_eq!(from_two.len(), 3);
        assert_eq!(from_two[0].event_id().as_str(), "evt_1");

        assert_eq!(
            log.replay(SequenceNumber::GENESIS).expect("in range").len(),
            log.replay(SequenceNumber::FIRST).expect("in range").len()
        );

        // head + 1 is legal and empty.
        assert!(log
            .replay(SequenceNumber::new(5))
            .expect("head + 1 is legal")
            .is_empty());

        match log.replay(SequenceNumber::new(6)) {
            Err(BusError::ReplayOutOfRange { from_seq, head }) => {
                assert_eq!((from_seq, head), (6, 4));
            }
            other => panic!("expected ReplayOutOfRange, got {other:?}"),
        }
    }

    #[test]
    fn empty_log_replay_and_root_behave() {
        let log = EventLog::new();
        assert!(log.replay(SequenceNumber::FIRST).expect("ok").is_empty());
        assert_eq!(log.current_root(), MerkleRoot::empty());
        assert_eq!(merkle_root(&[]), MerkleRoot::empty());
        assert!(log.checkpoint().is_genesis());
        assert!(log.root_at(SequenceNumber::GENESIS).is_ok());
        assert!(log.root_at(SequenceNumber::FIRST).is_err());
    }

    #[test]
    fn root_is_stable_for_the_same_log_and_moves_for_different_content() {
        let a = log(4);
        let b = log(4);
        assert_eq!(a.current_root(), b.current_root());
        assert_eq!(a.current_root().to_hex().len(), 64);

        let c = EventLog::restore(vec![
            event("evt_0", 0),
            event("evt_1", 99), // one payload byte different
            event("evt_2", 2),
            event("evt_3", 3),
        ])
        .expect("valid");
        assert_ne!(a.current_root(), c.current_root());
    }

    #[test]
    fn root_is_position_sensitive_and_length_sensitive() {
        let four = log(4);
        let swapped = EventLog::restore(vec![
            event("evt_1", 1),
            event("evt_0", 0),
            event("evt_2", 2),
            event("evt_3", 3),
        ])
        .expect("valid");
        assert_ne!(four.current_root(), swapped.current_root());

        assert_ne!(log(3).current_root(), log(4).current_root());
        assert_ne!(log(1).current_root(), MerkleRoot::empty());
    }

    #[test]
    fn merkle_root_of_explicit_leaves_tracks_leaf_content() {
        // The auditor's path: given digests from elsewhere, recompute the root.
        let log = log(3);
        let leaves: Vec<[u8; 32]> = log.entries().iter().map(|entry| entry.digest()).collect();
        assert_eq!(merkle_root(&leaves), log.current_root());

        let mut tampered = leaves.clone();
        tampered[1] = [0xffu8; 32];
        assert_ne!(merkle_root(&tampered), log.current_root());

        assert_eq!(merkle_root(&[]), MerkleRoot::empty());
        assert_ne!(merkle_root(&[leaves[0]]), MerkleRoot::empty());
    }

    #[test]
    fn root_at_prefixes_differ_from_each_other() {
        let log = log(4);
        let roots: Vec<MerkleRoot> = (0..=4)
            .map(|n| log.root_at(SequenceNumber::new(n)).expect("in range"))
            .collect();
        for (i, root) in roots.iter().enumerate() {
            assert_eq!(*root, log.checkpoint_at(SequenceNumber::new(i as u64)).expect("ok").root());
        }
        for i in 0..roots.len() {
            for j in (i + 1)..roots.len() {
                assert_ne!(roots[i], roots[j], "prefixes {i} and {j} must differ");
            }
        }
    }

    #[test]
    fn checkpoint_verifies_a_matching_log_and_refuses_a_rewritten_prefix() {
        let original = log(4);
        let checkpoint = original.checkpoint_at(SequenceNumber::new(2)).expect("ok");
        assert_eq!(checkpoint.sequence().get(), 2);
        assert!(original.verify_against(&checkpoint).is_ok());

        let rewritten = EventLog::restore(vec![
            event("evt_0", 0),
            event("evt_1", 1234), // history rewritten
            event("evt_2", 2),
            event("evt_3", 3),
        ])
        .expect("valid");

        match rewritten.verify_against(&checkpoint) {
            Err(BusError::CheckpointMismatch {
                checkpoint_seq,
                expected,
                observed,
            }) => {
                assert_eq!(checkpoint_seq, 2);
                assert_eq!(expected, checkpoint.root().to_hex());
                assert_ne!(expected, observed);
            }
            other => panic!("expected CheckpointMismatch, got {other:?}"),
        }
    }

    #[test]
    fn checkpoint_refuses_a_stream_that_does_not_reach_the_fence() {
        let short = log(1);
        let checkpoint = log(4).checkpoint_at(SequenceNumber::new(4)).expect("ok");
        match short.verify_against(&checkpoint) {
            Err(BusError::ReplayOutOfRange { from_seq, head }) => {
                assert_eq!((from_seq, head), (4, 1));
            }
            other => panic!("expected ReplayOutOfRange, got {other:?}"),
        }
    }

    #[test]
    fn fencing_rejects_sequences_at_or_below_the_checkpoint() {
        // A genesis checkpoint records "nothing yet", so it fences no sequence.
        let genesis = EventLog::new().checkpoint();
        assert!(genesis.is_genesis());
        assert!(!genesis.fences(SequenceNumber::FIRST));

        let log = log(4);
        let checkpoint = log.checkpoint_at(SequenceNumber::new(3)).expect("ok");
        assert!(checkpoint.fences(SequenceNumber::new(1)));
        assert!(checkpoint.fences(SequenceNumber::new(3)));
        assert!(!checkpoint.fences(SequenceNumber::new(4)));
        assert!(
            !checkpoint.fences(SequenceNumber::GENESIS),
            "genesis is a position, not an event, so it is never fenced"
        );

        assert!(log.admits_as_new(SequenceNumber::new(4), &checkpoint).is_ok());
        match log.admits_as_new(SequenceNumber::new(3), &checkpoint) {
            Err(BusError::FencedSequence { seq, fence }) => assert_eq!((seq, fence), (3, 3)),
            other => panic!("expected FencedSequence, got {other:?}"),
        }
        assert!(log
            .admits_as_new(SequenceNumber::new(1), &checkpoint)
            .is_err());
    }

    #[test]
    fn new_events_since_a_checkpoint_are_strictly_after_it() {
        let log = log(5);
        let checkpoint = log.checkpoint_at(SequenceNumber::new(2)).expect("ok");

        let new = log.new_events_since(&checkpoint).expect("ok");
        assert_eq!(new.len(), 3);
        assert_eq!(new[0].event_id().as_str(), "evt_2");
        assert_eq!(new[2].event_id().as_str(), "evt_4");

        let at_head = log.checkpoint();
        assert!(log.new_events_since(&at_head).expect("ok").is_empty());
    }

    #[test]
    fn a_genesis_checkpoint_only_matches_an_empty_log() {
        let genesis = EventLog::new().checkpoint();
        assert!(EventLog::new().verify_against(&genesis).is_ok());
        match log(1).verify_against(&genesis) {
            Err(BusError::CheckpointMismatch { checkpoint_seq, .. }) => {
                assert_eq!(checkpoint_seq, 0);
            }
            other => panic!("expected CheckpointMismatch, got {other:?}"),
        }
    }

    #[test]
    fn entry_lookup_and_contains_event_id() {
        let log = log(3);
        assert!(log.entry(SequenceNumber::GENESIS).is_none());
        assert!(log.entry(SequenceNumber::new(9)).is_none());
        let entry = log.entry(SequenceNumber::new(2)).expect("present");
        assert_eq!(entry.sequence().get(), 2);
        assert_eq!(entry.event_id().as_str(), "evt_1");
        assert_eq!(
            log.entry(SequenceNumber::new(3)).map(|entry| entry.event_id().as_str()),
            Some("evt_2")
        );

        assert!(log.contains_event_id("evt_0"));
        assert!(!log.contains_event_id("evt_9"));
    }

    #[test]
    fn verify_integrity_reports_a_discontinuity() {
        let mut log = log(3);
        // Force a gap by overwriting a sequence number, the way memory
        // corruption or a bad restore would.
        log.entries[1].seq = SequenceNumber::new(99);
        match log.verify_integrity() {
            Err(BusError::LogDiscontinuity { seq, expected }) => {
                assert_eq!((seq, expected), (99, 2));
            }
            other => panic!("expected LogDiscontinuity, got {other:?}"),
        }
    }

    #[test]
    fn verify_integrity_reports_a_stale_digest() {
        let mut log = log(2);
        log.entries[0].digest = [0u8; 32];
        match log.verify_integrity() {
            Err(BusError::LogIntegrity { seq, reason }) => {
                assert_eq!(seq, 1);
                assert!(reason.contains("disagrees"), "{reason}");
            }
            other => panic!("expected LogIntegrity, got {other:?}"),
        }
    }

    #[test]
    fn verify_integrity_reports_an_index_disagreement() {
        let mut log = log(2);
        log.event_ids.clear();
        match log.verify_integrity() {
            Err(BusError::LogIntegrity { reason, .. }) => {
                assert!(reason.contains("identifier index"), "{reason}");
            }
            other => panic!("expected LogIntegrity, got {other:?}"),
        }
    }

    #[test]
    fn root_serialises_as_hex_and_rejects_bad_input() {
        let root = log(2).current_root();
        let json = serde_json::to_string(&root).expect("serializes");
        assert_eq!(json.len(), 2 + 64);
        assert_eq!(MerkleRoot::from_hex(&root.to_hex()).expect("ok"), root);
        assert!(MerkleRoot::from_hex("00").is_err());
        assert!(MerkleRoot::from_hex("zz").is_err());
        assert_eq!(serde_json::from_str::<MerkleRoot>(&json).expect("ok"), root);
        assert_eq!(format!("{root}"), root.to_hex());
        assert_eq!(format!("{root:?}"), format!("MerkleRoot({})", root.to_hex()));
        assert_eq!(MerkleRoot::LEN, 32);
        assert_eq!(root.as_bytes().len(), 32);
        assert_eq!(root.to_array(), *root.as_bytes());
    }

    #[test]
    fn checkpoint_displays_sequence_and_root() {
        let checkpoint = log(1).checkpoint();
        let rendered = checkpoint.to_string();
        assert!(rendered.starts_with("Checkpoint(seq=1, root="), "{rendered}");
    }

    #[test]
    fn a_snapshot_is_a_copy() {
        let mut log = log(2);
        let snapshot = log.clone();
        log.append(event("evt_new", 9));
        assert_eq!(snapshot.len(), 2);
        assert_eq!(log.len(), 3);
        assert_ne!(snapshot.current_root(), log.current_root());
    }
}
