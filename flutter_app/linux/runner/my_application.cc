#include "my_application.h"

#include <flutter_linux/flutter_linux.h>
#include <pwd.h>
#include <stdlib.h>
#include <unistd.h>
#ifdef GDK_WINDOWING_X11
#include <gdk/gdkx.h>
#endif

#include "flutter/generated_plugin_registrant.h"

// Window body color until Flutter paints. It is the light theme background
// (`UpegTokens.light.bg` in lib/src/theme/upeg_theme.dart); the runner never
// reads the saved theme.
static constexpr char kStartupBackground[] = "#F4F3EE";

// Full window size (`_fullSize` in lib/src/platform/window.dart).
static constexpr gint kDefaultWidth = 1280;
static constexpr gint kDefaultHeight = 800;

// Starts the engine when the window's first draw never arrives.
static constexpr guint kEngineStartFallbackMs = 1000;

// Installed into the bundle's data directory by CMakeLists.txt.
static constexpr char kWindowIconFile[] = "app_icon.png";

struct _MyApplication {
  GtkApplication parent_instance;
  char** dart_entrypoint_arguments;
  // Wayland activation token of a launch without DESKTOP_STARTUP_ID; a remote
  // instance forwards it (see my_application_add_platform_data()).
  gchar* activation_token;
  // Weak pointer: cleared when the window is destroyed.
  GtkWindow* window;
  // Owned by `window`; holds the FlView once the engine starts.
  GtkWidget* overlay;
  gulong first_draw_handler;
  guint engine_fallback_source;
  gboolean engine_started;
  gboolean window_created;
};

G_DEFINE_TYPE(MyApplication, my_application, GTK_TYPE_APPLICATION)

// Returns the canonical form of `path`, or a copy of it when it cannot be
// resolved (for example because it does not exist yet).
static gchar* real_path_or_copy(const gchar* path) {
  char* resolved = realpath(path, nullptr);
  if (resolved == nullptr) {
    return g_strdup(path);
  }
  gchar* result = g_strdup(resolved);
  free(resolved);
  return result;
}

// Folds `.` and `..` in `path` (relative to `base`) lexically, as Rust's
// normalized_root() does, and only then resolves symlinks: `link/..` stays
// the directory holding `link`, not the parent of the link's target.
static gchar* fold_then_resolve(const gchar* path, const gchar* base) {
  g_autofree gchar* folded = g_canonicalize_filename(path, base);
  return real_path_or_copy(folded);
}

// Storage root this launch resolves to: UPEG_HOME (empty means the current
// directory) or `$HOME/.upeg`. g_get_home_dir() prefers $HOME like Rust does.
static gchar* storage_root_key() {
  const gchar* upeg_home = g_getenv("UPEG_HOME");
  if (upeg_home != nullptr) {
    // Rust resolves UPEG_HOME against the physical working directory
    // (getcwd); g_get_current_dir() may return a symlinked $PWD instead.
    g_autofree gchar* logical_cwd = g_get_current_dir();
    g_autofree gchar* cwd = real_path_or_copy(logical_cwd);
    return fold_then_resolve(upeg_home, cwd);
  }
  g_autofree gchar* home = fold_then_resolve(g_get_home_dir(), "/");
  return g_build_filename(home, ".upeg", nullptr);
}

// Storage root of a launch with neither UPEG_HOME nor a changed HOME.
static gchar* default_storage_root() {
  const struct passwd* entry = getpwuid(getuid());
  const gchar* home = entry != nullptr && entry->pw_dir != nullptr
                          ? entry->pw_dir
                          : g_get_home_dir();
  g_autofree gchar* real_home = fold_then_resolve(home, "/");
  return g_build_filename(real_home, ".upeg", nullptr);
}

