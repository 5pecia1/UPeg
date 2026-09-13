'use strict';

// upeg PWA app shell service worker.
//
// 왜 손으로 쓰는가: Flutter 3.29부터 `flutter_service_worker.js`는 캐싱을
// 버렸다. 지금 생성되는 파일은 `activate`에서 `self.registration.unregister()`를
// 부르는 청소용 SW다. 그래서 PWA의 오프라인 계약(`pwa.service-worker.cache`)을
// 유지하려면 SW를 우리가 소유해야 한다. 등록은 `flutter_bootstrap.js`가 한다.
//
// 캐시 전략은 세 갈래다:
//   - navigation(문서 요청): network-first. 온라인이면 항상 새 문서를 받고,
//     실패하면 캐시된 app shell 문서로 떨어진다. 앱이 영원히 낡은 채로
//     남는(Flutter가 자기 SW를 죽인 이유였던) 실패를 막는다.
//   - 부팅을 결정하는 산출물(`NETWORK_FIRST_FILE_NAMES`): 같은 network-first.
//     이유는 아래 상수 옆에 적었다.
//   - 그 외 same-origin GET: stale-while-revalidate. 캐시가 있으면 즉시
//     주고 뒤에서 갱신한다. 오프라인이면 갱신만 실패하고 응답은 캐시에서 온다.
//
// 프리캐시 목록이 짧은 이유: 빌드 형태와 무관하게 이름이 고정된 산출물은
// 문서 · 부트스트랩 · 웹 앱 manifest 셋뿐이다(`main.dart.js`는 dart2js
// 빌드에만 있는 이름이다). 이 셋은 SW가 페이지를 제어하기 *전에* 브라우저가
// 받아 가므로 설치 때 직접 넣는다. 나머지 셸(main.dart.js · canvaskit ·
// pkg/*.wasm · 폰트)은 첫 로드에서 SWR이 통째로 집어간다 —
// `flutter_bootstrap.js`가 SW가 페이지를 제어할 때까지 기다린 뒤 앱을 띄우기
// 때문이다.

// 캐시 이름은 빌드마다 달라야 한다. 이름이 고정이면 새 빌드가 옛 셸을
// 영원히 물려받는다 — Flutter가 자기 SW를 죽인 그 실패다. 등록 URL의 `?v=`가
// 그 식별자를 실어 온다(`flutter_bootstrap.js` 참고).
//
// 그 `?v=` 값의 정체를 정확히 적어 둔다. Flutter는 `{{flutter_service_worker_version}}`을
// **자기 SW를 만들던(지금은 deprecated) `--pwa-strategy` 경로에서** 채운다:
// 기본값 `offline-first`에서 `Random().nextInt(1 << 32)`를 문자열로 넣는다
// (flutter_tools `build_system/targets/web.dart`). 즉 콘텐츠 해시도 릴리스
// 식별자도 아니고 **빌드마다 새로 뽑는 난수**다 — 내용이 그대로여도 다시
// 빌드하면 바뀌고, 그래서 셸 캐시는 빌드 단위로 갈린다.
//
// 여기에 딸린 함정: `--pwa-strategy`는 숨겨졌을 뿐 아직 살아 있고,
// `--pwa-strategy none`을 주면 이 값이 `null`이 된다. 그러면
// `flutter_bootstrap.js`가 SW 등록 자체를 건너뛰어 이 파일은 한 번도 실행되지
// 않는다 — 오프라인 계약이 조용히 사라진다. 빌드 레시피는 이 플래그를
// 넘기지 않는다(Justfile `flutter-build-web` 주석 참고).
const APP_SHELL_CACHE_PREFIX = 'upeg-app-shell-';
const CACHE_VERSION_PARAM = 'v';
const UNVERSIONED_CACHE_SUFFIX = 'unversioned';
const CACHE_NAME =
  APP_SHELL_CACHE_PREFIX +
  (new URL(self.location.href).searchParams.get(CACHE_VERSION_PARAM) ||
    UNVERSIONED_CACHE_SUFFIX);
const SHELL_DOCUMENT_URL = './';
const BOOTSTRAP_URL = 'flutter_bootstrap.js';
const MANIFEST_URL = 'manifest.json';
const PRECACHED_URLS = [SHELL_DOCUMENT_URL, BOOTSTRAP_URL, MANIFEST_URL];
const CACHEABLE_METHOD = 'GET';
const NAVIGATE_REQUEST_MODE = 'navigate';

