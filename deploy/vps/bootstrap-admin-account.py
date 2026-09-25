#!/usr/bin/env python3
"""Bootstrap the first Vaultime admin account against the live self-hosted API."""

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
from urllib.parse import urlparse


DEFAULT_ENV_FILE = "/etc/vaultime/api.env"
DEFAULT_API_BASE_URL = "http://127.0.0.1:9005"
DEFAULT_PREFIX = "VTLINV"


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


def psql(database_url: str, sql: str, capture: bool = False) -> str:
    command = ["psql", database_url, "-v", "ON_ERROR_STOP=1", "-At", "-c", sql]
    result = subprocess.run(
        command,
        check=True,
        text=True,
        capture_output=capture,
    )
    return result.stdout.strip() if capture else ""


def parse_database_url(database_url: str) -> tuple[str, str]:
    parsed = urlparse(database_url)
    database_name = parsed.path.lstrip("/")
    if not database_name:
        raise SystemExit("database URL is missing a database name")
    database_user = parsed.username or "vaultime"
    return database_name, database_user


def psql_as_postgres(database_name: str, sql: str, capture: bool = False) -> str:
    result = subprocess.run(
        ["runuser", "-u", "postgres", "--", "psql", "-d", database_name, "-v", "ON_ERROR_STOP=1", "-At", "-c", sql],
        check=True,
        text=True,
        capture_output=capture,
    )
    return result.stdout.strip() if capture else ""


def ensure_table_ownership(database_name: str, database_user: str) -> None:
    psql_as_postgres(
        database_name,
        f"""
        ALTER TABLE IF EXISTS cloud_invites OWNER TO {database_user};
        ALTER TABLE IF EXISTS cloud_accounts OWNER TO {database_user};
        ALTER TABLE IF EXISTS cloud_account_passwords OWNER TO {database_user};
        ALTER TABLE IF EXISTS cloud_refresh_tokens OWNER TO {database_user};
        ALTER TABLE IF EXISTS cloud_invite_redemptions OWNER TO {database_user};
        ALTER TABLE IF EXISTS cloud_devices OWNER TO {database_user};
        ALTER TABLE IF EXISTS cloud_backups OWNER TO {database_user};
        """,
    )


def ensure_role_schema(database_url: str) -> None:
    database_name, database_user = parse_database_url(database_url)

    if os.geteuid() == 0:
        ensure_table_ownership(database_name, database_user)
        psql_as_postgres(
            database_name,
            """
            ALTER TABLE cloud_accounts
                ADD COLUMN IF NOT EXISTS role TEXT NOT NULL DEFAULT 'user';

            DO $$
            BEGIN
                IF NOT EXISTS (
                    SELECT 1
                    FROM pg_constraint
                    WHERE conname = 'cloud_accounts_role_check'
                ) THEN
                    ALTER TABLE cloud_accounts
                        ADD CONSTRAINT cloud_accounts_role_check
                        CHECK (role IN ('user', 'admin'));
                END IF;
            END
            $$;
            """,
        )
        return

    psql(
        database_url,
        """
        ALTER TABLE cloud_accounts
            ADD COLUMN IF NOT EXISTS role TEXT NOT NULL DEFAULT 'user';

        DO $$
        BEGIN
            IF NOT EXISTS (
                SELECT 1
                FROM pg_constraint
                WHERE conname = 'cloud_accounts_role_check'
            ) THEN
                ALTER TABLE cloud_accounts
                    ADD CONSTRAINT cloud_accounts_role_check
                    CHECK (role IN ('user', 'admin'));
            END IF;
        END
        $$;
        """,
    )


def normalize_email(raw: str) -> str:
    email = raw.strip().lower()
    if not email or "@" not in email:
        raise SystemExit("email must be valid")
    return email


def generate_body_token() -> str:
    while True:
        raw = base64.urlsafe_b64encode(secrets.token_bytes(18)).decode("ascii").rstrip("=")
        body = "".join(ch for ch in raw if ch.isalnum()).upper()
        if len(body) >= 24:
            return body[:24]


def chunk_token(token: str) -> str:
    return "-".join(token[index : index + 4] for index in range(0, len(token), 4))


def generate_bootstrap_invite() -> dict[str, str]:
    body = generate_body_token()
    code = f"{DEFAULT_PREFIX}-{chunk_token(body)}"
    lookup_key = body[:12]
    salt = secrets.token_hex(16)
    code_hash = hashlib.scrypt(
        code.encode("utf-8"),
        salt=salt.encode("utf-8"),
        n=16384,
        r=8,
        p=1,
        dklen=64,
    ).hex()
    return {
        "code": code,
        "lookup_key": lookup_key,
        "salt": salt,
        "code_hash": code_hash,
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
    if len(password) < 10:
        raise SystemExit("password must be at least 10 characters")

    database_url = args.db_url
    if not database_url:
        env = load_env_file(args.env_file)
        database_url = env.get("VAULTIME_DATABASE_URL")
    if not database_url:
        raise SystemExit("could not determine database URL")

    ensure_role_schema(database_url)

    existing_id = psql(
        database_url,
        f"SELECT id::text FROM cloud_accounts WHERE email = {sql_literal(email)} LIMIT 1;",
        capture=True,
    )
    if existing_id:
        psql(
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
    psql(
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

    psql(
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
