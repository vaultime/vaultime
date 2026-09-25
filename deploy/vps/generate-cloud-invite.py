#!/usr/bin/env python3
"""Generate and store invite-only cloud backup keys on the VPS."""

from __future__ import annotations

import argparse
import base64
import datetime as dt
import hashlib
import json
import os
import secrets
import subprocess
import sys
from typing import Any


DEFAULT_ENV_FILE = "/etc/vaultime/api.env"
DEFAULT_PREFIX = "VTLINV"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Generate invite codes and insert them into Vaultime PostgreSQL.",
    )
    parser.add_argument("--count", type=positive_int, default=1)
    parser.add_argument("--prefix", default=DEFAULT_PREFIX)
    parser.add_argument("--max-redemptions", type=positive_int, default=1)
    parser.add_argument("--expires-at", help="Optional ISO-8601 expiry timestamp")
    parser.add_argument("--note", default="")
    parser.add_argument("--format", choices=("pretty", "json"), default="pretty")
    parser.add_argument("--env-file", default=DEFAULT_ENV_FILE)
    parser.add_argument("--db-url", help="Override database URL instead of reading api.env")
    parser.add_argument("--skip-insert", action="store_true")
    return parser.parse_args()


def positive_int(value: str) -> int:
    parsed = int(value)
    if parsed <= 0:
        raise argparse.ArgumentTypeError("value must be a positive integer")
    return parsed


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


def normalize_prefix(raw: str) -> str:
    prefix = raw.strip().upper()
    if not prefix:
        raise SystemExit("invite prefix cannot be empty")
    if not prefix.isalnum():
        raise SystemExit("invite prefix must be alphanumeric")
    return prefix


def normalize_expiry(raw: str | None) -> str | None:
    if raw is None:
        return None
    value = raw.strip()
    if not value:
        return None
    try:
        parsed = dt.datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError as exc:
        raise SystemExit(f"invalid --expires-at value: {exc}") from exc
    if parsed.tzinfo is None:
        parsed = parsed.replace(tzinfo=dt.timezone.utc)
    return parsed.astimezone(dt.timezone.utc).isoformat().replace("+00:00", "Z")


def chunk_token(token: str) -> str:
    return "-".join(token[index : index + 4] for index in range(0, len(token), 4))


def generate_body_token() -> str:
    while True:
        raw = base64.urlsafe_b64encode(secrets.token_bytes(18)).decode("ascii").rstrip("=")
        body = "".join(ch for ch in raw if ch.isalnum()).upper()
        if len(body) >= 24:
            return body[:24]


def generate_invite(prefix: str, max_redemptions: int, expires_at: str | None, note: str) -> dict[str, Any]:
    body = generate_body_token()
    code = f"{prefix}-{chunk_token(body)}"
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
        "max_redemptions": max_redemptions,
        "expires_at": expires_at,
        "note": note.strip() or None,
        "created_at": dt.datetime.now(dt.timezone.utc).isoformat().replace("+00:00", "Z"),
    }


def sql_literal(value: Any) -> str:
    if value is None:
        return "NULL"
    text = str(value).replace("'", "''")
    return f"'{text}'"


def insert_invite(database_url: str, invite: dict[str, Any]) -> None:
    sql = f"""
    INSERT INTO cloud_invites (
        lookup_key,
        salt,
        code_hash,
        max_redemptions,
        expires_at,
        note,
        created_at
    ) VALUES (
        {sql_literal(invite["lookup_key"])},
        {sql_literal(invite["salt"])},
        {sql_literal(invite["code_hash"])},
        {invite["max_redemptions"]},
        {sql_literal(invite["expires_at"])},
        {sql_literal(invite["note"])},
        {sql_literal(invite["created_at"])}
    );
    """
    subprocess.run(
        ["psql", database_url, "-v", "ON_ERROR_STOP=1", "-c", sql],
        check=True,
        stdout=subprocess.DEVNULL,
    )


def print_pretty(invites: list[dict[str, Any]], inserted: bool) -> None:
    for index, invite in enumerate(invites, start=1):
        if index > 1:
            print()
        print(f"Invite {index}")
        print(f"  code:            {invite['code']}")
        print(f"  lookup_key:      {invite['lookup_key']}")
        print(f"  salt:            {invite['salt']}")
        print(f"  code_hash:       {invite['code_hash']}")
        print(f"  max_redemptions: {invite['max_redemptions']}")
        print(f"  expires_at:      {invite['expires_at'] or 'none'}")
        print(f"  note:            {invite['note'] or 'none'}")
        print(f"  created_at:      {invite['created_at']}")
        print(f"  inserted:        {'yes' if inserted else 'no'}")


def main() -> int:
    args = parse_args()
    prefix = normalize_prefix(args.prefix)
    expires_at = normalize_expiry(args.expires_at)

    database_url = args.db_url
    if not database_url:
        env = load_env_file(args.env_file)
        database_url = env.get("VAULTIME_DATABASE_URL")
    if not database_url:
        raise SystemExit("could not determine database URL")

    invites = [
        generate_invite(prefix, args.max_redemptions, expires_at, args.note)
        for _ in range(args.count)
    ]

    if not args.skip_insert:
        for invite in invites:
            insert_invite(database_url, invite)

    if args.format == "json":
        print(json.dumps(invites, indent=2))
    else:
        print_pretty(invites, inserted=not args.skip_insert)

    return 0


if __name__ == "__main__":
    sys.exit(main())
