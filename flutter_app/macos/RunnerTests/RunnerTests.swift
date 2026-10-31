import Cocoa
import FlutterMacOS
import XCTest

class RunnerTests: XCTestCase {

  func testTrayIconNativeCallbackKeepsAppAlive() throws {
    let environment = ProcessInfo.processInfo.environment
    let home = try XCTUnwrap(environment["UPEG_HOME"],
                             "The hosted app did not receive UPEG_HOME")
    XCTAssertTrue(home.contains("/upeg-macos-tray.") && home.hasSuffix("/home"),
                  "The hosted app is not using the isolated smoke home: \(home)")
    XCTAssertEqual(environment["UPEG_PROJECT_MANIFEST_PATH"], "off")
    if let expected = environment["UPEG_EXPECT_REDUCE_MOTION"], !expected.isEmpty {
      XCTAssertTrue(expected == "0" || expected == "1")
      let reduceMotion = expected == "1"
      XCTAssertEqual(NSWorkspace.shared.accessibilityDisplayShouldReduceMotion, reduceMotion,
                     "The OS did not apply the requested Reduce Motion preference")
      try observeStartupTrayMotion(reduceMotion: reduceMotion)
    }
    let ready = waitUntil(timeout: 45) {
      guard let window = self.flutterWindow() else { return false }
      return self.statusItemVisible() && window.isVisible && self.isFullWindow(window)
    }
    XCTAssertTrue(ready, "Flutter app or tray did not become ready; windows: \(windowClasses())")
    let window = try XCTUnwrap(flutterWindow())

    // A full, visible Pegboard must remain visible on a tray click.
    sendNativeClick(to: try XCTUnwrap(trayIcon()))
    RunLoop.main.run(until: Date().addingTimeInterval(1))
    XCTAssertTrue(window.isVisible, "tray click hid the full Pegboard")
    XCTAssertTrue(isFullWindow(window), "tray click changed the full window mode")
    XCTAssertTrue(statusItemVisible(), "tray icon or status button disappeared after click")

    // Hide the Pegboard, then close a separate visible window. The test
    // observes that the app and tray remain available after this sequence;
    // AppKit decides whether it invokes the last-window callback.
    window.orderOut(nil)
    XCTAssertFalse(window.isVisible)
    let transient = NSWindow(
      contentRect: NSRect(x: 80, y: 80, width: 160, height: 100),
      styleMask: [.titled, .closable], backing: .buffered, defer: false
    )
    transient.isReleasedWhenClosed = false
    transient.makeKeyAndOrderFront(nil)
    XCTAssertTrue(transient.isVisible)
    transient.close()
    RunLoop.main.run(until: Date().addingTimeInterval(0.5))
    XCTAssertFalse(transient.isVisible)
    XCTAssertFalse(window.isVisible)
    XCTAssertTrue(statusItemVisible(), "tray disappeared after closing a transient window")

    // A click on the real plugin view must reach the Dart tray action and
    // restore the original full window after the transient window closes.
    sendNativeClick(to: try XCTUnwrap(trayIcon()))
    XCTAssertTrue(waitUntil(timeout: 5) { window.isVisible },
                  "native tray callback did not restore the hidden Pegboard")
    RunLoop.main.run(until: Date().addingTimeInterval(1))
    XCTAssertTrue(window.isVisible, "restored Pegboard disappeared again")
    XCTAssertTrue(isFullWindow(window), "restored Pegboard lost its full window mode")
    XCTAssertTrue(statusItemVisible(), "tray icon or status button disappeared after restore")
  }

  private func isFullWindow(_ window: NSWindow) -> Bool {
    window.titleVisibility == .visible && !window.styleMask.contains(.fullSizeContentView)
  }

