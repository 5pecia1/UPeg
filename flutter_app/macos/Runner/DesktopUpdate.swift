import Cocoa
import Darwin
import Security

enum DesktopUpdate {
  enum Outcome {
    case staged
    case manual
  }

  enum Failure: LocalizedError {
    case invalidVersion
    case verification
    case command(String)
    case handoff
    case exchange(Int32)
    case relaunch
    case rollback(String)

    var errorDescription: String? {
      switch self {
      case .invalidVersion: return "The update version is invalid or is already installed."
      case .verification: return "macOS could not verify the UPeg update."
      case .command(let operation): return "The UPeg update failed during \(operation)."
      case .handoff: return "UPeg could not start the installer."
      case .exchange(let code): return "UPeg could not replace the app (\(String(cString: strerror(code))))."
      case .relaunch: return "UPeg could not open the new version. The previous version was restored."
      case .rollback(let path): return "UPeg could not restore the previous version. It is saved at \(path)."
      }
    }
  }

  struct SigningIdentity {
    let team: String
    let requirement: SecRequirement
  }

  static let helperArgument = "--upeg-update-helper"
  private static let bundleIdentifier = "io.github.5pecia1.upeg"
  private static let stagePrefix = ".upeg-update-"
  private static let fm = FileManager.default

  static func versionParts(_ version: String) -> [UInt64]? {
    let fields = version.split(separator: ".", omittingEmptySubsequences: false)
    guard fields.count == 3 else { return nil }
    let numbers = fields.compactMap { field -> UInt64? in
      guard !field.isEmpty,
            field.utf8.allSatisfy({ (48...57).contains($0) }),
            field == "0" || field.first != "0" else { return nil }
      return UInt64(field)
    }
    return numbers.count == 3 ? numbers : nil
  }

  static func releasePage(_ version: String) -> URL {
    URL(string: "https://github.com/5pecia1/UPeg/releases/tag/v\(version)")!
  }

  static func imageURL(_ version: String) -> URL {
    URL(string: "https://github.com/5pecia1/UPeg/releases/download/v\(version)/upeg-v\(version).dmg")!
  }

  static func prepare(_ version: String) throws -> Outcome {
    guard let requested = versionParts(version) else { throw Failure.invalidVersion }
    let installed = Bundle.main.bundleURL.resolvingSymlinksInPath()
    guard let current = versionParts(bundleVersion(installed) ?? "") else { return .manual }
    guard current.lexicographicallyPrecedes(requested) else { throw Failure.invalidVersion }
    guard !installed.path.contains("/AppTranslocation/"),
          fm.isDeletableFile(atPath: installed.path),
          let team = Bundle.main.object(forInfoDictionaryKey: "UPegUpdateSigningTeam") as? String,
          let identity = try? signingIdentity(installed, expectedTeam: team),
          let executable = Bundle.main.executableURL,
          let stage = try? privateDirectory(in: installed.deletingLastPathComponent()) else {
      return .manual
    }

    var handedOff = false
    defer { if !handedOff { try? fm.removeItem(at: stage) } }
    try stageImage(version, identity: identity, at: candidate(in: stage))

    let helper = Process()
    let ready = Pipe()
    helper.executableURL = executable
    helper.arguments = [helperArgument, String(getpid()), stage.path, version]
    helper.standardOutput = ready
    try helper.run()
    guard ready.fileHandleForReading.readData(ofLength: 1) == Data([1]) else {
      throw Failure.handoff
    }
    handedOff = true
    return .staged
  }

  private static func bundleVersion(_ app: URL) -> String? {
    let plist = app.appendingPathComponent("Contents/Info.plist")
    guard let data = try? Data(contentsOf: plist),
          let info = try? PropertyListSerialization.propertyList(from: data, options: [], format: nil)
            as? [String: Any] else { return nil }
    return info["CFBundleShortVersionString"] as? String
  }

