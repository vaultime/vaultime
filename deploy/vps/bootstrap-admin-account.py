#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
# SPDX-License-Identifier: AGPL-3.0-or-later
"""Creates the first Vaultime admin account through the running API."""

from __future__ import annotations

import argparse
import getpass
import importlib.util
import json
import sys
import urllib.error
import urllib.request
from pathlib import Path
from types import ModuleType


DEFAULT_ENV_FILE = "/etc/vaultime/api.env"
# Same as MIN_PASSWORD_CHARS in apps/api/src/constants.rs.
MIN_PASSWORD_CHARS = 10
# Same as api_addr in install-api.sh, where the API listens.
DEFAULT_API_BASE_URL = "http://127.0.0.1:9005"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Create or promote the first admin account for Vaultime.",
    )
    parser.add_argument("--email", required=True)
    parser.add_argument(
        "--password",
        help="Admin password. Prompted for when omitted, so it stays out of shell history.",
    )
    parser.add_argument("--api-base-url", default=DEFAULT_API_BASE_URL)
    parser.add_argument("--env-file", default=DEFAULT_ENV_FILE)
    parser.add_argument("--db-url", help="Override database URL instead of reading api.env")
    return parser.parse_args()


def load_invite_tool() -> ModuleType:
    """generate-cloud-invite.py, which install-api.sh puts next to this
    script. Invites and database access work the same way in both."""
    path = Path(__file__).resolve().with_name("generate-cloud-invite.py")
    spec = importlib.util.spec_from_file_location("generate_cloud_invite", path)
    if not path.is_file() or spec is None or spec.loader is None:
        raise SystemExit(f"{path} is missing")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


invite_tool = load_invite_tool()
load_env_file = invite_tool.load_env_file
run_psql = invite_tool.run_psql
sql_literal = invite_tool.sql_literal


def normalize_email(raw: str) -> str:
    email = raw.strip().lower()
    if not email or "@" not in email:
        raise SystemExit("email must be valid")
    return email


def http_post_json(url: str, payload: dict[str, str]) -> dict[str, object]:
    request = urllib.request.Request(
        url,
        data=json.dumps(payload).encode("utf-8"),
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    try:
        with urllib.request.urlopen(request) as response:
            return json.loads(response.read().decode("utf-8"))
    except urllib.error.HTTPError as exc:
        body = exc.read().decode("utf-8", errors="replace")
        raise SystemExit(f"{url} returned {exc.code}: {body}") from exc


def main() -> int:
    args = parse_args()
    email = normalize_email(args.email)
    password = args.password or getpass.getpass("Admin password: ")
    if len(password) < MIN_PASSWORD_CHARS:
        raise SystemExit(f"password must be at least {MIN_PASSWORD_CHARS} characters")

    database_url = args.db_url
    if not database_url:
        env = load_env_file(args.env_file)
        database_url = env.get("VAULTIME_DATABASE_URL")
    if not database_url:
        raise SystemExit("could not determine database URL")

    existing_id = run_psql(
        database_url,
        f"SELECT id::text FROM cloud_accounts WHERE email = {sql_literal(email)} LIMIT 1;",
    )
    if existing_id:
        run_psql(
            database_url,
            f"""
            UPDATE cloud_accounts
            SET role = 'admin',
                access_state = 'active',
                access_granted_at = COALESCE(access_granted_at, NOW())
            WHERE email = {sql_literal(email)};
            """,
        )

        try:
            http_post_json(
                args.api_base_url.rstrip("/") + "/v1/auth/login",
                {"email": email, "password": password},
            )
            print(f"Admin account already existed and is now marked admin: {email}")
            return 0
        except SystemExit:
            print(f"Admin role was granted to existing account: {email}")
            print("Password was not changed because the account already existed.")
            print("If you need a known password, delete that account and rerun this script.")
            return 1

    invite = invite_tool.generate_invite(
        invite_tool.INVITE_PREFIX, 1, None, "temporary bootstrap invite for admin account"
    )
    invite_tool.insert_invite(database_url, invite)

    http_post_json(
        args.api_base_url.rstrip("/") + "/v1/auth/signup",
        {
            "email": email,
            "password": password,
            "invite_code": invite["code"],
        },
    )

    run_psql(
        database_url,
        f"""
        UPDATE cloud_accounts
        SET role = 'admin',
            access_state = 'active',
            access_granted_at = COALESCE(access_granted_at, NOW())
        WHERE email = {sql_literal(email)};
        """,
    )

    print(f"Admin account created: {email}")
    print("The account can now call POST /v1/admin/invites after login.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
