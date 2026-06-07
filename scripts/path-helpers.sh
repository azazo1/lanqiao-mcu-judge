#!/usr/bin/env bash

expand_tilde_path() {
    local value="${1:-}"
    case "$value" in
        "~")
            printf '%s\n' "$HOME"
            ;;
        "~/"*)
            printf '%s/%s\n' "$HOME" "${value:2}"
            ;;
        *)
            printf '%s\n' "$value"
            ;;
    esac
}