  private func observeStartupTrayMotion(reduceMotion: Bool) throws {
    let assets = try XCTUnwrap(Bundle.main.privateFrameworksURL)
      .appendingPathComponent("App.framework/Resources/flutter_assets/assets/tray")
    func fingerprint(_ image: NSImage) throws -> Data {
      try XCTUnwrap(image.tiffRepresentation, "Tray image could not be decoded")
    }
    func bundledImage(_ path: String) throws -> Data {
      let data = try Data(contentsOf: assets.appendingPathComponent(path))
      let image = try XCTUnwrap(NSImage(data: data),
                               "Missing bundled tray fixture: \(path)")
      image.size = NSSize(width: 22, height: 22)
      image.isTemplate = true
      return try fingerprint(image)
    }
    let resting = try bundledImage("trayTemplate@2x.png")
    let frames = try (0..<9).map { try bundledImage("sway/macos/frame_\($0).png") }
    let sequence = [resting] + frames + [resting]
    let window = try XCTUnwrap(flutterWindow(), "Flutter window did not launch")
    guard window.responds(to: NSSelectorFromString("upegTrayStartupSamples")) else {
      XCTFail("The native startup tray recorder was not armed before Flutter")
      return
    }
    let armedAt = try XCTUnwrap(
      (window.value(forKey: "upegTrayStartupArmedAt") as? NSNumber)?.doubleValue,
      "The native tray recorder has no arm timestamp")
    XCTAssertEqual(window.value(forKey: "upegTrayStartupInitialImageAbsent") as? Bool, true,
                   "The tray already had an image when early observation armed")
    XCTAssertEqual(window.value(forKey: "upegTrayStartupRecordingActive") as? Bool, true,
                   "The native tray recorder stopped before XCTest inspected it")
    func recordedSamples() -> [[String: Any]] {
      window.value(forKey: "upegTrayStartupSamples") as? [[String: Any]] ?? []
    }
    var sequenceIndex = 0
    var lastFingerprint: Data?
    var sawMotion = false
    var trace: [String] = []
    let startedAt = ProcessInfo.processInfo.systemUptime
    let readinessDeadline = startedAt + 45
    defer {
      let observation = "reduceMotion=\(reduceMotion) armedAt=\(armedAt) " +
        "testStartedAt=\(startedAt)\n" + trace.joined(separator: "\n")
      print("UPEG_TRAY_STARTUP_OBSERVATION \(observation)")
      let attachment = XCTAttachment(string: observation)
      attachment.name = "Native tray startup images"
      attachment.lifetime = .keepAlways
      add(attachment)
    }

    // The recorder was armed before FlutterViewController and retains actual
    // NSStatusBarButton images written before XCTest scheduled this method.
    while recordedSamples().isEmpty && ProcessInfo.processInfo.systemUptime < readinessDeadline {
      RunLoop.main.run(until: Date().addingTimeInterval(0.01))
    }
    let first = try XCTUnwrap(
      recordedSamples().first?["uptime"] as? TimeInterval,
      "No native tray image appeared; windows: \(windowClasses())")
    XCTAssertLessThan(armedAt, first, "Tray observation began after its first image")
    // Frames 6 and 8 equal the resting image. Collect for three seconds from
    // the first real image, then require a further idle second.
    while ProcessInfo.processInfo.systemUptime < first + 3 {
      RunLoop.main.run(until: Date().addingTimeInterval(0.01))
    }
    let idleImage = try XCTUnwrap(statusButton()?.image)
    XCTAssertEqual(try fingerprint(idleImage), resting,
                   "The current native tray image was not restored to static")
    let idleWrites = recordedSamples().count
    let idleDeadline = ProcessInfo.processInfo.systemUptime + 1
    while ProcessInfo.processInfo.systemUptime < idleDeadline {
      RunLoop.main.run(until: Date().addingTimeInterval(0.01))
      XCTAssertTrue(statusButton()?.image === idleImage,
                    "Tray continued writing images after the startup cue")
    }
    XCTAssertEqual(try fingerprint(try XCTUnwrap(statusButton()?.image)), resting,
                   "The native tray image changed from static during idle")
    let samples = recordedSamples()
    XCTAssertEqual(samples.count, idleWrites,
                   "Tray recorder saw image writes during the idle interval")
    XCTAssertTrue(samples.allSatisfy {
      ($0["uptime"] as? TimeInterval).map { $0 <= first + 3 } ?? false
    }, "Tray wrote another native image after the startup cue")
    XCTAssertEqual(window.value(forKey: "upegTrayStartupOverflowed") as? Bool, false,
                   "Tray recorder exceeded its bounded image history")
    XCTAssertEqual(window.value(forKey: "upegTrayStartupRecordingActive") as? Bool, true,
                   "The native tray recorder stopped during XCTest observation")
    for (index, sample) in samples.enumerated() {
      let data = try XCTUnwrap(sample["fingerprint"] as? Data,
                               "Tray image \(index + 1) could not be decoded")
      let image = try XCTUnwrap(sample["image"] as? NSImage)
      let uptime = try XCTUnwrap(sample["uptime"] as? TimeInterval)
      let matches = sequence.indices.filter { sequence[$0] == data }
      trace.append(String(format: "%.3fs image %d", uptime - armedAt, index + 1) +
                   " static=\(data == resting) sequencePositions=\(matches)")
      XCTAssertTrue(sample["template"] as? Bool == true && image.isTemplate,
                    "Tray image lost template rendering")
      XCTAssertEqual((sample["width"] as? NSNumber)?.doubleValue, 22)
      XCTAssertEqual((sample["height"] as? NSNumber)?.doubleValue, 22)
      if reduceMotion {
        XCTAssertEqual(data, resting, "Reduced-motion startup displayed a moving tray frame")
      } else if let next = sequence[sequenceIndex...].firstIndex(of: data) {
        sequenceIndex = next + 1
      } else {
        XCTFail("Tray images were unknown or out of order: \(trace)")
      }
      sawMotion = sawMotion || data != resting
      lastFingerprint = data
    }
    XCTAssertEqual(lastFingerprint, resting, "Tray did not restore its static image")
    let writes = samples.count
    XCTAssertLessThanOrEqual(writes, reduceMotion ? 1 : sequence.count,
                             "Startup performed unexpected native icon writes: \(trace)")
    if !reduceMotion {
      XCTAssertTrue(sawMotion,
                    "No moving native tray frame captured after the early observer armed: \(trace)")
    }
    XCTAssertEqual(NSWorkspace.shared.accessibilityDisplayShouldReduceMotion, reduceMotion,
                   "OS Reduce Motion changed during startup observation")
  }

