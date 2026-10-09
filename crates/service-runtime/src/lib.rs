//! Shared no-std checkpoint/replay engine for one reviewed Wasm fixture.
//!
//! This is not a general Wasm snapshotter. The fixed
//! fixture has one memory and one mutable global, and snapshots are taken only
//! after handlers return. No interpreter stack or live host resources are saved.
//! Synthetic effects are compared in memory; nothing is sent to an external peer.
#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;
use alloc::{
    borrow::ToOwned,
    format,
    string::{String, ToString},
    vec::Vec,
};

use core::fmt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use wasmi::{
    Caller, CompilationMode, Config, Engine, Global, Linker, Memory, Module, Store, StoreLimits,
    StoreLimitsBuilder, TypedFunc, Val,
};

#[cfg(test)]
const WORKLOAD: &str = include_str!("../../../workloads/kv/service.wat");
const WORKLOAD_BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/kv-service.wasm"));
const MEMORY_BYTES: usize = 65_536;
const MAX_EVENTS: usize = 4_096;
const FUEL_PER_REQUEST: u64 = 10_000;
const SCHEMA: u32 = 1;
// Bump this when the engine version, validation, host ABI, metering, or limits change.
const PROFILE: &str = "mnemosyne/kv-v1/wasmi-2.0.0/integer/fixed64k/fuel10000/portable";

pub type Result<T> = core::result::Result<T, LabError>;

#[derive(Debug)]
pub enum LabError {
    Contract(&'static str),
    Trace(String),
    Wasm(wasmi::Error),
}

impl fmt::Display for LabError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Contract(message) => f.write_str(message),
            Self::Trace(message) => f.write_str(message),
            Self::Wasm(error) => write!(f, "Wasm execution: {error}"),
        }
    }
}

impl core::error::Error for LabError {}

