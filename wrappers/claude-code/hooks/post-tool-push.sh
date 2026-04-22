#!/usr/bin/env bash
# Auto-push to no-tickets after tool use if .notickets/ has entities.
# Errors are logged to stderr so users can diagnose issues.

if npx no-tickets push --dry-run 2>&1 | grep -q '"entities"'; then
  npx no-tickets push
fi
