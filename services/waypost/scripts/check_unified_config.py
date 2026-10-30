#!/usr/bin/env python3
"""Validate non-secret fresh unified production bindings; never reads credentials."""
import argparse
import json
from pathlib import Path
import tomllib
from urllib.parse import urlparse
from uuid import UUID


def check(path: Path) -> list[str]:
    config = tomllib.loads(path.read_text())
    issues = []
    variables = config.get("vars", {})
    for key, value in {
        "DEPLOYMENT_ENV": "production", "WAYPOST_PRODUCT": "unified",
        "WAYPOST_OPERATOR_MODE": "disabled", "WAYPOST_ONBOARDING_MODE": "accounts",
    }.items():
        if variables.get(key) != value:
            issues.append(f"{key} must be {value}")
    def configured(value):
        return isinstance(value, str) and value and "replace" not in value.lower()
    if not configured(config.get("account_id")):
        issues.append("Cloudflare account_id must be configured")
    if config.get("name") in (None, "waypost-production", "waypost-keys-production"):
        issues.append("Worker name must be new and separate from both legacy Workers")
    subject = variables.get("WORKS_OWNER_SUBJECT")
    if not configured(subject) or len(subject) > 255 or any(ord(c) < 32 for c in subject):
        issues.append("WORKS_OWNER_SUBJECT must be the verified Google subject, not a placeholder/email")
    elif "@" in subject:
        issues.append("WORKS_OWNER_SUBJECT is a Google subject, not an email")
    if variables.get("WAYPOST_ACCOUNT_SIGNUP_LIMIT") not in ("0", "1"):
        issues.append("Personal production signup limit must be 1 (or 0 after initialization)")
    parsed = urlparse(variables.get("PUBLIC_ORIGIN", ""))
    if parsed.scheme != "https" or not parsed.netloc or parsed.path or parsed.query or parsed.fragment or parsed.username or parsed.password:
        issues.append("PUBLIC_ORIGIN must be a bare HTTPS origin")
    if parsed.hostname in ("waypost.miles1150.top", "keys.miles1150.top"):
        issues.append("Unified origin must preserve both legacy origins")
    if not any(r.get("custom_domain") is True and r.get("pattern") == parsed.netloc for r in config.get("routes", [])):
        issues.append("Custom domain route must exactly match PUBLIC_ORIGIN")
    if config.get("workers_dev") is not False or config.get("preview_urls") is not False:
        issues.append("workers_dev and preview_urls must be disabled")
    try:
        discovery = json.loads(variables.get("WAYPOST_OAUTH_CLIENT_DISCOVERY", "{}"))
        if discovery.get("dynamic_registration") is not True:
            issues.append("Dynamic client registration must be enabled")
    except (TypeError, ValueError):
        issues.append("OAuth discovery configuration must be JSON")
    databases = {d.get("binding"): d for d in config.get("d1_databases", [])}
    ids = []
    for binding in ("DB", "KEYS_DB"):
        database = databases.get(binding, {})
        try:
            ids.append(str(UUID(database.get("database_id", ""))))
        except (TypeError, ValueError, AttributeError):
            issues.append(f"{binding} must name a fresh D1 database UUID")
        if database.get("database_name") in (None, "waypost-production", "waypost-keys-production"):
            issues.append(f"{binding} must use a fresh database name")
        if database.get("migrations_dir") != "migrations":
            issues.append(f"{binding} must apply the full migrations directory")
    if len(ids) == 2 and ids[0] == ids[1]:
        issues.append("DB and KEYS_DB must point to different database IDs")
    objects = [b for b in config.get("r2_buckets", []) if b.get("binding") == "OBJECTS"]
    if len(objects) != 1 or objects[0].get("bucket_name") in (None, "waypost-production-objects"):
        issues.append("OBJECTS must use a fresh R2 bucket")
    for secret in ("WAYPOST_OIDC_CONFIG", "KEYS_DATA_KEYS", "WAYPOST_ENCRYPTION_KEYS", "WAYPOST_CURSOR_SECRET", "WAYPOST_CSRF_SECRET"):
        if secret in variables:
            issues.append(f"{secret} must be a Worker secret, not a plain variable")
    limiters = {r.get("name"): r for r in config.get("ratelimits", [])}
    namespaces = []
    for binding in ("ADMIN_RATE_LIMITER", "API_RATE_LIMITER"):
        namespace = str(limiters.get(binding, {}).get("namespace_id", ""))
        if not namespace.isdigit() or namespace in ("1001", "1002", "2001", "2002"):
            issues.append(f"{binding} must use a new rate limiter namespace")
        namespaces.append(namespace)
    if len(set(namespaces)) != 2:
        issues.append("Rate limiters must use different namespaces")
    return issues


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("config", type=Path)
    args = parser.parse_args()
    try:
        issues = check(args.config)
    except (OSError, ValueError) as error:
        print(json.dumps({"ok": False, "errors": [str(error)]}))
        return 1
    print(json.dumps({"ok": not issues, "errors": issues, "credentials_read": False}))
    return int(bool(issues))


if __name__ == "__main__":
    raise SystemExit(main())