// GApplication id for this launch. Launches that share a storage root share
// the id, so a relaunch reaches the running instance over D-Bus; the default
// root keeps the plain APPLICATION_ID and any other root gets
// `<APPLICATION_ID>.h<first 16 hex digits of SHA-256(root)>`.
//
// - The key approximates Rust `resolve_storage` (upeg-core storage_paths.rs).
//   The Rust instance lock (upeg-frb/src/platform/instance_lock.rs) stays the
//   authority on single-instance.
// - Remaining mismatches only go one way: different ids for the same Rust root
//   (a legacy redirect, symlinked spellings). The second launch then starts
//   and the Rust lock shows its "already running" screen, which is safe. HOME
//   is part of the key so that a launch with only HOME changed (a different
//   Rust root) is never forwarded to an instance that uses another root.
// - GTK 3 derives the X11 WM_CLASS and the Wayland app_id from the prgname,
//   which stays APPLICATION_ID and matches the desktop entry. This id (with a
//   possible hash suffix) only names the D-Bus service and the window's GTK
//   application-id property.
static gchar* desktop_instance_id() {
  g_autofree gchar* key = storage_root_key();
  g_autofree gchar* default_root = default_storage_root();
  if (g_strcmp0(key, default_root) == 0) {
    return g_strdup(APPLICATION_ID);
  }
  g_autofree gchar* digest =
      g_compute_checksum_for_string(G_CHECKSUM_SHA256, key, -1);
  return g_strdup_printf("%s.h%.16s", APPLICATION_ID, digest);
}

// Sets the bundled app icon as every window's default icon (_NET_WM_ICON on
// X11). Wayland shells take the icon from the desktop entry matching app_id.
static void set_default_window_icon() {
  g_autoptr(FlDartProject) project = fl_dart_project_new();
  g_autofree gchar* data_dir =
      g_path_get_dirname(fl_dart_project_get_assets_path(project));
  g_autofree gchar* icon_path =
      g_build_filename(data_dir, kWindowIconFile, nullptr);
  g_autoptr(GError) error = nullptr;
  if (!gtk_window_set_default_icon_from_file(icon_path, &error)) {
    g_warning("Failed to load window icon %s: %s", icon_path, error->message);
    gtk_window_set_default_icon_name("upeg");
  }
}

// Paints the window body in the startup background before Flutter does.
static void set_startup_background(GtkWindow* window) {
  gtk_widget_set_name(GTK_WIDGET(window), "upeg-window");
  g_autofree gchar* css = g_strdup_printf(
      "#upeg-window { background-color: %s; }", kStartupBackground);
  g_autoptr(GtkCssProvider) provider = gtk_css_provider_new();
  gtk_css_provider_load_from_data(provider, css, -1, nullptr);
  gtk_style_context_add_provider(
      gtk_widget_get_style_context(GTK_WIDGET(window)),
      GTK_STYLE_PROVIDER(provider), GTK_STYLE_PROVIDER_PRIORITY_APPLICATION);
}

// Starts the Flutter engine inside the already visible window. Runs once,
// from whichever comes first: the idle after the first window draw or the
// fallback timer.
static void start_engine(MyApplication* self) {
  if (self->engine_started) {
    return;
  }
  self->engine_started = TRUE;
  g_clear_handle_id(&self->engine_fallback_source, g_source_remove);
  if (self->window == nullptr) {
    // The window was closed before the engine started.
    return;
  }
  // Set when the fallback timer wins before the first draw.
  g_clear_signal_handler(&self->first_draw_handler, self->window);

  g_autoptr(FlDartProject) project = fl_dart_project_new();
  fl_dart_project_set_dart_entrypoint_arguments(
      project, self->dart_entrypoint_arguments);

  FlView* view = fl_view_new(project);
  GdkRGBA background_color;
  gdk_rgba_parse(&background_color, kStartupBackground);
  fl_view_set_background_color(view, &background_color);
  gtk_widget_show(GTK_WIDGET(view));
  gtk_container_add(GTK_CONTAINER(self->overlay), GTK_WIDGET(view));

  // Realizing the view starts the engine synchronously on this thread.
  gtk_widget_realize(GTK_WIDGET(view));

  // Register plugins before returning to the main loop: window_manager needs
  // the view inside the toplevel window and replaces FlView's delete-event
  // handler, and the gtk plugin starts receiving forwarded command lines.
  fl_register_plugins(FL_PLUGIN_REGISTRY(view));

  gtk_widget_grab_focus(GTK_WIDGET(view));
}