  static func signingIdentity(_ app: URL, expectedTeam: String) throws -> SigningIdentity {
    guard expectedTeam.utf8.count == 10,
          expectedTeam.utf8.allSatisfy({ (48...57).contains($0) || (65...90).contains($0) }),
          Bundle(url: app)?.bundleIdentifier == bundleIdentifier else { throw Failure.verification }
    let code = try staticCode(app)
    let developerID = """
      anchor apple generic and identifier "\(bundleIdentifier)" and
      certificate leaf[field.1.2.840.113635.100.6.1.13] exists and
      certificate leaf[subject.OU] = "\(expectedTeam)"
      """
    var ownership: SecRequirement?
    var designated: SecRequirement?
    guard SecRequirementCreateWithString(developerID as CFString, [], &ownership) == errSecSuccess,
          let ownership,
          SecStaticCodeCheckValidity(code, [], ownership) == errSecSuccess,
          SecCodeCopyDesignatedRequirement(code, [], &designated) == errSecSuccess,
          let designated else { throw Failure.verification }
    return SigningIdentity(team: expectedTeam, requirement: designated)
  }

  private static func staticCode(_ app: URL) throws -> SecStaticCode {
    var code: SecStaticCode?
    guard SecStaticCodeCreateWithPath(app as CFURL, [], &code) == errSecSuccess,
          let code else { throw Failure.verification }
    return code
  }

  static func verifyCandidate(_ app: URL, version: String, identity: SigningIdentity) throws {
    guard isDirectory(app),
          Bundle(url: app)?.bundleIdentifier == bundleIdentifier,
          bundleVersion(app) == version else { throw Failure.verification }
    _ = try signingIdentity(app, expectedTeam: identity.team)
    let code = try staticCode(app)
    guard SecStaticCodeCheckValidity(code, [], identity.requirement) == errSecSuccess else {
      throw Failure.verification
    }
    try run("/usr/bin/codesign", [
      "--verify", "--deep", "--strict", "--check-notarization", "-R=notarized", app.path,
    ], "app signature verification")
    try run("/usr/sbin/spctl", ["--assess", "--type", "execute", app.path], "app assessment")
  }

  static func stageImage(_ version: String, identity: SigningIdentity, at app: URL) throws {
    let scratch = try privateDirectory(in: fm.temporaryDirectory)
    let disk = scratch.appendingPathComponent("UPeg.dmg")
    let mount = scratch.appendingPathComponent("volume", isDirectory: true)
    var mounted = false
    defer {
      if mounted {
        mounted = (try? run("/usr/bin/hdiutil", ["detach", mount.path], "disk image detachment")) == nil
      }
      if !mounted { try? fm.removeItem(at: scratch) }
    }

    try run("/usr/bin/curl", [
      "--disable", "--fail", "--silent", "--show-error", "--location", "--max-redirs", "5",
      "--proto", "=https", "--proto-redir", "=https", "--connect-timeout", "15",
      "--max-time", "300", "--max-filesize", "1073741824",
      "--output", disk.path, imageURL(version).absoluteString,
    ], "download")
    let diskRequirement = """
      =anchor apple generic and certificate leaf[subject.OU] = "\(identity.team)" and
      certificate leaf[field.1.2.840.113635.100.6.1.13] exists and notarized
      """
    try run("/usr/bin/codesign", [
      "--verify", "--strict", "--check-notarization", "-R" + diskRequirement, disk.path,
    ], "disk image signature verification")
    try run("/usr/sbin/spctl", [
      "--assess", "--type", "open", "--context", "context:primary-signature", disk.path,
    ], "disk image assessment")
    try fm.createDirectory(at: mount, withIntermediateDirectories: false)
    try run("/usr/bin/hdiutil", [
      "attach", "-readonly", "-nobrowse", "-noautoopen", "-mountpoint", mount.path, disk.path,
    ], "disk image mounting")
    mounted = true
    let source = mount.appendingPathComponent("upeg.app", isDirectory: true)
    guard isDirectory(source) else { throw Failure.verification }
    try run("/usr/bin/ditto", [source.path, app.path], "copying the new app")
    try verifyCandidate(app, version: version, identity: identity)
  }

  private static func privateDirectory(in parent: URL) throws -> URL {
    let folder = parent.appendingPathComponent(stagePrefix + UUID().uuidString, isDirectory: true)
    try fm.createDirectory(at: folder, withIntermediateDirectories: false,
                           attributes: [.posixPermissions: 0o700])
    return folder
  }

  private static func candidate(in stage: URL) -> URL {
    stage.appendingPathComponent("upeg.app", isDirectory: true)
  }

