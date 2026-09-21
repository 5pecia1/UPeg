use std::sync::{Arc, Mutex};

use super::{
    ProgressEvent, ProgressReporter, ProgressSink, ProgressStream, SharedProgressSink,
    active_progress_sink, progress_channel, with_progress_sink,
};

/// Records everything it is handed so a test can assert on order.
#[derive(Default)]
struct RecordingSink {
    events: Mutex<Vec<ProgressEvent>>,
}

impl ProgressSink for RecordingSink {
    fn emit(&self, event: ProgressEvent) {
        self.events.lock().expect("recording sink lock").push(event);
    }
}

fn recorder() -> (Arc<RecordingSink>, SharedProgressSink) {
    let sink = Arc::new(RecordingSink::default());
    let shared: SharedProgressSink = Arc::clone(&sink) as SharedProgressSink;
    (sink, shared)
}

#[test]
fn ambient_sink_is_empty_when_none_installed() {
    assert!(active_progress_sink().is_none());
}

#[test]
fn installed_sink_is_only_visible_inside_scope() {
    let (_, shared) = recorder();
    with_progress_sink(shared, || {
        assert!(active_progress_sink().is_some());
    });
    assert!(active_progress_sink().is_none());
}

#[test]
fn nested_installation_inner_wins_and_outer_is_restored() {
    let (outer_recording, outer) = recorder();
    let (inner_recording, inner) = recorder();
    with_progress_sink(outer, || {
        with_progress_sink(inner, || {
            ProgressReporter::capture()
                .expect("inner sink")
                .report(ProgressStream::Stdout, "inner\n".to_string());
        });
        ProgressReporter::capture()
            .expect("outer sink")
            .report(ProgressStream::Stdout, "outer\n".to_string());
    });

    assert_eq!(inner_recording.events.lock().expect("lock").len(), 1);
    assert_eq!(outer_recording.events.lock().expect("lock").len(), 1);
}

#[test]
fn ambient_sink_is_restored_even_on_panic() {
    let (_, shared) = recorder();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        with_progress_sink(shared, || panic!("the tool blew up"));
    }));
    assert!(result.is_err());
    assert!(active_progress_sink().is_none());
}

#[test]
fn sequence_numbers_increase_monotonically_across_both_streams() {
    let (recording, shared) = recorder();
    let reporter = ProgressReporter::new(shared);
    let for_other_thread = reporter.clone();

    reporter.report(ProgressStream::Stdout, "a\n".to_string());
    for_other_thread.report(ProgressStream::Stderr, "b\n".to_string());
    reporter.report(ProgressStream::Stdout, "c\n".to_string());

    let events = recording.events.lock().expect("lock");
    let seqs: Vec<u64> = events.iter().map(|event| event.seq).collect();
    assert_eq!(seqs, vec![0, 1, 2]);
    assert_eq!(events[1].stream, ProgressStream::Stderr);
}

#[test]
fn empty_chunks_do_not_consume_sequence_numbers() {
    let (recording, shared) = recorder();
    let reporter = ProgressReporter::new(shared);

    reporter.report(ProgressStream::Stdout, String::new());
    reporter.report(ProgressStream::Stdout, "real\n".to_string());

    let events = recording.events.lock().expect("lock");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].seq, 0);
}

#[test]
fn channel_sink_delivers_events_to_receiver() {
    let (sink, receiver) = progress_channel();
    let reporter = ProgressReporter::new(sink);
    reporter.report(ProgressStream::Stderr, "compiling\n".to_string());

    let event = receiver.recv().expect("event received");
    assert_eq!(event.stream, ProgressStream::Stderr);
    assert_eq!(event.chunk, "compiling\n");
}

#[test]
fn channel_sink_with_gone_receiver_drops_silently() {
    let (sink, receiver) = progress_channel();
    drop(receiver);
    ProgressReporter::new(sink).report(ProgressStream::Stdout, "무주공산\n".to_string());
}

#[test]
fn a_closure_is_also_a_sink() {
    let collected = Arc::new(Mutex::new(Vec::new()));
    let collector = Arc::clone(&collected);
    let sink: SharedProgressSink = Arc::new(move |event: ProgressEvent| {
        collector.lock().expect("lock").push(event.chunk);
    });

    ProgressReporter::new(sink).report(ProgressStream::Stdout, "closure\n".to_string());

    assert_eq!(collected.lock().expect("lock").as_slice(), &["closure\n"]);
}

#[test]
fn stream_names_match_wire_spelling() {
    assert_eq!(ProgressStream::Stdout.to_string(), "stdout");
    assert_eq!(ProgressStream::Stderr.wire_name(), "stderr");
}

#[test]
fn reporters_captured_in_same_installed_scope_inherit_sequence_numbers() {
    // A Chain runs its steps one after another on the dispatching
    // thread, so each step's invoker calls `capture()` inside the one
    // sink the surface installed. The consumer was promised one stream.
    let (recording, shared) = recorder();
    with_progress_sink(shared, || {
        for step in ["first\n", "second\n", "third\n"] {
            ProgressReporter::capture()
                .expect("installed sink")
                .report(ProgressStream::Stdout, step.to_string());
        }
    });

    let events = recording.events.lock().expect("lock");
    let seqs: Vec<u64> = events.iter().map(|event| event.seq).collect();
    assert_eq!(seqs, vec![0, 1, 2], "each step must not reset to 0");
}

#[test]
fn fresh_installation_restarts_sequence_numbers_at_zero() {
    // A different `with_progress_sink` call is a different consumer, so
    // it gets its own stream — the counter must not be process-global.
    let (recording, shared) = recorder();
    for _ in 0..2 {
        with_progress_sink(SharedProgressSink::clone(&shared), || {
            ProgressReporter::capture()
                .expect("installed sink")
                .report(ProgressStream::Stdout, "call\n".to_string());
        });
    }

    let events = recording.events.lock().expect("lock");
    let seqs: Vec<u64> = events.iter().map(|event| event.seq).collect();
    assert_eq!(seqs, vec![0, 0]);
}

#[test]
fn nested_inner_scope_uses_its_own_sequence_numbers() {
    // The inner scope has its own consumer, so it starts at 0 while the
    // outer scope's counter keeps going where it left off.
    let (outer_recording, outer) = recorder();
    let (inner_recording, inner) = recorder();
    with_progress_sink(outer, || {
        let outer_reporter = ProgressReporter::capture().expect("outer sink");
        outer_reporter.report(ProgressStream::Stdout, "outer-0\n".to_string());
        with_progress_sink(inner, || {
            ProgressReporter::capture()
                .expect("inner sink")
                .report(ProgressStream::Stdout, "inner-0\n".to_string());
        });
        ProgressReporter::capture()
            .expect("restored outer sink")
            .report(ProgressStream::Stdout, "outer-1\n".to_string());
    });

    let inner_events = inner_recording.events.lock().expect("lock");
    assert_eq!(inner_events[0].seq, 0);
    let outer_events = outer_recording.events.lock().expect("lock");
    let seqs: Vec<u64> = outer_events.iter().map(|event| event.seq).collect();
    assert_eq!(seqs, vec![0, 1]);
}