static gboolean start_engine_idle_cb(gpointer user_data) {
  // A second call (the fallback already won) is a no-op in start_engine().
  start_engine(MY_APPLICATION(user_data));
  return G_SOURCE_REMOVE;
}

// Called after the window's first draw.
static gboolean first_draw_cb(GtkWidget* widget,
                              cairo_t* cr,
                              gpointer user_data) {
  MyApplication* self = MY_APPLICATION(user_data);
  g_clear_signal_handler(&self->first_draw_handler, widget);
  // Start the engine from an idle so this frame reaches the screen before the
  // engine blocks the main thread.
  g_idle_add(start_engine_idle_cb, self);
  return FALSE;
}

static gboolean engine_fallback_cb(gpointer user_data) {
  MyApplication* self = MY_APPLICATION(user_data);
  // GLib drops this source after G_SOURCE_REMOVE; forget the id first so no
  // later g_source_remove() gets an expired id.
  self->engine_fallback_source = 0;
  start_engine(self);
  return G_SOURCE_REMOVE;
}

// Implements GApplication::activate.
static void my_application_activate(GApplication* application) {
  MyApplication* self = MY_APPLICATION(application);

  if (self->window != nullptr) {
    // A relaunch: bring the existing window back. GTK 3 present() only shows
    // a hidden window, so show it first to take present()'s focus path, which
    // uses the remote launch's startup id.
    gtk_widget_show(GTK_WIDGET(self->window));
    gtk_window_present(self->window);
    return;
  }
  if (self->window_created) {
    // The window is gone and the app is shutting down. One window per
    // process: never open an engine-less window that holds the D-Bus name.
    return;
  }

  GtkWindow* window =
      GTK_WINDOW(gtk_application_window_new(GTK_APPLICATION(application)));

  // Use a header bar when running in GNOME as this is the common style used
  // by applications and is the setup most users will be using (e.g. Ubuntu
  // desktop).
  // If running on X and not using GNOME then just use a traditional title bar
  // in case the window manager does more exotic layout, e.g. tiling.
  // If running on Wayland assume the header bar will work (may need changing
  // if future cases occur).
  gboolean use_header_bar = TRUE;
#ifdef GDK_WINDOWING_X11
  GdkScreen* screen = gtk_window_get_screen(window);
  if (GDK_IS_X11_SCREEN(screen)) {
    const gchar* wm_name = gdk_x11_screen_get_window_manager_name(screen);
    if (g_strcmp0(wm_name, "GNOME Shell") != 0) {
      use_header_bar = FALSE;
    }
  }
#endif
  if (use_header_bar) {
    GtkHeaderBar* header_bar = GTK_HEADER_BAR(gtk_header_bar_new());
    gtk_widget_show(GTK_WIDGET(header_bar));
    gtk_header_bar_set_title(header_bar, "upeg");
    gtk_header_bar_set_show_close_button(header_bar, TRUE);
    gtk_window_set_titlebar(window, GTK_WIDGET(header_bar));
  } else {
    gtk_window_set_title(window, "upeg");
  }

  gtk_window_set_default_size(window, kDefaultWidth, kDefaultHeight);
  set_default_window_icon();
  set_startup_background(window);

  // webview_all needs a GtkOverlay between the GtkWindow and the FlView
  // so its WebKitGTK widgets can be hosted alongside the Flutter view.
  // Without this layer, the package's inline WebViewWidget cannot mount
  // its GtkOffscreenWindow → texture pipeline. See `linux/runner/` patch
  // documented in webview_all's README.
  self->overlay = gtk_overlay_new();
  gtk_widget_show(self->overlay);
  gtk_container_add(GTK_CONTAINER(window), self->overlay);

  self->window = window;
  g_object_add_weak_pointer(G_OBJECT(window),
                            reinterpret_cast<gpointer*>(&self->window));
  self->window_created = TRUE;

  // Show the window now, before the engine, Dart and Rust load; the engine
  // starts after the first draw.
  self->first_draw_handler = g_signal_connect_after(
      window, "draw", G_CALLBACK(first_draw_cb), self);
  gtk_widget_show(GTK_WIDGET(window));

  // Fallback for a first draw that never comes (the window is not mapped).
  // Armed after show, below GDK_PRIORITY_REDRAW (120) and the idle priority
  // (200), so a slow show cannot let it overtake a pending first draw.
  self->engine_fallback_source =
      g_timeout_add_full(G_PRIORITY_DEFAULT_IDLE + 10, kEngineStartFallbackMs,
                         engine_fallback_cb, self, nullptr);
}

