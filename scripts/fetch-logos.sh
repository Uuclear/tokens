#!/usr/bin/env bash
# Re-download bundled platform logos and generate theme variants.
# Run from repo root: ./scripts/fetch-logos.sh

set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DIR="$ROOT/src/serve/assets/logos"
mkdir -p "$DIR"

curl -fsSL -o "$DIR/claude_code.png" "https://claude.ai/images/claude_app_icon.png"
curl -fsSL -o "$DIR/cursor.png" "https://www.cursor.com/apple-touch-icon.png"
curl -fsSL -o "$DIR/opencode.ico" "https://opencode.ai/favicon.ico"
curl -fsSL -o "$DIR/hermes.ico" "https://www.nousresearch.com/favicon.ico"
curl -fsSL -o "$DIR/chatbox.ico" "https://chatboxai.app/favicon.ico"
curl -fsSL -o "$DIR/postman.ico" "https://www.postman.com/favicon.ico"

for pair in \
  "codex:openai.com" \
  "qwen_code:qwen.ai" \
  "openclaw:openclaw.ai" \
  "cherry_studio:cherry-ai.com" \
  "dify:dify.ai" \
  "qoder:qoder.com" \
  "cline:cline.bot" \
  "kilo_cli:kilocode.ai"; do
  name="${pair%%:*}"
  domain="${pair##*:}"
  curl -fsSL -o "$DIR/${name}.png" "https://www.google.com/s2/favicons?domain=${domain}&sz=128"
done

cp -f "$DIR/kilo_cli.png" "$DIR/kilo_ide.png" 2>/dev/null || true
(cd "$ROOT" && cargo run --bin generate_theme_logos --features logo-gen)
echo "Done. Rebuild: cargo build"
