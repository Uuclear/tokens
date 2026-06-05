# Postman

- **Kind:** hybrid (desktop + cloud)
- **Status:** optional_api

## Local

No standard token log on disk.

## Optional API

```bash
cargo build --features postman
tokens config set postman_api_key <key>
tokens scan --api
```

Cloud [Agent Mode usage reports](https://learning.postman.com/docs/reports/agent-mode-usage-reports/) expose **AI credits**, not raw input/output tokens.

## Stored as

- `usage_unit = credits`
- `quality = credits`
