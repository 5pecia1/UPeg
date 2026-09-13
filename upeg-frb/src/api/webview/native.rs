//! Worker-side request correlation and lifetime management. The event sender
//! is a small test seam; production and tests use the same bridge state machine.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, SyncSender};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

use upeg_runtime::controlled_embed::{
    ControlledEmbedBackend, ControlledEmbedError, ControlledEmbedRequest, ControlledEmbedResponse,
    set_controlled_embed_backend,
};

use super::{
    FrbError, WebViewExecutionCompletionDto, WebViewExecutionEventDto, WebViewExecutionRequestDto,
};

const PROVIDER_ID_FIELD: &str = "provider_id";
const MAX_PENDING_REQUESTS: usize = 32;
const CANCEL_POLL_INTERVAL: Duration = Duration::from_millis(50);
const BASE_RESPONSE_TIMEOUT: Duration = Duration::from_secs(60);

type EventSender = Arc<dyn Fn(WebViewExecutionEventDto) -> bool + Send + Sync>;
type ExecutionResult = Result<ControlledEmbedResponse, ControlledEmbedError>;

#[derive(Clone, Copy, PartialEq, Eq)]
struct ProviderId(u64);

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct RequestId(u64);

struct PendingResponse {
    tool_id: String,
    deadline: Instant,
    sender: SyncSender<ExecutionResult>,
}

struct Provider {
    id: ProviderId,
    sender: Option<EventSender>,
    pending: BTreeMap<RequestId, PendingResponse>,
}

impl Provider {
    /// Ask a still-live Dart consumer to stop later page actions before its
    /// Rust callers observe disconnected response channels. Enqueueing these
    /// events is nonblocking and cannot undo actions already sent to the page.
    fn retire(self) {
        let Some(sender) = &self.sender else {
            return;
        };
        for request_id in self.pending.keys() {
            if !sender(WebViewExecutionEventDto::Cancel {
                request_id: request_id.0,
            }) {
                break;
            }
        }
    }
}

pub(super) struct WebViewBridge {
    provider: Mutex<Option<Provider>>,
    next_provider: AtomicU64,
    next_request: AtomicU64,
}

pub(super) fn bridge() -> &'static Arc<WebViewBridge> {
    static BRIDGE: OnceLock<Arc<WebViewBridge>> = OnceLock::new();
    BRIDGE.get_or_init(|| Arc::new(WebViewBridge::new()))
}

pub(super) fn install_provider() -> Result<u64, FrbError> {
    let bridge = bridge();
    let id = bridge.reserve()?;
    // Every generation shares this bridge, so concurrent registrations cannot
    // reinstall an obsolete backend. A detach leaves an unavailable backend.
    set_controlled_embed_backend(Arc::<WebViewBridge>::clone(bridge));
    Ok(id)
}

fn unavailable() -> ControlledEmbedError {
    ControlledEmbedError::Unavailable {
        reason: "desktop WebView provider is not connected".to_string(),
    }
}

impl WebViewBridge {
    fn new() -> Self {
        Self {
            provider: Mutex::new(None),
            next_provider: AtomicU64::new(0),
            next_request: AtomicU64::new(0),
        }
    }

