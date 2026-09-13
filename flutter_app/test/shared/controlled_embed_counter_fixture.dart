import 'dart:io';

/// A real page whose counter is lost if a client opens a different session.
final class ControlledEmbedCounterFixture {
  ControlledEmbedCounterFixture._(this._server);

  static const _pagePath = '/controlled-counter';
  final HttpServer _server;
  int pageLoads = 0;

  Uri get uri => Uri(
    scheme: 'http',
    host: InternetAddress.loopbackIPv4.address,
    port: _server.port,
    path: _pagePath,
  );

  static Future<ControlledEmbedCounterFixture> start() async {
    final server = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
    final fixture = ControlledEmbedCounterFixture._(server);
    server.listen(fixture._respond);
    return fixture;
  }

  Future<void> _respond(HttpRequest request) async {
    if (request.uri.path != _pagePath) {
      request.response.statusCode = HttpStatus.notFound;
      await request.response.close();
      return;
    }
    pageLoads++;
    request.response.headers.contentType = ContentType.html;
    request.response.write('''
<!doctype html>
<html lang="ko">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>UPeg CLI shared browser counter</title>
</head>
<body>
  <input id="step" type="number" aria-label="증가량">
  <input id="text" aria-label="입력">
  <button id="trigger">실행</button>
  <output id="echo"></output>
  <output id="counter">0</output>
  <script>
    window.counter = 0;
    document.getElementById('trigger').addEventListener('click', () => {
      window.counter += Number(document.getElementById('step').value);
      document.getElementById('counter').textContent = String(window.counter);
      document.getElementById('echo').textContent =
          document.getElementById('text').value;
    });
  </script>
</body>
</html>
''');
    await request.response.close();
  }

  Future<void> close() => _server.close(force: true);
}
