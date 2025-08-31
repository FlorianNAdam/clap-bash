#!/usr/bin/env bash

echo "=== Command-line Arguments ==="
for arg in "$@"; do
    echo "$arg"
done

echo ""
echo "=== Environment Variables ==="
env

