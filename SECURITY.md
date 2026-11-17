# Security

Vaultime is a free project. Reports about security problems are welcome and
get an answer as soon as possible, usually within a week.

## Report a problem privately

Please do not open a public issue for a security problem. Use one of these
instead:

- [Report a vulnerability](https://github.com/vaultime/vaultime/security/advisories/new)
  on GitHub. The report stays private between you and the maintainer.
- Email [vaultime@outlook.de](mailto:vaultime@outlook.de).

Describe what you found, how to reproduce it and what it affects. Once a fix
is out, the problem is described in the changelog and, if you like, you are
named for finding it.

## What counts

- The desktop app on Windows and Linux, including its updates, which are
  signed and come from `https://vaultime.codfishcloud.de/downloads/`.
- The cloud server in `apps/api` and its setup in `deploy/server`, as
  anyone runs it.
- The website and its setup in `deploy/vps`.

Only the latest release gets fixes.

## Good to know

- The trust labels in the app, such as Suspicious and Recovered, are integrity
  scoring, not protection. Editing your own history on your own PC is not a
  vulnerability.
- There is no hosted cloud service. Test against a server you run yourself,
  as the documentation on running your own server describes, and never touch
  other people's servers, accounts or backups. Please do not run load tests
  or automated scans against the website.