    fn state(&self) -> Result<MutexGuard<'_, Option<Provider>>, FrbError> {
        self.provider.lock().map_err(|_| FrbError::Internal {
            message: "WebView provider registry poisoned".to_string(),
        })
    }

    fn reserve(&self) -> Result<u64, FrbError> {
        let mut state = self.state()?;
        let id = ProviderId(self.next_provider.fetch_add(1, Ordering::Relaxed));
        if let Some(previous) = state.take() {
            previous.retire();
        }
        *state = Some(Provider {
            id,
            sender: None,
            pending: BTreeMap::new(),
        });
        Ok(id.0)
    }

    pub(super) fn attach(&self, id: u64, sender: EventSender) -> Result<(), FrbError> {
        let mut state = self.state()?;
        let Some(provider) = state
            .as_mut()
            .filter(|provider| provider.id == ProviderId(id))
        else {
            return Err(FrbError::HostUnavailable);
        };
        if provider.sender.is_some() {
            return Err(FrbError::Validation {
                field: PROVIDER_ID_FIELD.to_string(),
                reason: "WebView provider stream is already attached".to_string(),
            });
        }
        // StreamSink::add enqueues without waiting on Dart. Sending while the
        // registry is locked preserves Ready-before-Execute ordering.
        if !sender(WebViewExecutionEventDto::Ready) {
            *state = None;
            return Err(FrbError::HostUnavailable);
        }
        provider.sender = Some(sender);
        Ok(())
    }

    pub(super) fn unregister(&self, id: u64) -> bool {
        let Ok(mut state) = self.state() else {
            return false;
        };
        if state
            .as_ref()
            .is_some_and(|provider| provider.id == ProviderId(id))
            && let Some(provider) = state.take()
        {
            provider.retire();
            true
        } else {
            false
        }
    }

    pub(super) fn complete(
        &self,
        provider_id: u64,
        request_id: u64,
        completion: WebViewExecutionCompletionDto,
    ) -> bool {
        let response = {
            let Ok(mut state) = self.state() else {
                return false;
            };
            let Some(provider) = state
                .as_mut()
                .filter(|provider| provider.id == ProviderId(provider_id))
            else {
                return false;
            };
            provider.pending.remove(&RequestId(request_id))
        };
        response.is_some_and(|pending| pending.sender.try_send(completion.into()).is_ok())
    }

    fn remove_pending(&self, provider_id: ProviderId, request_id: RequestId, cancel: bool) {
        let Ok(mut state) = self.state() else { return };
        let Some(provider) = state.as_mut().filter(|provider| provider.id == provider_id) else {
            return;
        };
        if provider.pending.remove(&request_id).is_some()
            && cancel
            && let Some(sender) = &provider.sender
            && !sender(WebViewExecutionEventDto::Cancel {
                request_id: request_id.0,
            })
        {
            *state = None;
        }
    }

    fn run_with_timeout(
        &self,
        request: ControlledEmbedRequest<'_>,
        timeout: Duration,
    ) -> ExecutionResult {
        let cancellation = upeg_runtime::active_cancellation();
        if cancellation
            .as_ref()
            .is_some_and(upeg_runtime::CancellationToken::is_cancelled)
        {
            return Err(ControlledEmbedError::Cancelled);
        }
        let request_id = RequestId(self.next_request.fetch_add(1, Ordering::Relaxed));
        let (response_tx, response_rx) = mpsc::sync_channel(1);
        let started = Instant::now();
        let (provider_id, deadline) = {
            let mut state = self
                .state()
                .map_err(|error| ControlledEmbedError::BackendFailed(error.to_string()))?;
            let provider = state.as_mut().ok_or_else(unavailable)?;
            let sender = provider.sender.as_ref().ok_or_else(unavailable)?;
            if provider.pending.len() >= MAX_PENDING_REQUESTS {
                return Err(ControlledEmbedError::BackendFailed(
                    "desktop WebView request queue is full".to_string(),
                ));
            }
            // Dart serializes calls per tool. Reserve enough time for the
            // preceding calls as well as this pipeline, without extending
            // deadlines for unrelated tools or allowing unbounded waiting.
            let available = provider
                .pending
                .values()
                .filter(|pending| pending.tool_id == request.tool_id)
                .map(|pending| pending.deadline)
                .max()
                .unwrap_or(started)
                .max(started);
            let deadline = available.checked_add(timeout).ok_or_else(|| {
                ControlledEmbedError::BackendFailed(
                    "desktop WebView response deadline exceeds clock range".to_string(),
                )
            })?;
            let id = provider.id;
            provider.pending.insert(
                request_id,
                PendingResponse {
                    tool_id: request.tool_id.to_string(),
                    deadline,
                    sender: response_tx,
                },
            );
            let sent = sender(WebViewExecutionEventDto::Execute {
                request: WebViewExecutionRequestDto {
                    request_id: request_id.0,
                    tool_id: request.tool_id.to_string(),
                    url: request.url.to_string(),
                    bindings: request.bindings.iter().cloned().map(Into::into).collect(),
                    settings: request.settings.into(),
                    inputs: request
                        .inputs
                        .iter()
                        .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
                        .collect(),
                },
            });
            if !sent {
                *state = None;
                return Err(unavailable());
            }
            (id, deadline)
        };
        let mut pending = PendingRequest {
            bridge: self,
            provider_id,
            request_id,
            cancel: true,
        };
        let response_timeout = deadline.duration_since(started);
        loop {
            if cancellation
                .as_ref()
                .is_some_and(upeg_runtime::CancellationToken::is_cancelled)
            {
                return Err(ControlledEmbedError::Cancelled);
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(ControlledEmbedError::ResponseTimeout {
                    timeout_ms: response_timeout.as_millis().try_into().unwrap_or(u64::MAX),
                });
            }
            match response_rx.recv_timeout(remaining.min(CANCEL_POLL_INTERVAL)) {
                Ok(result) => {
                    pending.cancel = false;
                    return result;
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return Err(unavailable()),
            }
        }
    }
}

impl ControlledEmbedBackend for WebViewBridge {
    fn run(&self, request: ControlledEmbedRequest<'_>) -> ExecutionResult {
        let timeout = response_timeout(&request);
        self.run_with_timeout(request, timeout)
    }
}

/// Cover every declared wait plus page readiness and JavaScript overhead.
/// A fixed cap would silently reject otherwise valid long binding pipelines.
fn response_timeout(request: &ControlledEmbedRequest<'_>) -> Duration {
    request
        .bindings
        .iter()
        .filter_map(|binding| binding.wait.as_ref())
        .fold(BASE_RESPONSE_TIMEOUT, |total, wait| {
            total
                .saturating_add(Duration::from_millis(wait.timeout_ms))
                .saturating_add(Duration::from_millis(wait.settle_ms))
        })
}

/// Cleanup also runs when a dispatch unwinds. No registry lock is held while
/// waiting for Dart, and late completion cannot reach another invocation.
struct PendingRequest<'a> {
    bridge: &'a WebViewBridge,
    provider_id: ProviderId,
    request_id: RequestId,
    cancel: bool,
}

impl Drop for PendingRequest<'_> {
    fn drop(&mut self) {
        self.bridge
            .remove_pending(self.provider_id, self.request_id, self.cancel);
    }
}

#[cfg(test)]
mod tests;
