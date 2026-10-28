# Security policy

VARden handles API keys, passwords and the PATH your system runs programs from, so security reports are taken seriously and handled first.

## Reporting a vulnerability

**Please don't open a public issue.** Instead, use GitHub's private reporting: on the repository page, go to **Security > Report a vulnerability**. Only the maintainer can see the report.

Include what you can:

- what an attacker could do, and what they need first (for example, a crafted `.env` file the user imports);
- steps or a small proof of concept;
- the VARden version and your operating system.

You'll get a reply within 7 days. Fixes for confirmed issues are released as soon as they're ready, and you're credited in the release notes unless you'd rather not be.

## Supported versions

Only the latest release gets security fixes. VARden updates itself, so staying current is one click.

## What's in scope

- Anything that lets a value, file or update run code it shouldn't (escaping in startup files and terminal-sync scripts, update verification).
- Values leaking somewhere they shouldn't (logs, problem reports, files readable by other accounts).
- VARden writing outside the user's own variables, or anywhere it didn't say it would.

Out of scope: problems that need an attacker to already control your user account. Anyone with that access can change your environment without VARden.
