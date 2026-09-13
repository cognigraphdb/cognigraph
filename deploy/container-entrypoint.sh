#!/bin/sh
set -eu

# Railway mounts volumes as root. Only this fixed, private child directory is
# provisioned; existing data is never recursively chowned or repaired.
if [ "$(id -u)" = 0 ]; then
    if [ "${RAILWAY_VOLUME_MOUNT_PATH:-}" != /data ] ||
       [ "${COGNIGRAPH_NATIVE_PATH:-}" != /data/native/cognigraph.redb ] ||
       [ ! -d /data ] || [ -L /data ] || [ -L /data/native ]; then
        echo 'Root startup requires the Railway /data volume and its fixed Native path' >&2
        exit 1
    fi
    umask 077
    if [ ! -e /data/native ]; then
        mkdir /data/native
        chown 10001:10001 /data/native
    fi
    if [ ! -d /data/native ] ||
       [ "$(stat -c '%u:%g:%a' /data/native)" != 10001:10001:700 ]; then
        echo 'Native directory must be owned by 10001:10001 with mode 0700' >&2
        exit 1
    fi
    exec setpriv --reuid=10001 --regid=10001 --clear-groups \
        --inh-caps=-all --ambient-caps=-all --no-new-privs cognigraph-server "$@"
fi

exec cognigraph-server "$@"
