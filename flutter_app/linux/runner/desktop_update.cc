#include "desktop_update.h"

#include <unistd.h>

namespace {

constexpr int kUnsupported = 20;
bool update_in_progress = false;

void update_finished(GObject* source, GAsyncResult* result, gpointer user_data) {
  g_autoptr(FlMethodCall) call = FL_METHOD_CALL(user_data);
  auto* process = G_SUBPROCESS(source);
  g_autoptr(GError) error = nullptr;
  if (g_subprocess_wait_finish(process, result, &error) &&
      g_subprocess_get_if_exited(process)) {
    switch (g_subprocess_get_exit_status(process)) {
      case 0: {
        g_autoptr(FlValue) value = fl_value_new_string("installed");
        fl_method_call_respond_success(call, value, nullptr);
        return;
      }
      case kUnsupported:
        update_in_progress = false;
        fl_method_call_respond_not_implemented(call, nullptr);
        return;
    }
  }
  update_in_progress = false;
  fl_method_call_respond_error(call, "update_failed",
                               "Could not install the Linux update.",
                               nullptr, nullptr);
}

void handle_update(FlMethodChannel*, FlMethodCall* call, gpointer) {
  if (g_strcmp0(fl_method_call_get_name(call), "performUpdate") != 0) {
    fl_method_call_respond_not_implemented(call, nullptr);
    return;
  }
  FlValue* args = fl_method_call_get_args(call);
  FlValue* value = fl_value_get_type(args) == FL_VALUE_TYPE_MAP
                       ? fl_value_lookup_string(args, "version")
                       : nullptr;
  const gchar* version = value != nullptr &&
                                  fl_value_get_type(value) == FL_VALUE_TYPE_STRING
                              ? fl_value_get_string(value)
                              : nullptr;
  if (version == nullptr ||
      !g_regex_match_simple(
          "\\A(?:0|[1-9][0-9]*)\\.(?:0|[1-9][0-9]*)\\.(?:0|[1-9][0-9]*)\\z",
          version, G_REGEX_DEFAULT, G_REGEX_MATCH_DEFAULT)) {
    fl_method_call_respond_error(call, "invalid_version",
                                 "Expected a stable release version.", nullptr,
                                 nullptr);
    return;
  }
  if (update_in_progress) {
    fl_method_call_respond_error(call, "update_in_progress",
                                 "An update is already in progress.", nullptr,
                                 nullptr);
    return;
  }

  g_autofree gchar* executable = g_file_read_link("/proc/self/exe", nullptr);
  if (executable == nullptr) {
    fl_method_call_respond_not_implemented(call, nullptr);
    return;
  }
  g_autofree gchar* bundle = g_path_get_dirname(executable);
  g_autofree gchar* helper =
      g_build_filename(bundle, "data", "desktop_update.py", nullptr);
  if (!g_file_test("/usr/bin/python3", G_FILE_TEST_IS_EXECUTABLE) ||
      !g_file_test(helper, G_FILE_TEST_IS_REGULAR)) {
    fl_method_call_respond_not_implemented(call, nullptr);
    return;
  }

  g_autofree gchar* pid = g_strdup_printf("%ld", static_cast<long>(getpid()));
  const gchar* argv[] = {"/usr/bin/python3", "-I", helper, pid, version, nullptr};
  g_autoptr(GError) error = nullptr;
  g_autoptr(GSubprocess) process =
      g_subprocess_newv(argv, G_SUBPROCESS_FLAGS_NONE, &error);
  if (process == nullptr) {
    fl_method_call_respond_error(call, "update_failed", error->message, nullptr,
                                 nullptr);
    return;
  }
  update_in_progress = true;
  g_subprocess_wait_async(process, nullptr, update_finished, g_object_ref(call));
}

}  // namespace

void register_desktop_update_channel(FlPluginRegistry* registry) {
  g_autoptr(FlPluginRegistrar) registrar =
      fl_plugin_registry_get_registrar_for_plugin(registry, "DesktopUpdate");
  g_autoptr(FlStandardMethodCodec) codec = fl_standard_method_codec_new();
  g_autoptr(FlMethodChannel) channel = fl_method_channel_new(
      fl_plugin_registrar_get_messenger(registrar), "upeg/desktop_update",
      FL_METHOD_CODEC(codec));
  fl_method_channel_set_method_call_handler(channel, handle_update, nullptr,
                                            nullptr);
}