  private static func isDirectory(_ url: URL) -> Bool {
    guard let values = try? url.resourceValues(forKeys: [.isDirectoryKey, .isSymbolicLinkKey])
    else { return false }
    return values.isDirectory == true && values.isSymbolicLink == false
  }

  private static func run(_ executable: String, _ args: [String], _ operation: String) throws {
    let process = Process()
    process.executableURL = URL(fileURLWithPath: executable)
    process.arguments = args
    process.standardOutput = FileHandle.nullDevice
    process.standardError = FileHandle.standardError
    do {
      try process.run()
      process.waitUntilExit()
    } catch {
      throw Failure.command(operation)
    }
    guard process.terminationStatus == 0 else { throw Failure.command(operation) }
  }

  static func install(_ staged: URL, over installed: URL, launch: (URL) -> Bool) throws {
    func exchange() -> Int32 {
      renameatx_np(AT_FDCWD, staged.path, AT_FDCWD, installed.path, UInt32(RENAME_SWAP))
    }
    guard exchange() == 0 else { throw Failure.exchange(errno) }
    if !launch(installed) {
      guard exchange() == 0 else { throw Failure.rollback(staged.path) }
      throw Failure.relaunch
    }
  }

  private static func openApp(_ app: URL) -> Bool {
    (try? run("/usr/bin/open", ["-n", app.path], "relaunch")) != nil
  }

  static func runHelper(_ arguments: [String]) -> Int32 {
    let installed = Bundle.main.bundleURL.resolvingSymlinksInPath()
    guard arguments.count == 5,
          let pid = Int32(arguments[2]), pid > 1, pid == getppid(),
          let requested = versionParts(arguments[4]),
          let oldVersion = bundleVersion(installed), let current = versionParts(oldVersion),
          current.lexicographicallyPrecedes(requested),
          let team = Bundle.main.object(forInfoDictionaryKey: "UPegUpdateSigningTeam") as? String,
          let identity = try? signingIdentity(installed, expectedTeam: team) else { return 1 }

    let stage = URL(fileURLWithPath: arguments[3], isDirectory: true).standardizedFileURL
    let staged = candidate(in: stage)
    let name = stage.lastPathComponent
    guard name.hasPrefix(stagePrefix),
          UUID(uuidString: String(name.dropFirst(stagePrefix.count))) != nil,
          stage.deletingLastPathComponent().path == installed.deletingLastPathComponent().path,
          isDirectory(stage), isDirectory(staged) else { return 1 }
    var preserveOld = false
    defer { if !preserveOld { try? fm.removeItem(at: stage) } }

    // Register for this process's exit before notifying Dart that it can quit.
    let queue = kqueue()
    guard queue >= 0 else { return 1 }
    defer { close(queue) }
    var event = kevent(ident: UInt(pid), filter: Int16(EVFILT_PROC), flags: UInt16(EV_ADD | EV_ONESHOT),
                       fflags: UInt32(NOTE_EXIT), data: 0, udata: nil)
    guard kevent(queue, &event, 1, nil, 0, nil) == 0 else { return 1 }
    FileHandle.standardOutput.write(Data([1]))

    var timeout = timespec(tv_sec: 120, tv_nsec: 0)
    guard kevent(queue, nil, 0, &event, 1, &timeout) == 1,
          event.fflags & UInt32(NOTE_EXIT) != 0 else { return 1 }

    do {
      try verifyCandidate(staged, version: arguments[4], identity: identity)
      // Check the destination again in case it changed while the app quit.
      _ = try signingIdentity(installed, expectedTeam: team)
      guard bundleVersion(installed) == oldVersion else {
        throw Failure.verification
      }
      try install(staged, over: installed, launch: openApp)
      return 0
    } catch {
      if case .rollback? = error as? Failure {
        preserveOld = true
      } else if bundleVersion(installed) == oldVersion,
                (try? signingIdentity(installed, expectedTeam: team)) != nil {
        _ = openApp(installed)
      }
      NSLog("UPeg update failed: %@", error.localizedDescription)
      let app = NSApplication.shared
      app.setActivationPolicy(.accessory)
      app.activate(ignoringOtherApps: true)
      let alert = NSAlert()
      alert.alertStyle = .warning
      alert.messageText = "UPeg update failed"
      alert.informativeText = error.localizedDescription
      alert.runModal()
      return 1
    }
  }
}