impl From<wasmi::Error> for LabError {
    fn from(error: wasmi::Error) -> Self {
        Self::Wasm(error)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Request {
    pub key: i32,
    pub delta: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Effect {
    pub key: i32,
    pub value: i64,
    pub version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum Event {
    Request { request: Request },
    Clock { value: i64 },
    Random { bound: i32, value: i64 },
    Emit { effect: Effect },
    Reply { value: i64 },
}

/// A corruption-detecting image, not an authenticated or durable checkpoint.
/// Fields are private because only this fixed fixture's state schema is supported.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    schema: u32,
    profile: String,
    module: [u8; 32],
    memory: Vec<u8>,
    version: i64,
    digest: [u8; 32],
}

impl Snapshot {
    fn hash(&self) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(b"mnemosyne-snapshot\0");
        hash.update(self.schema.to_le_bytes());
        hash.update((self.profile.len() as u64).to_le_bytes());
        hash.update(self.profile.as_bytes());
        hash.update(self.module);
        hash.update((self.memory.len() as u64).to_le_bytes());
        hash.update(&self.memory);
        hash.update(self.version.to_le_bytes());
        hash.finalize().into()
    }

    fn validate(&self, module: [u8; 32]) -> Result<()> {
        if self.schema != SCHEMA || self.profile != PROFILE || self.module != module {
            return Err(LabError::Contract(
                "checkpoint schema, module, or profile mismatch",
            ));
        }
        if self.memory.len() != MEMORY_BYTES || self.digest != self.hash() {
            return Err(LabError::Contract("checkpoint size or checksum mismatch"));
        }
        Ok(())
    }

    pub fn digest_hex(&self) -> String {
        hex(&self.digest)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayLog {
    schema: u32,
    profile: String,
    module: [u8; 32],
    checkpoint: [u8; 32],
    events: Vec<Event>,
}

impl ReplayLog {
    fn validate(&self, snapshot: &Snapshot) -> Result<()> {
        if self.schema != SCHEMA
            || self.profile != PROFILE
            || self.module != snapshot.module
            || self.checkpoint != snapshot.digest
        {
            return Err(LabError::Contract(
                "trace is not bound to this checkpoint/profile",
            ));
        }
        if self.events.len() > MAX_EVENTS {
            return Err(LabError::Contract("trace event bound exceeded"));
        }
        Ok(())
    }
}

enum Mode {
    Record {
        anchor: [u8; 32],
        events: Vec<Event>,
    },
    Replay {
        events: Vec<Event>,
        cursor: usize,
    },
}

struct Host {
    limits: StoreLimits,
    mode: Mode,
    clock: i64,
    random: u64,
    pending: Vec<Effect>,
    observed: Vec<Effect>,
}

impl Host {
    fn new(mode: Mode, seed: u64) -> Self {
        Self {
            limits: limits(),
            mode,
            clock: seed as i64,
            random: seed,
            pending: Vec::new(),
            observed: Vec::new(),
        }
    }

    fn is_replay(&self) -> bool {
        matches!(self.mode, Mode::Replay { .. })
    }

    fn append(&mut self, event: Event) -> Result<()> {
        let Mode::Record { events, .. } = &mut self.mode else {
            return Err(LabError::Contract("cannot append during replay"));
        };
        if events.len() == MAX_EVENTS {
            return Err(LabError::Contract("trace event bound exceeded"));
        }
        events.push(event);
        Ok(())
    }

    fn next(&mut self) -> Result<Event> {
        let Mode::Replay { events, cursor } = &mut self.mode else {
            return Err(LabError::Contract("cannot consume during recording"));
        };
        let event = events
            .get(*cursor)
            .ok_or_else(|| LabError::Trace(format!("trace ended at event {cursor}")))?
            .clone();
        *cursor += 1;
        Ok(event)
    }

    fn observe(&mut self, expected: Event) -> Result<()> {
        if self.is_replay() {
            let actual = self.next()?;
            if actual != expected {
                return Err(LabError::Trace(format!(
                    "trace mismatch: expected {expected:?}, got {actual:?}"
                )));
            }
            Ok(())
        } else {
            self.append(expected)
        }
    }

    fn clock(&mut self) -> Result<i64> {
        if self.is_replay() {
            return match self.next()? {
                Event::Clock { value } => Ok(value),
                event => Err(LabError::Trace(format!("expected clock, got {event:?}"))),
            };
        }
        // Controlled synthetic nondeterminism for the experiment, not wall time.
        self.clock = self.clock.wrapping_add(17);
        self.append(Event::Clock { value: self.clock })?;
        Ok(self.clock)
    }

    fn random(&mut self, bound: i32) -> Result<i64> {
        if bound <= 0 {
            return Err(LabError::Contract("random bound must be positive"));
        }
        if self.is_replay() {
            return match self.next()? {
                Event::Random {
                    bound: recorded,
                    value,
                } if recorded == bound && (0..i64::from(bound)).contains(&value) => Ok(value),
                event => Err(LabError::Trace(format!(
                    "expected random({bound}), got {event:?}"
                ))),
            };
        }
        // Test PRNG only; this is deliberately not a cryptographic entropy source.
        self.random = self
            .random
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        let value = ((self.random >> 32) % bound as u64) as i64;
        self.append(Event::Random { bound, value })?;
        Ok(value)
    }

    fn emit(&mut self, effect: Effect) -> Result<()> {
        self.observe(Event::Emit { effect })?;
        // Only publish to the in-memory observation list after a successful reply.
        self.pending.push(effect);
        Ok(())
    }
}

fn limits() -> StoreLimits {
    StoreLimitsBuilder::new()
        .memory_size(MEMORY_BYTES)
        .memories(1)
        .instances(1)
        .tables(0)
        .table_elements(0)
        .trap_on_grow_failure(true)
        .build()
}

fn engine() -> Engine {
    let mut config = Config::default();
    config
        .consume_fuel(true)
        .compilation_mode(CompilationMode::Eager)
        .allow_start_fn(false)
        .floats(false)
        .wasm_sign_extension(false)
        .wasm_saturating_float_to_int(false)
        .wasm_multi_value(false)
        .wasm_multi_memory(false)
        .wasm_bulk_memory(false)
        .wasm_reference_types(false)
        .wasm_tail_call(false)
        .wasm_extended_const(false)
        .wasm_custom_page_sizes(false)
        .wasm_wide_arithmetic(false)
        .set_max_recursion_depth(32)
        .set_min_stack_height(1_024)
        .set_max_stack_height(65_536)
        .set_max_cached_stacks(1);
    Engine::new(&config)
}

fn host_error(error: LabError) -> wasmi::Error {
    wasmi::Error::new(error.to_string())
}

/// A disposable instance of the bundled KV fixture. Any failed call poisons it;
/// recovery must construct a fresh instance from a known checkpoint.
pub struct Worker {
    store: Store<Host>,
    memory: Memory,
    version: Global,
    apply: TypedFunc<(i32, i64), i64>,
    module: [u8; 32],
    healthy: bool,
}

impl Worker {
    pub fn record(seed: u64) -> Result<Self> {
        let mut worker = Self::instantiate(
            WORKLOAD_BYTES,
            Mode::Record {
                anchor: [0; 32],
                events: Vec::new(),
            },
            seed,
        )?;
        worker.start_recording()?;
        Ok(worker)
    }

    /// Restores only the fixed, compiled-in fixture. This API does not accept
    /// arbitrary modules with unexported globals, tables, or other hidden state.
    pub fn replay(snapshot: &Snapshot, log: &ReplayLog, seed: u64) -> Result<Self> {
        let bytes = WORKLOAD_BYTES;
        let module = Sha256::digest(bytes).into();
        snapshot.validate(module)?;
        log.validate(snapshot)?;
        let mut worker = Self::instantiate(
            bytes,
            Mode::Replay {
                events: log.events.clone(),
                cursor: 0,
            },
            seed,
        )?;
        worker
            .memory
            .data_mut(&mut worker.store)
            .copy_from_slice(&snapshot.memory);
        worker
            .version
            .set(&mut worker.store, Val::I64(snapshot.version))
            .map_err(|_| LabError::Contract("invalid version global"))?;
        Ok(worker)
    }

    fn instantiate(bytes: &[u8], mode: Mode, seed: u64) -> Result<Self> {
        let engine = engine();
        let module = Module::new(&engine, bytes)?;
        let mut store = Store::new(&engine, Host::new(mode, seed));
        store.limiter(|host| &mut host.limits);
        let mut linker = Linker::new(&engine);
        linker
            .func_wrap("env", "clock", |mut caller: Caller<Host>| {
                caller.data_mut().clock().map_err(host_error)
            })
            .map_err(wasmi::Error::from)?;
        linker
            .func_wrap("env", "random", |mut caller: Caller<Host>, bound: i32| {
                caller.data_mut().random(bound).map_err(host_error)
            })
            .map_err(wasmi::Error::from)?;
        linker
            .func_wrap(
                "env",
                "emit",
                |mut caller: Caller<Host>, key: i32, value: i64, version: i64| {
                    caller
                        .data_mut()
                        .emit(Effect {
                            key,
                            value,
                            version,
                        })
                        .map_err(host_error)
                },
            )
            .map_err(wasmi::Error::from)?;
        let instance = linker.instantiate_and_start(&mut store, &module)?;
        let memory = instance
            .get_memory(&store, "memory")
            .ok_or(LabError::Contract("missing memory export"))?;
        let version = instance
            .get_global(&store, "version")
            .ok_or(LabError::Contract("missing version export"))?;
        let apply = instance.get_typed_func::<(i32, i64), i64>(&store, "apply")?;
        Ok(Self {
            store,
            memory,
            version,
            apply,
            module: Sha256::digest(bytes).into(),
            healthy: true,
        })
    }

    fn require_healthy(&self) -> Result<()> {
        if !self.healthy {
            return Err(LabError::Contract(
                "worker is poisoned; restore a checkpoint",
            ));
        }
        Ok(())
    }

    pub fn apply(&mut self, request: Request) -> Result<i64> {
        self.apply_budget(request, FUEL_PER_REQUEST)
    }

    fn apply_budget(&mut self, request: Request, fuel: u64) -> Result<i64> {
        self.require_healthy()?;
        self.healthy = false;
        let result = (|| {
            self.store.data_mut().observe(Event::Request { request })?;
            self.store.set_fuel(fuel)?;
            let value = self
                .apply
                .call(&mut self.store, (request.key, request.delta))?;
            self.store.data_mut().observe(Event::Reply { value })?;
            Ok(value)
        })();
        let host = self.store.data_mut();
        if result.is_ok() {
            host.observed.append(&mut host.pending);
            self.healthy = true;
        } else {
            host.pending.clear();
        }
        result
    }

    /// Copies all state in the fixture's schema at a completed-handler boundary.
    /// This is a full copy, not a COW checkpoint.
    pub fn snapshot(&self) -> Result<Snapshot> {
        self.require_healthy()?;
        let Val::I64(version) = self.version.get(&self.store) else {
            return Err(LabError::Contract("invalid version global"));
        };
        let mut snapshot = Snapshot {
            schema: SCHEMA,
            profile: PROFILE.to_owned(),
            module: self.module,
            memory: self.memory.data(&self.store).to_vec(),
            version,
            digest: [0; 32],
        };
        snapshot.digest = snapshot.hash();
        Ok(snapshot)
    }

    /// Starts a bounded log segment and returns the checkpoint it is bound to.
    pub fn start_recording(&mut self) -> Result<Snapshot> {
        self.require_healthy()?;
        if self.store.data().is_replay() {
            return Err(LabError::Contract(
                "cannot change replay into live execution",
            ));
        }
        let snapshot = self.snapshot()?;
        self.store.data_mut().mode = Mode::Record {
            anchor: snapshot.digest,
            events: Vec::new(),
        };
        self.store.data_mut().observed.clear();
        Ok(snapshot)
    }

    pub fn log(&self) -> Result<ReplayLog> {
        self.require_healthy()?;
        let Mode::Record { anchor, events } = &self.store.data().mode else {
            return Err(LabError::Contract("replay cannot produce a recording"));
        };
        Ok(ReplayLog {
            schema: SCHEMA,
            profile: PROFILE.to_owned(),
            module: self.module,
            checkpoint: *anchor,
            events: events.clone(),
        })
    }

    /// Must be called after all expected requests. An unread suffix is a mismatch.
    pub fn finish_replay(&mut self) -> Result<()> {
        self.require_healthy()?;
        match &self.store.data().mode {
            Mode::Replay { events, cursor } if *cursor == events.len() => Ok(()),
            Mode::Replay { .. } => {
                self.healthy = false;
                Err(LabError::Contract("unconsumed trace events"))
            }
            Mode::Record { .. } => Err(LabError::Contract("worker is not replaying")),
        }
    }

    pub fn observed_effects(&self) -> &[Effect] {
        &self.store.data().observed
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn replay_requests(snapshot: &Snapshot, log: &ReplayLog, requests: &[Request]) -> Result<Worker> {
    // Deliberately different from the live providers; replay must not consult them.
    let mut replay = Worker::replay(snapshot, log, 99_999)?;
    for request in requests {
        replay.apply(*request)?;
    }
    replay.finish_replay()?;
    Ok(replay)
}

#[derive(Debug, Serialize)]
pub struct DemoReport {
    pub passed: bool,
    profile: &'static str,
    scope: &'static str,
    replayed_requests: usize,
    trace_events: usize,
    snapshot_mutable_state_bytes: usize,
    module_sha256: String,
    checkpoint_sha256: String,
    final_state_sha256: String,
    replay_state_sha256: String,
    state_equal: bool,
    effects_equal: bool,
    missing_event_rejected: bool,
    reordered_event_rejected: bool,
    trailing_event_rejected: bool,
    corrupted_checkpoint_rejected: bool,
    /// Actual observed input/output sequence, exported for the host inspector.
    /// This does not change the checkpoint or replay acceptance contract.
    trace: Vec<Event>,
}

pub fn demo() -> Result<DemoReport> {
    let mut live = Worker::record(42)?;
    for key in 0..4 {
        live.apply(Request {
            key,
            delta: i64::from(key) * 3,
        })?;
    }
    let checkpoint = live.start_recording()?;
    let requests: Vec<_> = (0..12)
        .map(|index| Request {
            key: index % 7,
            delta: i64::from(index) - 5,
        })
        .collect();
    for request in &requests {
        live.apply(*request)?;
    }
    let final_state = live.snapshot()?;
    let log = live.log()?;
    let replay = replay_requests(&checkpoint, &log, &requests)?;
    let replay_state = replay.snapshot()?;

    let mut missing = log.clone();
    missing.events.remove(1);
    let mut reordered = log.clone();
    reordered.events.swap(1, 2);
    let mut trailing = log.clone();
    trailing.events.push(Event::Clock { value: 123 });
    let mut corrupted = checkpoint.clone();
    corrupted.memory[0] ^= 1;

    let mut report = DemoReport {
        passed: false,
        profile: PROFILE,
        scope: "fixed Wasmi fixture; synthetic inputs and in-memory effects; no external delivery",
        replayed_requests: requests.len(),
        trace_events: log.events.len(),
        snapshot_mutable_state_bytes: checkpoint.memory.len() + 8,
        module_sha256: hex(&checkpoint.module),
        checkpoint_sha256: checkpoint.digest_hex(),
        final_state_sha256: final_state.digest_hex(),
        replay_state_sha256: replay_state.digest_hex(),
        state_equal: final_state == replay_state,
        effects_equal: live.observed_effects() == replay.observed_effects(),
        missing_event_rejected: replay_requests(&checkpoint, &missing, &requests).is_err(),
        reordered_event_rejected: replay_requests(&checkpoint, &reordered, &requests).is_err(),
        trailing_event_rejected: replay_requests(&checkpoint, &trailing, &requests).is_err(),
        corrupted_checkpoint_rejected: Worker::replay(&corrupted, &log, 0).is_err(),
        trace: log.events.clone(),
    };
    report.passed = report.state_equal
        && report.effects_equal
        && report.missing_event_rejected
        && report.reordered_event_rejected
        && report.trailing_event_rejected
        && report.corrupted_checkpoint_rejected;
    Ok(report)
}

/// Durable serialization envelope for the fixed fixture. Filesystem integrity
/// protects bytes at rest; neither these bytes nor an AI plan grant authority.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct RecoveryBundle {
    checkpoint: Snapshot,
    log: ReplayLog,
    requests: Vec<Request>,
    expected_state: String,
    expected_effects: Vec<Effect>,
}
impl RecoveryBundle {
    pub fn checkpoint_sha256(&self) -> String {
        self.checkpoint.digest_hex()
    }
    pub fn event_count(&self) -> usize {
        self.log.events.len()
    }
    pub fn request_count(&self) -> usize {
        self.requests.len()
    }
    pub fn validate(&self) -> Result<()> {
        self.checkpoint
            .validate(Sha256::digest(WORKLOAD_BYTES).into())?;
        self.log.validate(&self.checkpoint)
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct RecoveryReport {
    pub passed: bool,
    pub state_equal: bool,
    pub effects_equal: bool,
    pub requests: usize,
    pub events: usize,
    pub final_sha256: String,
}

pub fn capture_fault_demo() -> Result<RecoveryBundle> {
    let mut live = Worker::record(42)?;
    for key in 0..4 {
        live.apply(Request {
            key,
            delta: i64::from(key) * 3,
        })?;
    }
    let checkpoint = live.start_recording()?;
    let requests: Vec<_> = (0..12)
        .map(|index| Request {
            key: index % 7,
            delta: i64::from(index) - 5,
        })
        .collect();
    for request in &requests {
        live.apply(*request)?;
    }
    let bundle = RecoveryBundle {
        checkpoint,
        log: live.log()?,
        requests,
        expected_state: live.snapshot()?.digest_hex(),
        expected_effects: live.observed_effects().to_vec(),
    };
    if live.apply(Request { key: 16, delta: 1 }).is_ok() || live.snapshot().is_ok() {
        return Err(LabError::Contract(
            "fault injection did not poison the worker",
        ));
    }
    // Dropping the failed instance ensures no mutated interpreter state leaks
    // into the subsequent reconstruction.
    Ok(bundle)
}

pub fn recover_bundle(bundle: &RecoveryBundle) -> Result<RecoveryReport> {
    recover_live(bundle).map(|(_, report)| report)
}

/// Publish only after complete replay and comparison. The caller can atomically
/// replace its failed instance with this new worker and accept new live requests.
pub fn recover_live(bundle: &RecoveryBundle) -> Result<(Worker, RecoveryReport)> {
    if bundle.requests.len() > MAX_EVENTS / 5
        || bundle.expected_effects.len() != bundle.requests.len()
    {
        return Err(LabError::Contract("recovery bundle bounds"));
    }
    let mut replay = replay_requests(&bundle.checkpoint, &bundle.log, &bundle.requests)?;
    let final_sha256 = replay.snapshot()?.digest_hex();
    let state_equal = final_sha256 == bundle.expected_state;
    let effects_equal = replay.observed_effects() == bundle.expected_effects;
    if !state_equal || !effects_equal {
        return Err(LabError::Contract("recovered state or effects differ"));
    }
    let anchor = replay.snapshot()?.digest;
    replay.store.data_mut().mode = Mode::Record {
        anchor,
        events: Vec::new(),
    };
    replay.store.data_mut().observed.clear();
    Ok((
        replay,
        RecoveryReport {
            passed: true,
            state_equal,
            effects_equal,
            requests: bundle.requests.len(),
            events: bundle.log.events.len(),
            final_sha256,
        },
    ))
}

#[cfg(test)]
mod tests;