// Implements GApplication::local_command_line.
//
// Messages printed here must stay ASCII: a remote instance never runs startup
// (so never gtk_init/setlocale) and g_printerr() turns non-ASCII into '?' in
// the C locale.
static gboolean my_application_local_command_line(GApplication* application,
                                                  gchar*** arguments,
                                                  int* exit_status) {
  MyApplication* self = MY_APPLICATION(application);
  // Strip out the first argument as it is the binary name.
  self->dart_entrypoint_arguments = g_strdupv(*arguments + 1);

  g_autoptr(GError) error = nullptr;
  if (!g_application_register(application, nullptr, &error)) {
    // For example a running instance did not answer in time. Start as a
    // separate instance; the Rust instance lock then decides.
    g_warning("Failed to register as the single instance: %s", error->message);
    g_clear_error(&error);
    g_application_set_flags(
        application, static_cast<GApplicationFlags>(
                         g_application_get_flags(application) |
                         G_APPLICATION_NON_UNIQUE));
    if (!g_application_register(application, nullptr, &error)) {
      g_warning("Failed to register: %s", error->message);
      *exit_status = 1;
      return TRUE;
    }
  }

  if (g_application_get_is_remote(application)) {
    g_printerr("upeg: already running; activated the existing window\n");
    g_application_activate(application);
    if (g_strv_length(self->dart_entrypoint_arguments) > 0) {
      // GLib forwards argv to the running instance as CommandLine and exits
      // with its reply.
      return FALSE;
    }
    GDBusConnection* connection =
        g_application_get_dbus_connection(application);
    if (connection != nullptr) {
      // Activate is sent asynchronously; deliver it before exiting.
      g_dbus_connection_flush_sync(connection, nullptr, nullptr);
    }
    *exit_status = 0;
    return TRUE;
  }

  // Primary instance: never re-emit its own argv as a command-line signal. A
  // cold-boot link reaches Dart only through Rust current_launch_intent(),
  // which reads the process argv.
  g_application_activate(application);
  *exit_status = 0;

  return TRUE;
}

// Implements GApplication::command_line.
//
// Once plugins are registered, the gtk plugin's handler runs first and the
// signal's first-wins accumulator ends the emission there. This class handler
// only receives a CommandLine forwarded before the engine started; its link
// is dropped (the preceding Activate already presented the window). It also
// keeps GApplication's default handler from warning.
static int my_application_command_line(GApplication* application,
                                       GApplicationCommandLine* command_line) {
  return 0;
}

// Implements GApplication::add_platform_data, which only a remote instance
// calls (for Activate and CommandLine). GtkApplication adds the
// DESKTOP_STARTUP_ID that GDK read when it loaded; a launch that only got a
// Wayland activation token sends that token under the same key, so the running
// instance's present() can use it.
static void my_application_add_platform_data(GApplication* application,
                                             GVariantBuilder* builder) {
  G_APPLICATION_CLASS(my_application_parent_class)
      ->add_platform_data(application, builder);
  MyApplication* self = MY_APPLICATION(application);
  if (self->activation_token != nullptr) {
    g_variant_builder_add(builder, "{sv}", "desktop-startup-id",
                          g_variant_new_string(self->activation_token));
  }
}

