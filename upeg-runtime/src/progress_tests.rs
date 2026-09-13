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
        self.events.lock().expect("기록 sink 잠금").push(event);
    }
}

fn 기록기() -> (Arc<RecordingSink>, SharedProgressSink) {
    let sink = Arc::new(RecordingSink::default());
    let shared: SharedProgressSink = Arc::clone(&sink) as SharedProgressSink;
    (sink, shared)
}

#[test]
fn sink이_없으면_주변_sink는_비어_있다() {
    assert!(active_progress_sink().is_none());
}

#[test]
fn 설치된_sink는_범위_안에서만_보인다() {
    let (_, shared) = 기록기();
    with_progress_sink(shared, || {
        assert!(active_progress_sink().is_some());
    });
    assert!(active_progress_sink().is_none());
}

#[test]
fn 중첩_설치는_안쪽이_이기고_바깥이_복원된다() {
    let (바깥_기록, 바깥) = 기록기();
    let (안쪽_기록, 안쪽) = 기록기();
    with_progress_sink(바깥, || {
        with_progress_sink(안쪽, || {
            ProgressReporter::capture()
                .expect("안쪽 sink")
                .report(ProgressStream::Stdout, "inner\n".to_string());
        });
        ProgressReporter::capture()
            .expect("바깥 sink")
            .report(ProgressStream::Stdout, "outer\n".to_string());
    });

    assert_eq!(안쪽_기록.events.lock().expect("잠금").len(), 1);
    assert_eq!(바깥_기록.events.lock().expect("잠금").len(), 1);
}

#[test]
fn 패닉이_나도_주변_sink는_복원된다() {
    let (_, shared) = 기록기();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        with_progress_sink(shared, || panic!("도구가 터졌다"));
    }));
    assert!(result.is_err());
    assert!(active_progress_sink().is_none());
}

#[test]
fn 순번은_두_스트림에_걸쳐_단조증가한다() {
    let (기록, shared) = 기록기();
    let reporter = ProgressReporter::new(shared);
    let 다른_스레드용 = reporter.clone();

    reporter.report(ProgressStream::Stdout, "a\n".to_string());
    다른_스레드용.report(ProgressStream::Stderr, "b\n".to_string());
    reporter.report(ProgressStream::Stdout, "c\n".to_string());

    let events = 기록.events.lock().expect("잠금");
    let seqs: Vec<u64> = events.iter().map(|event| event.seq).collect();
    assert_eq!(seqs, vec![0, 1, 2]);
    assert_eq!(events[1].stream, ProgressStream::Stderr);
}

#[test]
fn 빈_청크는_순번을_소비하지_않는다() {
    let (기록, shared) = 기록기();
    let reporter = ProgressReporter::new(shared);

    reporter.report(ProgressStream::Stdout, String::new());
    reporter.report(ProgressStream::Stdout, "real\n".to_string());

    let events = 기록.events.lock().expect("잠금");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].seq, 0);
}

#[test]
fn 채널_sink는_수신자에게_이벤트를_전달한다() {
    let (sink, receiver) = progress_channel();
    let reporter = ProgressReporter::new(sink);
    reporter.report(ProgressStream::Stderr, "compiling\n".to_string());

    let event = receiver.recv().expect("이벤트 수신");
    assert_eq!(event.stream, ProgressStream::Stderr);
    assert_eq!(event.chunk, "compiling\n");
}

#[test]
fn 수신자가_사라진_채널_sink는_조용히_버린다() {
    let (sink, receiver) = progress_channel();
    drop(receiver);
    ProgressReporter::new(sink).report(ProgressStream::Stdout, "무주공산\n".to_string());
}

#[test]
fn 클로저도_sink가_된다() {
    let 수집 = Arc::new(Mutex::new(Vec::new()));
    let 수집기 = Arc::clone(&수집);
    let sink: SharedProgressSink = Arc::new(move |event: ProgressEvent| {
        수집기.lock().expect("잠금").push(event.chunk);
    });

    ProgressReporter::new(sink).report(ProgressStream::Stdout, "closure\n".to_string());

    assert_eq!(수집.lock().expect("잠금").as_slice(), &["closure\n"]);
}

#[test]
fn 스트림_이름은_wire_철자와_일치한다() {
    assert_eq!(ProgressStream::Stdout.to_string(), "stdout");
    assert_eq!(ProgressStream::Stderr.wire_name(), "stderr");
}

#[test]
fn 같은_설치_범위에서_잡은_보고자들은_순번을_이어받는다() {
    // A Chain runs its steps one after another on the dispatching
    // thread, so each step's invoker calls `capture()` inside the one
    // sink the surface installed. The consumer was promised one stream.
    let (기록, shared) = 기록기();
    with_progress_sink(shared, || {
        for 단계 in ["first\n", "second\n", "third\n"] {
            ProgressReporter::capture()
                .expect("설치된 sink")
                .report(ProgressStream::Stdout, 단계.to_string());
        }
    });

    let events = 기록.events.lock().expect("잠금");
    let seqs: Vec<u64> = events.iter().map(|event| event.seq).collect();
    assert_eq!(seqs, vec![0, 1, 2], "단계마다 0으로 되돌아가면 안 된다");
}

#[test]
fn 설치를_새로_하면_순번도_0부터_다시_시작한다() {
    // A different `with_progress_sink` call is a different consumer, so
    // it gets its own stream — the counter must not be process-global.
    let (기록, shared) = 기록기();
    for _ in 0..2 {
        with_progress_sink(SharedProgressSink::clone(&shared), || {
            ProgressReporter::capture()
                .expect("설치된 sink")
                .report(ProgressStream::Stdout, "call\n".to_string());
        });
    }

    let events = 기록.events.lock().expect("잠금");
    let seqs: Vec<u64> = events.iter().map(|event| event.seq).collect();
    assert_eq!(seqs, vec![0, 0]);
}

#[test]
fn 중첩된_안쪽_범위는_자기_순번을_쓴다() {
    // The inner scope has its own consumer, so it starts at 0 while the
    // outer scope's counter keeps going where it left off.
    let (바깥_기록, 바깥) = 기록기();
    let (안쪽_기록, 안쪽) = 기록기();
    with_progress_sink(바깥, || {
        let 바깥_보고자 = ProgressReporter::capture().expect("바깥 sink");
        바깥_보고자.report(ProgressStream::Stdout, "outer-0\n".to_string());
        with_progress_sink(안쪽, || {
            ProgressReporter::capture()
                .expect("안쪽 sink")
                .report(ProgressStream::Stdout, "inner-0\n".to_string());
        });
        ProgressReporter::capture()
            .expect("복원된 바깥 sink")
            .report(ProgressStream::Stdout, "outer-1\n".to_string());
    });

    let 안쪽_이벤트 = 안쪽_기록.events.lock().expect("잠금");
    assert_eq!(안쪽_이벤트[0].seq, 0);
    let 바깥_이벤트 = 바깥_기록.events.lock().expect("잠금");
    let seqs: Vec<u64> = 바깥_이벤트.iter().map(|event| event.seq).collect();
    assert_eq!(seqs, vec![0, 1]);
}
