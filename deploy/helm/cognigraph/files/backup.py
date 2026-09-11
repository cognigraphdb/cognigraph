#!/usr/bin/env python3
"""Hot application snapshot; restore into a fresh instance for an exact copy."""
from contextlib import contextmanager
import datetime as dt
import json
import os
from pathlib import Path
import shutil
import sys
import tempfile
import time
from urllib.error import HTTPError
from urllib.request import Request, urlopen


@contextmanager
def response_for(request, timeout):
    try:
        with urlopen(request, timeout=timeout) as response:
            yield response
    except HTTPError as error:
        error.close()
        raise


def backup(url, password, directory, retention_days):
    if retention_days < 1:
        raise ValueError("retention days must be positive")
    login = Request(f"{url}/api/auth/login", method="POST",
                    data=json.dumps({"username": "admin", "password": password}).encode(),
                    headers={"Content-Type": "application/json"})
    with response_for(login, timeout=30) as response:
        token = json.load(response).get("token")
    if not isinstance(token, str) or not token:
        raise ValueError("login returned no token")

    stamp = dt.datetime.now(dt.timezone.utc).strftime("%Y%m%dT%H%M%S%fZ")
    output = directory / f"cognigraph-{stamp}.json"
    partial = None
    try:
        request = Request(f"{url}/api/admin/export", headers={"Authorization": f"Bearer {token}"})
        with tempfile.NamedTemporaryFile(dir=directory, suffix=".partial", delete=False) as stream:
            partial = Path(stream.name)
            with response_for(request, timeout=300) as response:
                shutil.copyfileobj(response, stream)
            stream.flush()
            os.fsync(stream.fileno())
        # Fully parse before publishing a completed backup or deleting older ones.
        # Size the job's memory limit for the decoded snapshot, not just its bytes.
        with partial.open(encoding="utf-8") as stream:
            snapshot = json.load(stream)
        if not isinstance(snapshot, dict) or not isinstance(snapshot.get("collections"), dict):
            raise ValueError("export is not a CogniGraph snapshot")
        partial.replace(output)
    finally:
        if partial is not None:
            partial.unlink(missing_ok=True)

    print(f"wrote {output} ({output.stat().st_size} bytes)", flush=True)
    cutoff = time.time() - retention_days * 86400
    for previous in directory.glob("cognigraph-*.json"):
        if previous != output and previous.is_file() and previous.stat().st_mtime < cutoff:
            previous.unlink()
    return output


if __name__ == "__main__":
    try:
        backup(f"http://{os.environ['COGNIGRAPH_SERVICE']}:{os.environ['COGNIGRAPH_PORT']}",
               os.environ["COGNIGRAPH_ADMIN_PASSWORD"], Path("/backups"),
               int(os.environ["COGNIGRAPH_BACKUP_RETENTION_DAYS"]))
    except Exception as error:
        # Do not echo response bodies, credentials, or authorization headers.
        print(f"backup failed ({type(error).__name__})", file=sys.stderr)
        sys.exit(1)
