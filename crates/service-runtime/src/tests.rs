use super::*;

#[test]
fn durable_bundle_roundtrip_recovers_then_accepts_live_requests() {
    let bundle = capture_fault_demo().unwrap();
    let bytes = serde_json::to_vec(&bundle).unwrap();
    assert!(bytes.len() < 192 * 1024);
    let decoded: RecoveryBundle = serde_json::from_slice(&bytes).unwrap();
    decoded.validate().unwrap();
    let (mut worker, report) = recover_live(&decoded).unwrap();
    assert!(report.passed && report.state_equal && report.effects_equal);
    assert_eq!(worker.apply(Request { key: 1, delta: 2 }).unwrap(), 180);
    assert_eq!(worker.snapshot().unwrap().version, 17);
}

#[test]
fn corrupt_durable_bundle_never_publishes_a_worker() {
    let original = capture_fault_demo().unwrap();
    for case in 0..4 {
        let mut bundle = original.clone();
        match case {
            0 => bundle.checkpoint.memory[0] ^= 1,
            1 => {
                bundle.log.events.pop();
            }
            2 => bundle.expected_state = String::from("wrong-state"),
            _ => bundle.expected_effects[0].value += 1,
        }
        assert!(recover_live(&bundle).is_err());
    }
}
use alloc::vec;

fn request(key: i32) -> Request {
    Request {
        key,
        delta: i64::from(key) - 3,
    }
}

fn recorded() -> (Snapshot, ReplayLog, Vec<Request>, Worker) {
    let mut live = Worker::record(42).unwrap();
    live.apply(request(3)).unwrap();
    let checkpoint = live.start_recording().unwrap();
    let requests: Vec<_> = (0..8).map(request).collect();
    for request in &requests {
        live.apply(*request).unwrap();
    }
    (checkpoint, live.log().unwrap(), requests, live)
}

#[test]
fn restores_memory_and_mutable_global_into_a_fresh_instance() {
    let (checkpoint, log, requests, live) = recorded();
    assert_eq!(checkpoint.version, 1);
    assert!(checkpoint.memory.iter().any(|byte| *byte != 0));
    let replay = replay_requests(&checkpoint, &log, &requests).unwrap();
    assert_eq!(live.snapshot().unwrap(), replay.snapshot().unwrap());
    assert_eq!(replay.snapshot().unwrap().version, 9);
    assert_eq!(live.observed_effects(), replay.observed_effects());
    // The live providers in the replay store were never invoked.
    assert_eq!(replay.store.data().clock, 99_999);
    assert_eq!(replay.store.data().random, 99_999);
}

#[test]
fn empty_segment_restores_exactly() {
    let mut live = Worker::record(42).unwrap();
    let checkpoint = live.start_recording().unwrap();
    let replay = replay_requests(&checkpoint, &live.log().unwrap(), &[]).unwrap();
    assert_eq!(replay.snapshot().unwrap(), checkpoint);
}

#[test]
fn rejects_missing_reordered_and_extra_events() {
    let (checkpoint, log, requests, _) = recorded();
    // Exercise deletion at every event position, including the final reply.
    for index in 0..log.events.len() {
        let mut missing = log.clone();
        missing.events.remove(index);
        assert!(
            replay_requests(&checkpoint, &missing, &requests).is_err(),
            "removed event {index}"
        );
    }
    let mut reordered = log.clone();
    reordered.events.swap(1, 2);
    assert!(replay_requests(&checkpoint, &reordered, &requests).is_err());
    let mut extra = log.clone();
    extra.events.push(Event::Clock { value: 10 });
    assert!(replay_requests(&checkpoint, &extra, &requests).is_err());
}

#[test]
fn rejects_changed_requests_results_and_effects() {
    let (checkpoint, log, requests, _) = recorded();
    let mut changed_requests = requests.clone();
    changed_requests[0].delta += 1;
    assert!(replay_requests(&checkpoint, &log, &changed_requests).is_err());
    let mut wrong_reply = log.clone();
    let Event::Reply { value } = &mut wrong_reply.events[4] else {
        panic!()
    };
    *value += 1;
    assert!(replay_requests(&checkpoint, &wrong_reply, &requests).is_err());
    let mut wrong_effect = log.clone();
    let Event::Emit { effect } = &mut wrong_effect.events[3] else {
        panic!()
    };
    effect.version += 1;
    assert!(replay_requests(&checkpoint, &wrong_effect, &requests).is_err());
}

