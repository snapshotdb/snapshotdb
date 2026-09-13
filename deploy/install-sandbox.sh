#!/usr/bin/env bash
# Run as root on Ubuntu. Keep the global unprivileged-userns restriction enabled;
# grant namespace creation only to the distro-owned bubblewrap executable.
set -euo pipefail
apt-get install -y -qq bubblewrap
if [ -f /proc/sys/kernel/apparmor_restrict_unprivileged_userns ] && command -v apparmor_parser >/dev/null; then
  cat > /etc/apparmor.d/anybranch-bwrap <<'PROFILE'
abi <abi/4.0>,
include <tunables/global>
profile anybranch-bwrap /usr/bin/bwrap flags=(unconfined) {
  userns,
}
PROFILE
  apparmor_parser -r /etc/apparmor.d/anybranch-bwrap
fi
