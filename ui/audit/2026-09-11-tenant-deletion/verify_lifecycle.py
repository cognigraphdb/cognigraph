"""Interactive, disposable-server CG-51 verification; credentials stay in memory.

Supply CG51_HOST_PASSWORD and CG51_TENANT_PASSWORD in the environment. Run against
an isolated Enterprise instance only. Browser cancel/delete/recreate steps are
performed at the prompts; this script verifies them through independent HTTP.
"""

import json
import os
import subprocess
import urllib.error
import urllib.request
from pathlib import Path

BASE = "http://127.0.0.1:38473"
TENANT = "qa-delete-credentials-and-data"
CONTAINER = "cg-tenant-deletion-qa"
OUT = Path(__file__).with_name("lifecycle.json")
evidence = {}


def request(method, path, body=None, token=None, expected=200):
    headers = {"Content-Type": "application/json"}
    if token:
        headers["Authorization"] = "Bearer " + token
    req = urllib.request.Request(
        BASE + "/api" + path, method=method, headers=headers,
        data=None if body is None else json.dumps(body).encode(),
    )
    try:
        response = urllib.request.urlopen(req, timeout=15)
    except urllib.error.HTTPError as error:
        response = error
    with response:
        payload = json.load(response)
        assert response.status == expected, (path, response.status, expected)
        return payload


def login(username, password, expected=200):
    return request("POST", "/auth/login", {"username": username, "password": password}, expected=expected)


def files():
    return subprocess.check_output(
        ["docker", "exec", CONTAINER, "ls", "-1", "/data/tenants"], text=True
    ).splitlines()


def save():
    OUT.write_text(json.dumps(evidence, indent=2) + "\n")


host = login("host-admin", os.environ["CG51_HOST_PASSWORD"])["token"]
password = os.environ["CG51_TENANT_PASSWORD"]
evidence["created"] = request("POST", "/tenants", {"name": TENANT}, host)
user = request("POST", f"/tenants/{TENANT}/admin", {"username": "qa-deletion-admin", "password": password}, host)
session = login(user["username"], password)["token"]
api_token = request("POST", f"/users/{user['key']}/tokens", {"name": "CG-51 disposable token"}, session)["token"]
request("POST", "/collections", {"name": "qa_docs"}, session)
request("POST", "/documents", {"collection": "qa_docs", "_key": "proof", "content": "Synthetic tenant deletion evidence"}, session)
evidence["before"] = request("GET", "/documents/qa_docs/proof", token=api_token)
evidence["host_data_denied"] = request("GET", "/collections", token=host, expected=403)
save()
input("Fixture ready. Cancel deletion in the browser, then press Enter: ")
evidence["after_cancel"] = request("GET", "/documents/qa_docs/proof", token=api_token)
evidence["cancel_login_succeeded"] = "token" in login(user["username"], password)
assert evidence["after_cancel"] == evidence["before"]
save()
input("Confirm deletion in the browser, then press Enter: ")
evidence["after_delete_catalog"] = request("GET", "/tenants", token=host)
assert TENANT not in [t["name"] for t in evidence["after_delete_catalog"]["tenants"]]
evidence["after_delete_files"] = files()
assert any(name.startswith(TENANT + ".redb.deleted-") for name in evidence["after_delete_files"])
for name, token in (("session", session), ("api_token", api_token)):
    evidence["after_delete_" + name] = request("GET", "/collections", token=token, expected=401)
evidence["after_delete_login"] = login(user["username"], password, expected=401)
save()
input("Recreate the same name in the browser, then press Enter: ")
evidence["after_recreate_catalog"] = request("GET", "/tenants", token=host)
replacement = next(t for t in evidence["after_recreate_catalog"]["tenants"] if t["name"] == TENANT)
assert replacement["incarnation"] != evidence["created"]["incarnation"]
for name, token in (("session", session), ("api_token", api_token)):
    evidence["after_recreate_" + name] = request("GET", "/collections", token=token, expected=401)
evidence["after_recreate_login"] = login(user["username"], password, expected=401)
request("POST", f"/tenants/{TENANT}/admin", {"username": "qa-replacement-admin", "password": password}, host)
replacement_session = login("qa-replacement-admin", password)["token"]
evidence["replacement_catalog"] = request("GET", "/collections", token=replacement_session)
assert evidence["replacement_catalog"]["collections"] == []
evidence["replacement_files"] = files()
assert TENANT + ".redb" in evidence["replacement_files"]
assert any(name.startswith(TENANT + ".redb.deleted-") for name in evidence["replacement_files"])
save()
print("PASS: cancel preserves data/access; delete and recreation do not restore credentials or data")
