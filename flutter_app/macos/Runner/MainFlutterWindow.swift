import Cocoa
import FlutterMacOS

/// Passive native image history for the hosted startup smoke. XCTest begins
/// after the app launches, so observing only inside the test misses a short
/// cue that completed before the test method was scheduled.
private final class StartupTrayImageRecorder {
  let armedAt = ProcessInfo.processInfo.systemUptime
  private(set) var initialImageAbsent = false
  private(set) var samples: [[String: Any]] = []
  private(set) var overflowed = false
  private var firstImageAt: TimeInterval?
  private var previousImage: NSImage?
  private var timer: Timer?
  var isRecording: Bool { timer?.isValid == true }

  func start() {
    initialImageAbsent = statusButton()?.image == nil
    let timer = Timer(timeInterval: 0.01, repeats: true) { [weak self] _ in
      self?.sample()
    }
    self.timer = timer
    RunLoop.main.add(timer, forMode: .common)
  }

  private func sample() {
    let now = ProcessInfo.processInfo.systemUptime
    if firstImageAt.map({ now >= $0 + 60 }) ?? (now >= armedAt + 45) {
      timer?.invalidate()
      return
    }
    guard let image = statusButton()?.image, image !== previousImage else { return }
    if samples.count == 64 {
      overflowed = true
      timer?.invalidate()
      return
    }
    firstImageAt = firstImageAt ?? now
    previousImage = image // Retain identity across future native writes.
    samples.append([
      "uptime": now,
      "image": image,
      "fingerprint": image.tiffRepresentation as Any? ?? NSNull(),
      "template": image.isTemplate,
      "width": Double(image.size.width),
      "height": Double(image.size.height),
    ])
  }

  private func statusButton() -> NSStatusBarButton? {
    func findTrayIcon(in view: NSView) -> NSView? {
      let name = NSStringFromClass(type(of: view))
      if name == "TrayIcon" || name.hasSuffix(".TrayIcon") { return view }
      for child in view.subviews {
        if let icon = findTrayIcon(in: child) { return icon }
      }
      return nil
    }
    for window in NSApp.windows {
      guard let content = window.contentView,
            let icon = findTrayIcon(in: content) else { continue }
      var ancestor = icon.superview
      while let view = ancestor {
        if let button = view as? NSStatusBarButton { return button }
        ancestor = view.superview
      }
    }
    return nil
  }

  deinit { timer?.invalidate() }
}

class MainFlutterWindow: NSWindow {
  private var accessibilityChannel: FlutterMethodChannel?
  private var startupTrayImageRecorder: StartupTrayImageRecorder?
  // KVC gives the hosted Release XCTest read-only access without requiring
  // the app target to expose a testable Swift module.
  @objc dynamic var upegTrayStartupArmedAt: NSNumber? {
    startupTrayImageRecorder.map { NSNumber(value: $0.armedAt) }
  }
  @objc dynamic var upegTrayStartupSamples: NSArray {
    (startupTrayImageRecorder?.samples ?? []) as NSArray
  }
  @objc dynamic var upegTrayStartupOverflowed: Bool {
    startupTrayImageRecorder?.overflowed ?? false
  }
  @objc dynamic var upegTrayStartupRecordingActive: Bool {
    startupTrayImageRecorder?.isRecording ?? false
  }
  @objc dynamic var upegTrayStartupInitialImageAbsent: Bool {
    startupTrayImageRecorder?.initialImageAbsent ?? false
  }
  private static let startupProfileEnabled =
    ProcessInfo.processInfo.environment["UPEG_STARTUP_PROFILE"] == "1"

  private func logStartupProfile(_ phase: String) {
    guard Self.startupProfileEnabled,
          let payload = try? JSONSerialization.data(
            withJSONObject: [
              "phase": phase,
              "native_uptime_s": ProcessInfo.processInfo.systemUptime,
            ],
            options: []) else {
      return
    }
    FileHandle.standardError.write(Data("UPEG_STARTUP_PROFILE ".utf8) + payload + Data([0x0A]))
  }

  override func awakeFromNib() {
    if let expected = ProcessInfo.processInfo.environment["UPEG_EXPECT_REDUCE_MOTION"],
       expected == "0" || expected == "1" {
      startupTrayImageRecorder = StartupTrayImageRecorder()
      startupTrayImageRecorder?.start()
    }
    logStartupProfile("native.awake_begin")
    let flutterViewController = FlutterViewController()
    logStartupProfile("native.controller_ready")
    let windowFrame = self.frame
    self.contentViewController = flutterViewController
    self.setFrame(windowFrame, display: true)

    RegisterGeneratedPlugins(registry: flutterViewController)
    logStartupProfile("native.plugins_ready")

    let accessibilityChannel = FlutterMethodChannel(
      name: "upeg/accessibility",
      binaryMessenger: flutterViewController.engine.binaryMessenger)
    accessibilityChannel.setMethodCallHandler { call, result in
      guard call.method == "reduceMotion" else {
        result(FlutterMethodNotImplemented)
        return
      }
      result(NSWorkspace.shared.accessibilityDisplayShouldReduceMotion)
    }
    self.accessibilityChannel = accessibilityChannel

    super.awakeFromNib()
    logStartupProfile("native.awake_done")
  }
}
