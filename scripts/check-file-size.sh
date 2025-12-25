#!/bin/bash
# Check that modified Rust source files are <= 100 lines
MAX_LINES=100
VIOLATIONS=0

# Get modified/added .rs files (staged for pre-commit, or vs main for CI)
if [ -n "$CI" ]; then
    # In CI: compare against the base branch
    FILES=$(git diff --name-only --diff-filter=ACMR origin/main...HEAD -- '*.rs' 2>/dev/null || git diff --name-only --diff-filter=ACMR HEAD~1 -- '*.rs')
else
    # Local: check staged files
    FILES=$(git diff --cached --name-only --diff-filter=ACMR -- '*.rs')
fi

for file in $FILES; do
    if [ -f "$file" ]; then
        lines=$(wc -l < "$file")
        if [ "$lines" -gt "$MAX_LINES" ]; then
            echo "ERROR: $file has $lines lines (max: $MAX_LINES)"
            VIOLATIONS=$((VIOLATIONS + 1))
        fi
    fi
done

if [ "$VIOLATIONS" -gt 0 ]; then
    echo "Found $VIOLATIONS file(s) exceeding $MAX_LINES lines"
    exit 1
fi

echo "All modified files are within the $MAX_LINES line limit"
exit 0