// 새 빌드가 한 로드 늦게 도착하는 것을 막는다.
//
// SWR은 캐시본을 먼저 주고 뒤에서 갱신한다. 그런데 `flutter_bootstrap.js`가
// 바로 다음 셸 캐시의 이름(`?v=`)과 어느 SW를 등록할지를 들고 있는 파일이다.
// 그것을 SWR로 주면 이번 로드는 옛 부트스트랩으로 떠서 옛 SW를 다시 등록하고,
// 새 빌드는 그 다음 로드에나 산다. `main.dart.js`도 같은 이유로 뒤집는다 —
// 문서만 새것이고 앱 코드가 옛것이면 그 조합은 아무도 빌드한 적이 없다.
//
// 오프라인에서 잃는 것은 없다: network-first는 fetch가 실패하면 캐시로
// 떨어진다.
const NETWORK_FIRST_FILE_NAMES = ['flutter_bootstrap.js', 'main.dart.js'];
const NETWORK_FIRST_PATHS = new Set(
  NETWORK_FIRST_FILE_NAMES.map(
    (name) => new URL(name, self.location.href).pathname,
  ),
);

// network-first인데 브라우저 HTTP 캐시가 낡은 사본을 그대로 돌려주면
// "항상 새것"은 한 층 아래에서 다시 무너진다. 정적 호스트가
// `Cache-Control`을 주지 않으면 Chrome은 `Last-Modified` 기반 휴리스틱으로
// 재사용해도 되는 것으로 본다. `no-cache`는 캐시를 끄는 모드가 아니라
// **매번 조건부 요청을 보내게 하는** 모드다 — 바뀐 게 없으면 304 한 번이다.
const REVALIDATED_CACHE_MODE = 'no-cache';
const revalidatingRequest = (request) =>
  new Request(request, { cache: REVALIDATED_CACHE_MODE });

self.addEventListener('install', (event) => {
  event.waitUntil(
    (async () => {
      const cache = await caches.open(CACHE_NAME);
      // `cache: 'reload'` — 설치 시점의 셸은 HTTP 캐시가 아니라 서버에서 온
      // 것이어야 한다. 아니면 낡은 문서를 오프라인 폴백으로 굳혀 버린다.
      await cache.addAll(
        PRECACHED_URLS.map((url) => new Request(url, { cache: 'reload' })),
      );
      await self.skipWaiting();
    })(),
  );
});

self.addEventListener('activate', (event) => {
  event.waitUntil(
    (async () => {
      const names = await caches.keys();
      // 우리 접두사가 붙은 캐시만 지운다. origin은 우리 것만 쓰는 곳이
      // 아니다 — 같은 호스트에 얹힌 다른 앱이나 도구의 캐시까지 쓸어버리면
      // 이 SW가 자기 셸을 갱신하는 대가로 남의 오프라인을 깬다.
      await Promise.all(
        names
          .filter(
            (name) =>
              name.startsWith(APP_SHELL_CACHE_PREFIX) && name !== CACHE_NAME,
          )
          .map((name) => caches.delete(name)),
      );
      // 첫 방문에서도 이 SW가 곧바로 페이지를 제어해야 앱이 받아오는 셸
      // 리소스가 SWR을 타고 캐시에 들어간다.
      await self.clients.claim();
    })(),
  );
});

function isCacheableRequest(request) {
  return (
    request.method === CACHEABLE_METHOD &&
    new URL(request.url).origin === self.location.origin
  );
}

/**
 * network-first: 온라인이면 항상 새것, 실패하면 캐시.
 *
 * 무엇을 보내는지(`fetchRequest`)와 무엇으로 캐시에 넣고 찾는지(`cacheKey`)를
 * 따로 받는다. navigation은 어느 경로로 들어왔든 같은 app shell 문서로
 * 답해야 해서 키를 셸 문서 하나로 모으고, 부팅 산출물은 요청 자체를 키로
 * 쓰되 보내는 쪽만 재검증 사본으로 바꾼다.
 */
async function respondNetworkFirst(fetchRequest, cacheKey) {
  const cache = await caches.open(CACHE_NAME);
  try {
    const response = await fetch(fetchRequest);
    if (response.ok) {
      await cache.put(cacheKey, response.clone());
    }
    return response;
  } catch (networkError) {
    const cached = await cache.match(cacheKey);
    if (cached) {
      return cached;
    }
    throw networkError;
  }
}

async function respondFromCacheWhileRevalidating(event, request) {
  const cache = await caches.open(CACHE_NAME);
  const cached = await cache.match(request);
  const revalidated = fetch(request)
    .then(async (response) => {
      if (response.ok) {
        await cache.put(request, response.clone());
      }
      return response;
    })
    .catch((networkError) => {
      if (cached) {
        return cached;
      }
      throw networkError;
    });

  if (cached) {
    // 백그라운드 갱신이 끝날 때까지 SW를 살려 둔다.
    event.waitUntil(revalidated);
    return cached;
  }
  return revalidated;
}

self.addEventListener('fetch', (event) => {
  const { request } = event;
  if (request.mode === NAVIGATE_REQUEST_MODE) {
    event.respondWith(respondNetworkFirst(request, SHELL_DOCUMENT_URL));
    return;
  }
  if (!isCacheableRequest(request)) {
    return;
  }
  if (NETWORK_FIRST_PATHS.has(new URL(request.url).pathname)) {
    event.respondWith(respondNetworkFirst(revalidatingRequest(request), request));
    return;
  }
  event.respondWith(respondFromCacheWhileRevalidating(event, request));
});
