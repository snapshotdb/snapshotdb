//! Branch engines see their own data and runtime directories, never server metadata
//! or sibling sockets. The control process remains outside this mount namespace.
use crate::*;

pub fn command(b: &Branch, program: &str) -> R<Command> {
    if b.parent().is_none() && !hosted::enabled() {
        return Ok(Command::new(program));
    }
    if !cfg!(target_os = "linux") {
        return Err("agent database branches require a Linux server with bubblewrap; SQLite remains available on this platform".into());
    }
    let mut cmd = Command::new("bwrap");
    cmd.args(["--unshare-user", "--unshare-ipc", "--unshare-uts", "--cap-drop", "ALL",
        "--clearenv", "--chdir", "/", "--dev", "/dev", "--tmpfs", "/tmp", "--dir", "/proc"]);
    // Do not mount host /proc: /proc/<pid>/root and environ bypass filesystem
    // isolation or expose the control server's credentials. Engines only need
    // these non-process system counters. PID numbers remain host PID numbers.
    for path in ["/usr", "/bin", "/sbin", "/lib", "/lib64",
        "/etc/ld.so.cache", "/etc/passwd", "/etc/group", "/etc/nsswitch.conf",
        "/etc/resolv.conf", "/etc/hosts", "/etc/localtime", "/etc/ssl/certs",
        "/proc/meminfo", "/proc/cpuinfo", "/proc/stat", "/proc/diskstats",
        "/proc/vmstat", "/proc/loadavg", "/proc/uptime",
        "/sys/devices/system/cpu", "/sys/devices/system/node"] {
        if std::path::Path::new(path).exists() { cmd.args(["--ro-bind", path, path]); }
    }
    for path in [b.data(), b.run()] {
        let path = io(fs::canonicalize(path))?;
        cmd.arg("--bind").arg(&path).arg(&path);
    }
    let key = b.dir.join("keyfile");
    if key.exists() { cmd.arg("--ro-bind").arg(&key).arg(&key); }
    cmd.args(["--setenv", "PATH", &env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".into()),
        "--setenv", "LC_ALL", "C", "--setenv", "HOME", "/tmp", "--", program]);
    hosted::limit_process(&mut cmd, b)?;
    Ok(cmd)
}
