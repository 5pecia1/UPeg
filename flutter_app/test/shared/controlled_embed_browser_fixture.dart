import 'dart:io';

/// A real HTTP page for native WebView tests, isolated from external services.
final class ControlledEmbedBrowserFixture {
  ControlledEmbedBrowserFixture._(this._server);

  static const pagePath = '/shared-session';
  final HttpServer _server;
  int pageLoads = 0;

  Uri get uri => Uri(
    scheme: 'http',
    host: InternetAddress.loopbackIPv4.address,
    port: _server.port,
    path: pagePath,
  );

  static Future<ControlledEmbedBrowserFixture> start() async {
    final server = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
    final fixture = ControlledEmbedBrowserFixture._(server);
    server.listen(fixture._respond);
    return fixture;
  }

  Future<void> _respond(HttpRequest request) async {
    if (request.uri.path != pagePath) {
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
  <title>UPeg shared browser session</title>
  <style>html, body { margin: 0; padding: 0; }</style>
</head>
<body>
  <input id="input" aria-label="입력" autofocus>
  <button id="trigger" onclick="document.getElementById('output').textContent = document.getElementById('input').value">실행</button>
  <output id="output"></output>
  <script>window.sessionMarker = 'initial';</script>
</body>
</html>
''');
    await request.response.close();
  }

  Future<void> close() => _server.close(force: true);
}
