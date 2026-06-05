# Dify

- **Kind:** server
- **Status:** optional_api

## Self-hosted

PostgreSQL `messages` columns: `message_tokens`, `answer_tokens`, `total_price`

## Optional API

```bash
cargo build --features dify
tokens config set dify_api_url https://your-dify/v1
tokens config set dify_api_key <key>
tokens scan --api
```

## UI logs

Per-conversation token counts in admin Logs UI; Web App bulk export remains limited (see upstream issues).

## Quality

`exact` via DB/API when configured
