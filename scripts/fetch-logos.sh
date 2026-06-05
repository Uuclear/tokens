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

for pair in \
  "codex:openai.com" \
  "qwen_code:qwen.ai" \
  "openclaw:openclaw.ai" \
  "cherry_studio:cherry-ai.com" \
  "dify:dify.ai" \
  "qoder:qoder.com" \
  "cline:cline.bot" \
  "kilo_cli:kilocode.ai" \
  "hermes:nousresearch.com" \
  "chatbox:chatboxai.app" \
  "postman:postman.com" \
  "pi:pi.dev"; do
  name="${pair%%:*}"
  domain="${pair##*:}"
  curl -fsSL -o "$DIR/${name}.png" "https://www.google.com/s2/favicons?domain=${domain}&sz=128"
done

cp -f "$DIR/kilo_cli.png" "$DIR/kilo_ide.png" 2>/dev/null || true
cp -f "$DIR/qoder.png" "$DIR/qoder_cn.png" 2>/dev/null || true
# ICO favicons are often multi-frame; rasterize for theme generation.
if command -v convert >/dev/null 2>&1; then
  for ico in opencode chatbox; do
    if [[ -f "$DIR/${ico}.ico" ]]; then
      convert "$DIR/${ico}.ico[0]" -resize 128x128 "$DIR/${ico}.png" 2>/dev/null || true
    fi
  done
elif python3 -c "import PIL" 2>/dev/null; then
  python3 - "$DIR" <<'PY'
import sys
from pathlib import Path
from PIL import Image
root = Path(sys.argv[1])
for ico in ("opencode", "chatbox"):
    src = root / f"{ico}.ico"
    if src.exists():
        try:
            Image.open(src).save(root / f"{ico}.png")
        except OSError:
            pass
PY
fi
(cd "$ROOT" && cargo run --bin generate_theme_logos --features logo-gen)
echo "Done. Rebuild: cargo build"
