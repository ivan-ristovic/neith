#!/bin/bash

if [ -z "${SHLIB_ROOT+x}" ]; then
    declare SHLIB_ROOT
    SHLIB_ROOT="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
    export SHLIB_ROOT
fi

for __src in "$SHLIB_ROOT"/*.sh; do
    if [ "$(basename "$__src")" != "lib.sh" ]; then
        source "$__src"
    fi
done
unset __src

