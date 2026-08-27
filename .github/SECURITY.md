# Security policy

AutoSubs can read and write mounted media, launch FFmpeg, and send audio/text to transcription or LLM endpoints configured by the administrator. Treat an AutoSubs instance as an administrative service, not a public upload endpoint.

## Supported versions

Security fixes are applied to the latest released `3.x` version and to `main`. Older development snapshots are not supported.

## Reporting a vulnerability

Do **not** open a public issue for a vulnerability or include credentials, private media paths, API keys, tokens, or unredacted logs in a report.

Use GitHub's private vulnerability reporting for this repository:

`Security` → `Report a vulnerability`

Include the affected version/image digest, a minimal reproduction, impact, and any mitigation you already tested. Acknowledgement and remediation discussion will happen in the private advisory.

## Deployment boundary

AutoSubs does not provide Internet-facing authentication. Keep it on a trusted network or place it behind an authenticated reverse proxy/VPN. Mount only the media paths it needs. Keep `/config` on local storage and back it up like any other application database.
