import 'package:shared_preferences/shared_preferences.dart';

const _key = 'dismissed_update_version';

Future<String?> readDismissedUpdate() async {
  try {
    return await SharedPreferencesAsync().getString(_key);
  } on Exception {
    return null;
  }
}

Future<void> writeDismissedUpdate(String version) =>
    SharedPreferencesAsync().setString(_key, version);
