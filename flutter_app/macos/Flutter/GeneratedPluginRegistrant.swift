//
//  Generated file. Do not edit.
//

import FlutterMacOS
import Foundation

import app_links
import desktop_drop
import file_picker
import hotkey_manager_macos
import screen_retriever_macos
import tray_manager
import url_launcher_macos
import webview_all_wkwebview
import window_manager

func RegisterGeneratedPlugins(registry: FlutterPluginRegistry) {
  AppLinksMacosPlugin.register(with: registry.registrar(forPlugin: "AppLinksMacosPlugin"))
  DesktopDropPlugin.register(with: registry.registrar(forPlugin: "DesktopDropPlugin"))
  FilePickerPlugin.register(with: registry.registrar(forPlugin: "FilePickerPlugin"))
  HotkeyManagerMacosPlugin.register(with: registry.registrar(forPlugin: "HotkeyManagerMacosPlugin"))
  ScreenRetrieverMacosPlugin.register(with: registry.registrar(forPlugin: "ScreenRetrieverMacosPlugin"))
  TrayManagerPlugin.register(with: registry.registrar(forPlugin: "TrayManagerPlugin"))
  UrlLauncherPlugin.register(with: registry.registrar(forPlugin: "UrlLauncherPlugin"))
  WebviewAllWKWebViewPlugin.register(with: registry.registrar(forPlugin: "WebviewAllWKWebViewPlugin"))
  WindowManagerPlugin.register(with: registry.registrar(forPlugin: "WindowManagerPlugin"))
}
