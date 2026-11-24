import Cocoa
import FlutterMacOS

class AppDelegate: FlutterAppDelegate {
  // Application menu commands must work while the Flutter window is hidden.
  @objc func openSettings(_ sender: Any?) {
    (mainFlutterWindow as? MainFlutterWindow)?.openSettings(sender)
  }

  @objc func openKeyboardHelp(_ sender: Any?) {
    (mainFlutterWindow as? MainFlutterWindow)?.openKeyboardHelp(sender)
  }

  override func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
    NSLog("upeg lifecycle: last window closed; main visible=\(mainFlutterWindow?.isVisible == true)")
    return false
  }

  override func applicationSupportsSecureRestorableState(_ app: NSApplication) -> Bool {
    return true
  }
}
