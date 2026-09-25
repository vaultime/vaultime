#!/usr/bin/env node

import { randomBytes, scryptSync } from "node:crypto";

function printUsage() {
  console.log(`Usage: node scripts/generate-invite-key.mjs [options]

Options:
  --count <n>              Number of invite keys to generate (default: 1)
  --prefix <value>         Invite code prefix (default: VTLINV)
  --max-redemptions <n>    Allowed redemptions per invite (default: 1)
  --expires-at <iso>       Optional ISO-8601 expiry timestamp
  --format <json|pretty>   Output mode (default: pretty)
  --help                   Show this help
`);
}

function parsePositiveInt(raw, flagName) {
  const value = Number.parseInt(raw, 10);
  if (!Number.isInteger(value) || value <= 0) {
    throw new Error(`${flagName} must be a positive integer`);
  }
  return value;
}

function parseArgs(argv) {
  const options = {
    count: 1,
    prefix: "VTLINV",
    maxRedemptions: 1,
    expiresAt: null,
    format: "pretty",
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];

    if (arg === "--help") {
      printUsage();
      process.exit(0);
    }

    const next = argv[index + 1];

    switch (arg) {
      case "--count":
        options.count = parsePositiveInt(next, "--count");
        index += 1;
        break;
      case "--prefix":
        if (!next) {
          throw new Error("--prefix requires a value");
        }
        options.prefix = next.trim().toUpperCase();
        index += 1;
        break;
      case "--max-redemptions":
        options.maxRedemptions = parsePositiveInt(next, "--max-redemptions");
        index += 1;
        break;
      case "--expires-at":
        if (!next) {
          throw new Error("--expires-at requires a value");
        }
        if (Number.isNaN(Date.parse(next))) {
          throw new Error("--expires-at must be a valid ISO-8601 timestamp");
        }
        options.expiresAt = next;
        index += 1;
        break;
      case "--format":
        if (!next || (next !== "json" && next !== "pretty")) {
          throw new Error("--format must be either json or pretty");
        }
        options.format = next;
        index += 1;
        break;
      default:
        throw new Error(`Unknown argument: ${arg}`);
    }
  }

  return options;
}

function chunkToken(token) {
  return token.match(/.{1,4}/g)?.join("-") ?? token;
}

function createInvite(prefix, maxRedemptions, expiresAt) {
  const body = randomBytes(18)
    .toString("base64url")
    .replace(/[^A-Za-z0-9]/g, "")
    .slice(0, 24)
    .toUpperCase();

  const code = `${prefix}-${chunkToken(body)}`;
  const lookupKey = body.slice(0, 12);
  const salt = randomBytes(16).toString("hex");
  const codeHash = scryptSync(code, salt, 64).toString("hex");

  return {
    code,
    lookupKey,
    salt,
    codeHash,
    maxRedemptions,
    expiresAt,
    createdAt: new Date().toISOString(),
  };
}

function printPretty(invites) {
  invites.forEach((invite, index) => {
    if (index > 0) {
      console.log("");
    }

    console.log(`Invite ${index + 1}`);
    console.log(`  code:            ${invite.code}`);
    console.log(`  lookup_key:      ${invite.lookupKey}`);
    console.log(`  salt:            ${invite.salt}`);
    console.log(`  code_hash:       ${invite.codeHash}`);
    console.log(`  max_redemptions: ${invite.maxRedemptions}`);
    console.log(`  expires_at:      ${invite.expiresAt ?? "none"}`);
    console.log(`  created_at:      ${invite.createdAt}`);
  });
}

try {
  const options = parseArgs(process.argv.slice(2));
  const invites = Array.from({ length: options.count }, () =>
    createInvite(options.prefix, options.maxRedemptions, options.expiresAt),
  );

  if (options.format === "json") {
    console.log(JSON.stringify(invites, null, 2));
  } else {
    printPretty(invites);
  }
} catch (error) {
  console.error(String(error.message ?? error));
  console.error("");
  printUsage();
  process.exit(1);
}
