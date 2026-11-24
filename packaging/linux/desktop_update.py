"""Install the approved stable UPeg release for the active Linux package."""

import errno
import hashlib
from io import BytesIO
import os
from pathlib import Path
import platform
import re
import shutil
import signal
import stat
import subprocess
import sys
import tempfile
import time
from urllib.parse import parse_qs, urlsplit
from urllib.request import HTTPRedirectHandler, build_opener


RELEASE = "https://github.com/5pecia1/UPeg/releases/download"
SYSTEM_EXECUTABLE = Path("/opt/upeg/upeg")
VERSION = re.compile(r"(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)")
PAYLOAD_LIMIT = 512 * 1024 * 1024
SHUTDOWN_TIMEOUT = 60
APPIMAGE_MAGIC = b"\x7fELF"
APPIMAGE_TYPE = b"AI\x02"


class ReleaseAssetRedirect(HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        source = urlsplit(req.full_url)
        target = urlsplit(newurl)
        if (
            source.netloc != "github.com"
            or target.scheme != "https"
            or target.netloc != "release-assets.githubusercontent.com"
            or not re.match(r"/github-production-release-asset(?:-[a-f0-9]+)?/", target.path)
            or not parse_qs(target.query).get("sig")
            or target.fragment
        ):
            raise ValueError("Unexpected GitHub release asset redirect")
        return super().redirect_request(req, fp, code, msg, headers, newurl)


def download(opener, url, destination, limit):
    digest = hashlib.sha256()
    size = 0
    with opener.open(url, timeout=15) as response:
        while chunk := response.read(128 * 1024):
            size += len(chunk)
            if size > limit:
                raise ValueError("Release asset exceeds size limit")
            destination.write(chunk)
            digest.update(chunk)
    return digest.hexdigest()


def fetch_verified(version, asset, directory):
    opener = build_opener(ReleaseAssetRedirect())
    url = f"{RELEASE}/v{version}/{asset}"
    checksum = BytesIO()
    download(opener, f"{url}.sha256", checksum, 4096)
    expected = re.fullmatch(
        rb"([0-9a-f]{64}) [ *]" + re.escape(asset.encode()) + rb"\n?",
        checksum.getvalue(),
    )
    if expected is None:
        raise ValueError("Invalid release asset checksum")
    payload = directory / asset
    with payload.open("xb") as output:
        actual = download(opener, url, output, PAYLOAD_LIMIT)
        if actual != expected[1].decode("ascii"):
            raise ValueError("Release asset checksum mismatch")
        output.flush()
        os.fsync(output.fileno())
    return payload


def process_start(pid):
    # The comm field is parenthesized and may contain spaces or parentheses.
    try:
        return Path(f"/proc/{pid}/stat").read_text().rsplit(")", 1)[1].split()[19]
    except FileNotFoundError:
        return None


def image_stat(path, links=1):
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    with os.fdopen(fd, "rb") as image:
        info = os.fstat(image.fileno())
        if not stat.S_ISREG(info.st_mode) or info.st_uid != os.getuid() or info.st_nlink != links:
            raise ValueError("Expected a user-owned AppImage file")
        header = image.read(12)
        if header[:4] != APPIMAGE_MAGIC or header[8:11] != APPIMAGE_TYPE:
            raise ValueError("Expected a type 2 AppImage")
    return info


def appimage_path(executable):
    appdir = os.environ.get("APPDIR")
    appimage = os.environ.get("APPIMAGE")
    if not appdir or not appimage or os.environ.get("UPEG_APPIMAGE_LAUNCH") != "1":
        return None
    image = Path(appimage)
    if not image.is_absolute() or image.is_symlink():
        return None
    if executable != Path(appdir).resolve() / "upeg" or image.is_relative_to(Path(appdir)):
        return None
    if not os.access(image, os.W_OK | os.X_OK) or not os.access(image.parent, os.W_OK | os.X_OK):
        return None
    try:
        image_stat(image)
    except (OSError, ValueError):
        return None
    return image


def package_manager(executable):
    if executable != SYSTEM_EXECUTABLE or not os.access("/usr/bin/pkexec", os.X_OK):
        return None
    if os.access("/usr/bin/dpkg-query", os.X_OK) and os.access("/usr/bin/apt-get", os.X_OK):
        result = subprocess.run(
            ["/usr/bin/dpkg-query", "-S", str(executable)],
            capture_output=True, text=True, timeout=10,
        )
        if result.returncode == 0 and result.stdout.strip() == f"upeg: {executable}":
            return "deb", "/usr/bin/apt-get"
    if os.access("/usr/bin/rpm", os.X_OK) and os.access("/usr/bin/dnf", os.X_OK):
        result = subprocess.run(
            ["/usr/bin/rpm", "-qf", "--qf", "%{NAME}", str(executable)],
            capture_output=True, text=True, timeout=10,
        )
        if result.returncode == 0 and result.stdout == "upeg":
            return "rpm", "/usr/bin/dnf"
    return None


def image_identity(info):
    return info.st_dev, info.st_ino, info.st_size, info.st_mtime_ns


def restore_appimage(image, staging, replacement):
    if image_identity(image_stat(image)) == replacement:
        os.replace(staging / "previous.AppImage", image)
        return True
    return False


def launch(executable):
    env = os.environ.copy()
    for name in ("APPIMAGE", "APPDIR", "ARGV0", "OWD", "UPEG_APPIMAGE_LAUNCH"):
        env.pop(name, None)
    return subprocess.Popen([str(executable)], env=env, start_new_session=True)


def relaunch_after_exit(pid, started, executable, staging=None, replacement=None):
    # Return to Flutter so it can run its normal shutdown and release its
    # D-Bus name and storage lock before starting the replacement.
    child = os.fork()
    if child != 0:
        return child
    status = 1
    remove_staging = False
    try:
        os.setsid()
        with open(os.devnull, "r+b") as stream:
            for fd in (0, 1, 2):
                os.dup2(stream.fileno(), fd)
        deadline = time.monotonic() + SHUTDOWN_TIMEOUT
        while process_start(pid) == started:
            if time.monotonic() >= deadline:
                if staging is not None:
                    remove_staging = restore_appimage(executable, staging, replacement)
                return
            time.sleep(0.2)
        try:
            launch(executable)
        except OSError:
            if staging is None:
                raise
            remove_staging = restore_appimage(executable, staging, replacement)
            if not remove_staging:
                return
            launch(executable)
        else:
            remove_staging = staging is not None
        status = 0
    finally:
        if remove_staging:
            shutil.rmtree(staging, ignore_errors=True)
        os._exit(status)


def download_timeout(_signal, _frame):
    raise TimeoutError("Timed out downloading UPeg update")


def timed_fetch(version, asset, directory):
    signal.signal(signal.SIGALRM, download_timeout)
    signal.alarm(300)
    try:
        return fetch_verified(version, asset, directory)
    finally:
        signal.alarm(0)


def replace_appimage(image, version, staging):
    original = image_stat(image)
    payload = timed_fetch(version, f"upeg-v{version}-x86_64.AppImage", staging)
    image_stat(payload)
    # Preserve the path the user launched (even if its name predates the
    # release). Never overwrite an image replaced during download or backup.
    current = image_stat(image)
    if image_identity(current) != image_identity(original) or current.st_ctime_ns != original.st_ctime_ns:
        raise ValueError("The running AppImage changed during the update")
    backup = staging / "previous.AppImage"
    try:
        os.link(image, backup, follow_symlinks=False)
    except OSError as error:
        if error.errno not in (errno.EPERM, errno.EOPNOTSUPP, errno.ENOSYS, errno.EXDEV, errno.EMLINK):
            raise
        # Filesystems such as FAT cannot keep the original inode via a link.
        shutil.copy2(image, backup, follow_symlinks=False)
        image_stat(backup)
        current = image_stat(image)
        if image_identity(current) != image_identity(original) or current.st_ctime_ns != original.st_ctime_ns:
            raise ValueError("The running AppImage changed during backup")
    else:
        if (
            image_identity(image_stat(backup, links=2)) != image_identity(original)
            or image_identity(image_stat(image, links=2)) != image_identity(original)
        ):
            raise ValueError("The running AppImage changed during backup")
    payload.chmod(stat.S_IMODE(original.st_mode) & 0o777)
    replacement = image_identity(payload.stat())
    os.replace(payload, image)
    return replacement


def install_appimage(image, version, pid, started):
    staging = Path(tempfile.mkdtemp(prefix=".upeg-update-", dir=image.parent))
    try:
        replacement = replace_appimage(image, version, staging)
    except (OSError, ValueError, subprocess.SubprocessError):
        shutil.rmtree(staging, ignore_errors=True)
        raise
    try:
        relaunch_after_exit(pid, started, image, staging, replacement)
    except OSError:
        if restore_appimage(image, staging, replacement):
            shutil.rmtree(staging, ignore_errors=True)
        raise


def install_package(kind, manager, version):
    asset = (
        f"upeg_{version}_amd64.deb" if kind == "deb"
        else f"upeg-{version}-1.x86_64.rpm"
    )
    with tempfile.TemporaryDirectory(prefix="upeg-update-") as temp:
        package = timed_fetch(version, asset, Path(temp))
        package.chmod(0o400)
        subprocess.run(["/usr/bin/pkexec", manager, "install", "-y", str(package)], check=True)


def main(pid, version):
    if VERSION.fullmatch(version) is None:
        raise ValueError("Expected a stable release version")
    if platform.machine() != "x86_64" or os.getuid() == 0:
        return 20
    if pid != os.getppid():
        raise ValueError("Expected the UPeg parent process")
    started = process_start(pid)
    if started is None:
        raise ValueError("The UPeg parent process has exited")
    executable = Path(os.readlink(f"/proc/{pid}/exe"))
    image = appimage_path(executable)
    if image is not None:
        install_appimage(image, version, pid, started)
        return 0
    package = package_manager(executable)
    if package is None:
        return 20
    install_package(*package, version)
    relaunch_after_exit(pid, started, executable)
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main(int(sys.argv[1]), sys.argv[2]))
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print(f"UPeg update: {error}", file=sys.stderr)
        sys.exit(1)