// Implements GApplication::startup.
static void my_application_startup(GApplication* application) {
  // MyApplication* self = MY_APPLICATION(object);

  // Perform any actions required at application startup.

  G_APPLICATION_CLASS(my_application_parent_class)->startup(application);
}

// Implements GApplication::shutdown.
static void my_application_shutdown(GApplication* application) {
  // MyApplication* self = MY_APPLICATION(object);

  // Perform any actions required at application shutdown.

  G_APPLICATION_CLASS(my_application_parent_class)->shutdown(application);
}

// Implements GObject::dispose.
static void my_application_dispose(GObject* object) {
  MyApplication* self = MY_APPLICATION(object);
  g_clear_handle_id(&self->engine_fallback_source, g_source_remove);
  if (self->window != nullptr) {
    g_clear_signal_handler(&self->first_draw_handler, self->window);
    g_object_remove_weak_pointer(G_OBJECT(self->window),
                                 reinterpret_cast<gpointer*>(&self->window));
    self->window = nullptr;
  }
  g_clear_pointer(&self->dart_entrypoint_arguments, g_strfreev);
  g_clear_pointer(&self->activation_token, g_free);
  G_OBJECT_CLASS(my_application_parent_class)->dispose(object);
}

static void my_application_class_init(MyApplicationClass* klass) {
  G_APPLICATION_CLASS(klass)->activate = my_application_activate;
  G_APPLICATION_CLASS(klass)->local_command_line =
      my_application_local_command_line;
  G_APPLICATION_CLASS(klass)->command_line = my_application_command_line;
  G_APPLICATION_CLASS(klass)->add_platform_data =
      my_application_add_platform_data;
  G_APPLICATION_CLASS(klass)->startup = my_application_startup;
  G_APPLICATION_CLASS(klass)->shutdown = my_application_shutdown;
  G_OBJECT_CLASS(klass)->dispose = my_application_dispose;
}

static void my_application_init(MyApplication* self) {}

MyApplication* my_application_new() {
  // GDK reads DESKTOP_STARTUP_ID when the library loads, before main(), and
  // GTK 3 never reads XDG_ACTIVATION_TOKEN, so setting the former here would be
  // too late. Keep a token that came without a startup id for
  // my_application_add_platform_data(), and drop it from the environment so
  // child processes do not inherit a stale one.
  const gchar* token = g_getenv("XDG_ACTIVATION_TOKEN");
  const gchar* startup_id = g_getenv("DESKTOP_STARTUP_ID");
  g_autofree gchar* activation_token = nullptr;
  if (token != nullptr && token[0] != '\0' &&
      g_utf8_validate(token, -1, nullptr) &&
      (startup_id == nullptr || startup_id[0] == '\0')) {
    activation_token = g_strdup(token);
  }
  g_unsetenv("XDG_ACTIVATION_TOKEN");

  // Set the program name to the application ID, which helps various systems
  // like GTK and desktop environments map this running application to its
  // corresponding .desktop file. This ensures better integration by allowing
  // the application to be recognized beyond its binary name.
  g_set_prgname(APPLICATION_ID);

  g_autofree gchar* instance_id = desktop_instance_id();
  const gchar* application_id =
      g_application_id_is_valid(instance_id) ? instance_id : APPLICATION_ID;
  MyApplication* self = MY_APPLICATION(g_object_new(
      my_application_get_type(), "application-id", application_id, "flags",
      G_APPLICATION_HANDLES_COMMAND_LINE, nullptr));
  self->activation_token = g_steal_pointer(&activation_token);
  return self;
}
