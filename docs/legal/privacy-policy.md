# Privacy Policy

**Effective date:** April 6, 2026

Vaultime is a local-first desktop application for tracking PC game playtime. This policy explains what data we collect, how we use it, and how we protect it.

## 1. Local-Only Data

By default, all data stays on your device:

- **Game library** — titles, executable paths, and install folders you register.
- **Session history** — playtime sessions with start/end times, active/idle/runtime durations, and integrity metadata.
- **Artwork** — cached images scanned from local game folders or manually imported.
- **Settings** — your tracking preferences (idle threshold, background-as-active toggle).
- **Device identity** — a hostname-based device identifier stored in the local database.

This data is stored in a SQLite database in your operating system's application data directory. We never access, collect, or transmit local-only data.

## 2. Cloud Features (Optional)

If you create an account and sign in, the following data is transmitted to our cloud infrastructure:

### Account Data
- **Email address** and **hashed password** — stored by Supabase Auth for authentication.
- **Device registration** — device identifier, platform, and app version sent on sign-in so the cloud service can track which devices belong to your account.

### Cloud Sync
- **Session events** — the append-only event log for your sessions is uploaded to Supabase so that sessions can receive Verified trust status via server-side acknowledgement timestamps.
- Events include: session start/end, heartbeats, and integrity metadata (hashes, monotonic timestamps). They do not include screenshots, keystrokes, or application content.

### Cloud Backup
- **Database snapshots** — a consistent copy of your local SQLite database and cached artwork is uploaded to a private Supabase Storage bucket.
- Backups are encrypted in transit (HTTPS) and stored with provider-managed encryption at rest.
- Only you can access your backups via authenticated API calls.

### Billing
- **Stripe** handles all payment processing. We do not store credit card numbers or payment details in our own infrastructure.
- Stripe receives your email address to associate payments with your account.
- A subscription record (tier, status, period end date) is stored in our Supabase database so the app can check your plan status.

## 3. Data We Do Not Collect

- No analytics or telemetry.
- No crash reports.
- No usage tracking or behavioral profiling.
- No advertising identifiers.
- No data from other applications besides process name matching for tracking.

## 4. Third-Party Services

| Service | Purpose | Data shared |
|---------|---------|-------------|
| [Supabase](https://supabase.com) | Authentication, cloud sync, cloud backup storage | Email, session events, backup snapshots, device info |
| [Stripe](https://stripe.com) | Payment processing | Email, payment method (handled entirely by Stripe) |

Both services maintain their own privacy policies and are GDPR-compliant.

## 5. Data Retention

- **Local data** is retained until you delete the app or its data directory.
- **Cloud data** is retained while your account is active. You can delete your cloud backups from within the app.
- **Stripe data** is retained per Stripe's data retention policy.

## 6. Data Deletion

- Delete local data by removing the Vaultime application data directory.
- Delete cloud backups via the Cloud page in the app.
- To request full account deletion, contact us at the email below.

## 7. Security

- All cloud communication uses HTTPS.
- Authentication tokens are stored locally on your device.
- Cloud backups are stored in private buckets with row-level security (RLS) ensuring only your authenticated account can access your data.
- Session event integrity is protected by hash chains and monotonic time validation.

## 8. Children

Vaultime is not directed at children under 13. We do not knowingly collect data from children.

## 9. Changes

We may update this policy as the product evolves. Material changes will be noted in the changelog.

## 10. Contact

For privacy questions or data deletion requests, open an issue at [github.com/schwimmbeck/vaultime](https://github.com/schwimmbeck/vaultime) or email the project maintainer.
