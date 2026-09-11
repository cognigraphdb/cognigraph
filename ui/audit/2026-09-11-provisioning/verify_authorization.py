"""Verify browser-created CG-52 fixtures on the isolated local QA server.

Run after the browser creates qa-onboarding, its first Admin, all four
governance roles and qa-viewer. Supply CG52_HOST_PASSWORD, CG52_TENANT_PASSWORD
and CG52_USER_PASSWORD in the environment. Successful logins stay in memory.
"""

import json
import os
import urllib.error
import urllib.request
from pathlib import Path

BASE = "http://127.0.0.1:38474/api"
evidence = {}


def request(method, path, body=None, token=None, expected=200):
    headers = {"Content-Type": "application/json"}
    if token:
        headers["Authorization"] = "Bearer " + token
    req = urllib.request.Request(
        BASE + path, method=method, headers=headers,
        data=None if body is None else json.dumps(body).encode(),
    )
    try:
        response = urllib.request.urlopen(req, timeout=15)
    except urllib.error.HTTPError as error:
        response = error
    with response:
        body = json.load(response)
        assert response.status == expected, (path, response.status, expected)
        return {"status": response.status, "body": body}


def login(name, password):
    return request("POST", "/auth/login", {"username": name, "password": password})["body"]


host = login("host-admin", os.environ["CG52_HOST_PASSWORD"])["token"]
admin_login = login("qa-onboarding-admin", os.environ["CG52_TENANT_PASSWORD"])
assert admin_login["role"] == "admin" and admin_login["tenant"] == "qa-onboarding"
admin = admin_login["token"]
users = request("GET", "/users", token=admin)
expected = {
    "qa-onboarding-admin": "admin", "qa-viewer": "viewer",
    "qa-policy-author": "policy-author", "qa-policy-approver": "policy-approver",
    "qa-promoter": "promoter", "qa-artifact-attestor": "artifact-attestor",
}
assert {u["username"]: u["role"] for u in users["body"]} == expected
assert all(u["tenant"] == "qa-onboarding" for u in users["body"])
evidence["persisted_users"] = users
evidence["tenant_catalog"] = request("GET", "/tenants", token=host)
evidence["host_data_denial"] = request("GET", "/collections", token=host, expected=403)
evidence["host_user_admin_denial"] = request("GET", "/users", token=host, expected=403)
body = {"username": "qa-forbidden", "password": "synthetic-denied-input", "role": "viewer"}
evidence["cross_tenant_denial"] = request("POST", "/users", {**body, "tenant": "default"}, admin, 403)
evidence["host_role_creation_denial"] = request("POST", "/users", {**body, "role": "host-admin"}, admin, 403)
evidence["host_user_creation_denial"] = request("POST", "/users", body, host, 403)
evidence["tenant_admin_bootstrap_denial"] = request(
    "POST", "/tenants/qa-onboarding/admin", {"username": "qa-forbidden", "password": "synthetic-denied-input"}, admin, 403
)
for username, role in expected.items():
    if role == "admin":
        continue
    account = login(username, os.environ["CG52_USER_PASSWORD"])
    assert account["tenant"] == "qa-onboarding" and account["role"] == role
    evidence[username] = {"login_status": 200, "role": role, "tenant": account["tenant"]}
    evidence[username]["data_access"] = request("GET", "/collections", token=account["token"], expected=200 if role == "viewer" else 403)
    evidence[username]["user_creation_denial"] = request("POST", "/users", body, account["token"], 403)
assert request("GET", "/users", token=admin)["body"] == users["body"]
Path(__file__).with_name("authorization.json").write_text(json.dumps(evidence, indent=2) + "\n")
print("PASS: tenant-local accounts, all supported governance roles, and provisioning/data denial boundaries")
