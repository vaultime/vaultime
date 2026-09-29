#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
# SPDX-License-Identifier: AGPL-3.0-or-later
"""Creates the first Vaultime admin account through the running API."""

from __future__ import annotations

import argparse
import base64
import getpass
import hashlib
import json
import os
import secrets
import subprocess
import sys
import urllib.error
import urllib.request
from urllib.parse import unquote, urlparse


DEFAULT_ENV_FILE = "/etc/vaultime/api.env"
# Invite codes, same as INVITE_* in apps/api/src/constants.rs. Changing the
# scrypt settings breaks every stored invite hash.
INVITE_PREFIX = "VTLINV"
INVITE_BODY_RANDOM_BYTES = 18
INVITE_BODY_CHARS = 24
INVITE_LOOKUP_KEY_CHARS = 12
INVITE_CODE_GROUP_CHARS = 4
INVITE_SALT_BYTES = 16
INVITE_SCRYPT_N = 2**14
INVITE_SCRYPT_R = 8
INVITE_SCRYPT_P = 1
INVITE_HASH_BYTES = 64
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


def load_env_file(path: str) -> dict[str, str]:
    result: dict[str, str] = {}
    with open(path, "r", encoding="utf-8") as handle:
        for line in handle:
            line = line.strip()
            if not line or line.startswith("#") or "=" not in line:
                continue
            key, value = line.split("=", 1)
            result[key] = value
    return result


def sql_literal(value: str | None) -> str:
    if value is None:
        return "NULL"
    return "'" + value.replace("'", "''") + "'"


def run_psql(database_url: str, sql: str) -> str:
    """Runs SQL through psql. The password goes in the environment and the SQL
    on stdin, so neither shows up in the process list."""
    parsed = urlparse(database_url)
    port = f":{parsed.port}" if parsed.port else ""
    target = f"{parsed.scheme}://{parsed.username or ''}@{parsed.hostname or 'localhost'}{port}{parsed.path}"
    env = dict(os.environ, PGPASSWORD=unquote(parsed.password or ""))
    result = subprocess.run(
        ["psql", target, "-v", "ON_ERROR_STOP=1", "-At"],
        input=sql,
        env=env,
        check=True,
        text=True,
        capture_output=True,
    )
    return result.stdout.strip()


def normalize_email(raw: str) -> str:
    email = raw.strip().lower()
    if not email or "@" not in email:
        raise SystemExit("email must be valid")
    return email


def chunk_token(token: str) -> str:
    return "-".join(
        token[index : index + INVITE_CODE_GROUP_CHARS]
        for index in range(0, len(token), INVITE_CODE_GROUP_CHARS)
    )


def generate_body_token() -> str:
    while True:
        raw = base64.urlsafe_b64encode(secrets.token_bytes(INVITE_BODY_RANDOM_BYTES)).decode("ascii").rstrip("=")
        body = "".join(ch for ch in raw if ch.isalnum()).upper()
        if len(body) >= INVITE_BODY_CHARS:
            return body[:INVITE_BODY_CHARS]


def hash_invite(code: str, salt: str) -> str:
    return hashlib.scrypt(
        code.encode("utf-8"),
        salt=salt.encode("utf-8"),
        n=INVITE_SCRYPT_N,
        r=INVITE_SCRYPT_R,
        p=INVITE_SCRYPT_P,
        dklen=INVITE_HASH_BYTES,
    ).hex()


def generate_bootstrap_invite() -> dict[str, str]:
    body = generate_body_token()
    code = f"{INVITE_PREFIX}-{chunk_token(body)}"
    salt = secrets.token_hex(INVITE_SALT_BYTES)
    return {
        "code": code,
        "lookup_key": body[:INVITE_LOOKUP_KEY_CHARS],
        "salt": salt,
        "code_hash": hash_invite(code, salt),
    }


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

    invite = generate_bootstrap_invite()
    run_psql(
        database_url,
        f"""
        INSERT INTO cloud_invites (
            lookup_key,
            salt,
            code_hash,
            max_redemptions,
            note
        ) VALUES (
            {sql_literal(invite['lookup_key'])},
            {sql_literal(invite['salt'])},
            {sql_literal(invite['code_hash'])},
            1,
            'temporary bootstrap invite for admin account'
        );
        """,
    )

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