#[test]
fn validates_random_arguments_and_return_range() {
    let (checkpoint, log, requests, _) = recorded();
    for (bound, value) in [(96, 1), (97, -1), (97, 97)] {
        let mut changed = log.clone();
        changed.events[2] = Event::Random { bound, value };
        assert!(replay_requests(&checkpoint, &changed, &requests).is_err());
    }
}

#[test]
fn validates_checkpoint_schema_module_profile_size_and_checksum() {
    let (checkpoint, log, _, _) = recorded();
    for case in 0..7 {
        let mut changed = checkpoint.clone();
        match case {
            0 => changed.schema += 1,
            1 => changed.profile.push('!'),
            2 => changed.module[0] ^= 1,
            3 => {
                changed.memory.pop();
            }
            4 => changed.memory[0] ^= 1,
            5 => changed.version += 1,
            6 => changed.digest[0] ^= 1,
            _ => unreachable!(),
        }
        assert!(
            Worker::replay(&changed, &log, 1).is_err(),
            "corrupt field {case}"
        );
    }
    // Even with a recomputed checksum, mismatched execution metadata is rejected.
    let mut changed = checkpoint.clone();
    changed.profile.push('!');
    changed.digest = changed.hash();
    assert!(Worker::replay(&changed, &log, 1).is_err());
}

#[test]
fn binds_trace_to_checkpoint_and_profile() {
    let (checkpoint, log, _, _) = recorded();
    for case in 0..4 {
        let mut changed = log.clone();
        match case {
            0 => changed.schema += 1,
            1 => changed.profile.push('!'),
            2 => changed.module[0] ^= 1,
            3 => changed.checkpoint[0] ^= 1,
            _ => unreachable!(),
        }
        assert!(Worker::replay(&checkpoint, &changed, 1).is_err());
    }
    let other_checkpoint = Worker::record(0).unwrap().snapshot().unwrap();
    assert!(Worker::replay(&other_checkpoint, &log, 1).is_err());
}

#[test]
fn trap_poisons_worker_and_requires_restore() {
    let mut worker = Worker::record(42).unwrap();
    let checkpoint = worker.start_recording().unwrap();
    let log = worker.log().unwrap();
    assert!(worker.apply(request(16)).is_err());
    assert!(worker.observed_effects().is_empty());
    assert!(worker.snapshot().is_err());
    assert!(worker.start_recording().is_err());
    assert!(worker.log().is_err());
    assert!(worker.apply(request(0)).is_err());
    let restored = replay_requests(&checkpoint, &log, &[]).unwrap();
    assert_eq!(restored.snapshot().unwrap(), checkpoint);
}

#[test]
fn reply_mismatch_does_not_publish_a_staged_effect() {
    let (checkpoint, mut log, requests, _) = recorded();
    log.events[4] = Event::Reply { value: i64::MIN };
    let mut replay = Worker::replay(&checkpoint, &log, 7).unwrap();
    assert!(replay.apply(requests[0]).is_err());
    assert!(replay.observed_effects().is_empty());
    assert!(replay.store.data().pending.is_empty());
    assert!(replay.snapshot().is_err());
}

#[test]
fn wasm_trap_after_emit_discards_pending_effect_and_mutated_state() {
    // Private test fixture retains the same state schema; the public API accepts
    // only the compiled-in workload, never arbitrary module snapshots.
    let trapping = WORKLOAD.replace("(local.get $value)))", "unreachable (local.get $value)))");
    let bytes = wat::parse_str(trapping).unwrap();
    let mut worker = Worker::instantiate(
        &bytes,
        Mode::Record {
            anchor: [0; 32],
            events: vec![],
        },
        42,
    )
    .unwrap();
    worker.start_recording().unwrap();
    assert!(worker.apply(request(0)).is_err());
    assert_eq!(worker.version.get(&worker.store).i64(), Some(1));
    assert!(worker.observed_effects().is_empty());
    assert!(worker.store.data().pending.is_empty());
    assert!(worker.snapshot().is_err());
}