  private func flutterWindow() -> NSWindow? {
    NSApp.windows.first { $0.contentViewController is FlutterViewController }
  }

  private func trayIcon() -> NSView? {
    for window in NSApp.windows {
      if let content = window.contentView, let icon = findTrayIcon(in: content) {
        return icon
      }
    }
    return nil
  }

  private func findTrayIcon(in view: NSView) -> NSView? {
    let name = NSStringFromClass(type(of: view))
    if name == "TrayIcon" || name.hasSuffix(".TrayIcon") { return view }
    for child in view.subviews {
      if let icon = findTrayIcon(in: child) { return icon }
    }
    return nil
  }

  private func statusItemVisible() -> Bool {
    guard let icon = trayIcon(), let window = icon.window,
          let button = statusButton() else { return false }
    return window.isVisible && button.window === window &&
      !icon.isHidden && !button.isHidden && button.image != nil
  }

  private func statusButton() -> NSStatusBarButton? {
    guard let icon = trayIcon() else { return nil }
    var ancestor = icon.superview
    while let view = ancestor {
      if let button = view as? NSStatusBarButton {
        return button
      }
      ancestor = view.superview
    }
    return nil
  }

  private func sendNativeClick(to icon: NSView) {
    let windowNumber = icon.window?.windowNumber ?? 0
    for type in [NSEvent.EventType.leftMouseDown, .leftMouseUp] {
      guard let event = NSEvent.mouseEvent(
        with: type, location: .zero, modifierFlags: [],
        timestamp: ProcessInfo.processInfo.systemUptime,
        windowNumber: windowNumber, context: nil,
        eventNumber: 0, clickCount: 1, pressure: 1
      ) else {
        XCTFail("could not create native tray mouse event")
        return
      }
      if type == .leftMouseDown {
        icon.mouseDown(with: event)
      } else {
        icon.mouseUp(with: event)
      }
    }
  }

  private func waitUntil(timeout: TimeInterval, condition: () -> Bool) -> Bool {
    let deadline = Date().addingTimeInterval(timeout)
    while Date() < deadline {
      if condition() { return true }
      RunLoop.main.run(until: Date().addingTimeInterval(0.1))
    }
    return condition()
  }

  private func windowClasses() -> String {
    NSApp.windows.map { NSStringFromClass(type(of: $0)) }.joined(separator: ", ")
  }
}
