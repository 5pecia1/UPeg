import Cocoa

if CommandLine.arguments.dropFirst().first == DesktopUpdate.helperArgument {
  exit(DesktopUpdate.runHelper(CommandLine.arguments))
}

_ = NSApplicationMain(CommandLine.argc, CommandLine.unsafeArgv)