#[test]
fn caps_trace_and_effect_growth() {
    let mut worker = Worker::record(42).unwrap();
    // Each successful request contributes five events and one effect.
    for _ in 0..MAX_EVENTS / 5 {
        worker.apply(request(0)).unwrap();
    }
    assert!(worker.apply(request(0)).is_err());
    assert_eq!(worker.observed_effects().len(), MAX_EVENTS / 5);
    let Mode::Record { events, .. } = &worker.store.data().mode else {
        panic!()
    };
    assert_eq!(events.len(), MAX_EVENTS);

    let (checkpoint, mut log, _, _) = recorded();
    log.events.resize(MAX_EVENTS + 1, Event::Clock { value: 0 });
    assert!(Worker::replay(&checkpoint, &log, 0).is_err());
}

#[test]
fn fuel_exhaustion_poisons_worker() {
    let mut worker = Worker::record(42).unwrap();
    let error = worker.apply_budget(request(0), 0).unwrap_err();
    let LabError::Wasm(error) = error else {
        panic!("expected Wasm trap")
    };
    assert_eq!(error.as_trap_code(), Some(wasmi::TrapCode::OutOfFuel));
    assert!(worker.snapshot().is_err());
}

#[test]
fn infinite_loop_terminates_with_fuel_trap() {
    let engine = engine();
    let bytes =
        wat::parse_str("(module (func (export \"run\") (loop $again (br $again))))").unwrap();
    let module = Module::new(&engine, &bytes).unwrap();
    let mut store = Store::new(&engine, ());
    store.set_fuel(1_000).unwrap();
    let instance = Linker::new(&engine)
        .instantiate_and_start(&mut store, &module)
        .unwrap();
    let error = instance
        .get_typed_func::<(), ()>(&store, "run")
        .unwrap()
        .call(&mut store, ())
        .unwrap_err();
    assert_eq!(error.as_trap_code(), Some(wasmi::TrapCode::OutOfFuel));
}

#[test]
fn profile_rejects_floating_point_start_functions_and_shared_memory() {
    let engine = engine();
    for source in [
        "(module (func (result f32) f32.const 0))",
        "(module (func $start) (start $start))",
        "(module (memory 1 1 shared))",
        "(module (memory 1) (memory 1))",
    ] {
        let bytes = wat::parse_str(source).unwrap();
        assert!(Module::new(&engine, &bytes).is_err(), "accepted {source}");
    }
}

#[test]
fn memory_limit_rejects_growth_without_changing_size() {
    let engine = engine();
    let bytes = wat::parse_str("(module (memory (export \"memory\") 1) (func (export \"grow\") (result i32) i32.const 1 memory.grow))").unwrap();
    let module = Module::new(&engine, &bytes).unwrap();
    let mut store = Store::new(&engine, limits());
    store.limiter(|limits| limits);
    store.set_fuel(FUEL_PER_REQUEST).unwrap();
    let instance = Linker::new(&engine)
        .instantiate_and_start(&mut store, &module)
        .unwrap();
    let result = instance
        .get_typed_func::<(), i32>(&store, "grow")
        .unwrap()
        .call(&mut store, ());
    assert!(result.is_err());
    assert_eq!(
        instance
            .get_memory(&store, "memory")
            .unwrap()
            .data(&store)
            .len(),
        MEMORY_BYTES
    );
}

#[test]
fn demo_checks_all_claimed_properties() {
    let report = demo().unwrap();
    assert_eq!(report.trace.len(), report.trace_events);
    let emitted: Vec<_> = report
        .trace
        .iter()
        .filter_map(|event| match event {
            Event::Emit { effect } => Some(*effect),
            _ => None,
        })
        .collect();
    assert_eq!(emitted.len(), report.replayed_requests);
    for (index, sequence) in report.trace.chunks_exact(5).enumerate() {
        assert!(
            matches!(sequence[0], Event::Request { request } if request.key == (index % 7) as i32 && request.delta == index as i64 - 5)
        );
        assert!(matches!(sequence[1], Event::Clock { .. }));
        assert!(
            matches!(sequence[2], Event::Random { bound, value } if bound > 0 && value >= 0 && value < i64::from(bound))
        );
        assert!(matches!(sequence[3], Event::Emit { .. }));
        assert!(matches!(sequence[4], Event::Reply { value } if value == emitted[index].value));
    }
    assert!(report.passed);
    assert_eq!(report.replayed_requests, 12);
    assert_eq!(report.trace_events, 60);
}
