//! The ambient cancellation scope: installation, nesting, and the
//! guarantee that a panicking tool cannot leak a token into the next
//! dispatch on the same thread.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

use super::{CancellationToken, active_cancellation, with_cancellation};

#[test]
fn 새_토큰은_취소되지_않은_상태다() {
    let token = CancellationToken::new();

    assert!(!token.is_cancelled());
}

#[test]
fn 취소는_복제본_사이에서_공유된다() {
    let token = CancellationToken::new();
    let 복제 = token.clone();

    복제.cancel();

    assert!(token.is_cancelled(), "복제본의 취소는 원본에도 보인다");
}

#[test]
fn 취소는_되돌아가지_않는다() {
    let token = CancellationToken::new();

    token.cancel();
    token.cancel();

    assert!(token.is_cancelled());
}

#[test]
fn 설치하지_않으면_주변_토큰이_없다() {
    assert!(
        active_cancellation().is_none(),
        "아무도 설치하지 않은 dispatch는 취소 폴링 비용을 내지 않는다"
    );
}

#[test]
fn 설치한_토큰은_스코프_안에서_보인다() {
    let token = CancellationToken::new();

    let 보인_토큰 = with_cancellation(token.clone(), active_cancellation);

    let 보인_토큰 = 보인_토큰.expect("스코프 안에서는 주변 토큰이 있다");
    token.cancel();
    assert!(
        보인_토큰.is_cancelled(),
        "포착한 토큰은 설치한 토큰과 같은 래치를 본다"
    );
}

#[test]
fn 스코프를_벗어나면_이전_토큰이_복원된다() {
    let 안쪽 = CancellationToken::new();

    with_cancellation(CancellationToken::new(), || {
        with_cancellation(안쪽.clone(), || {
            안쪽.cancel();
            assert!(
                active_cancellation()
                    .expect("중첩 스코프의 토큰")
                    .is_cancelled(),
                "중첩된 안쪽 토큰이 그 구간을 이긴다"
            );
        });
        assert!(
            !active_cancellation()
                .expect("복원된 바깥 토큰")
                .is_cancelled(),
            "안쪽 스코프의 취소는 바깥 토큰으로 새지 않는다"
        );
    });

    assert!(active_cancellation().is_none(), "스코프 밖에는 토큰이 없다");
}

#[test]
fn 패닉해도_토큰이_다음_dispatch로_새지_않는다() {
    let token = CancellationToken::new();

    let 결과 = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        with_cancellation(token, || panic!("도구가 패닉했다"));
    }));

    assert!(결과.is_err(), "패닉은 그대로 전파된다");
    assert!(
        active_cancellation().is_none(),
        "unwind 중에도 이전 스코프가 복원된다"
    );
}

#[test]
fn 토큰은_다른_스레드에서도_취소할_수_있다() {
    let token = CancellationToken::new();
    let 관측 = Arc::new(AtomicBool::new(false));

    let 취소자 = {
        let token = token.clone();
        thread::spawn(move || token.cancel())
    };
    취소자.join().expect("취소 스레드가 끝난다");
    관측.store(token.is_cancelled(), Ordering::Relaxed);

    assert!(
        관측.load(Ordering::Relaxed),
        "소비자 스레드의 취소가 생산자 스레드에 보인다"
    );
}

#[test]
fn 설치된_스코프는_중첩_dispatch에_상속된다() {
    // A `Chain` runs its steps on the dispatching thread, so a step's
    // invoker must see the token the *call* was installed with.
    let token = CancellationToken::new();

    let 단계에서_본_토큰 = with_cancellation(token.clone(), || {
        fn 체인_단계() -> Option<CancellationToken> {
            active_cancellation()
        }
        체인_단계()
    });

    token.cancel();
    assert!(
        단계에서_본_토큰
            .expect("중첩 호출도 주변 토큰을 본다")
            .is_cancelled()
    );
}
