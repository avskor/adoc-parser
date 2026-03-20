#!/usr/bin/env bash
# Generate .expected.html files from .adoc fixtures using Asciidoctor.
# Usage: bash adoc-html-tests/scripts/generate-expected.sh
#
# Requires: asciidoctor (gem install asciidoctor)
# Uses `-e` (embedded) mode — outputs body content only, matching adoc_html::to_html().

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FIXTURES_DIR="$SCRIPT_DIR/../fixtures"

if ! command -v asciidoctor &>/dev/null; then
    echo "ERROR: asciidoctor not found. Install with: gem install asciidoctor" >&2
    exit 1
fi

count=0
for adoc_file in $(find "$FIXTURES_DIR" -name '*.adoc' | sort); do
    expected_file="${adoc_file%.adoc}.expected.html"
    asciidoctor -e -o - "$adoc_file" > "$expected_file"
    echo "Generated: $expected_file"
    count=$((count + 1))
done

echo "Done. Generated $count expected HTML files."
