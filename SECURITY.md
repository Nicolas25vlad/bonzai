# Security Policy

Bonzai is an early-stage local terminal application. It does not expose a network service and does not intentionally send telemetry or plant data anywhere.

## Reporting a vulnerability

Please avoid publishing exploit details in a public issue before a fix is available.

For security-sensitive reports, contact the repository owner privately through their GitHub profile or another private contact method they explicitly publish.

Include enough information to reproduce the problem, but do not include unrelated secrets, credentials or personal data.

## Scope

Security-relevant areas currently include:

- Unix socket permissions and lifecycle
- state-file handling
- installer behavior
- daemon process management
- terminal escape handling
- future integrations that inspect local developer activity

Bonzai is currently early alpha software and has not undergone a formal security audit.
