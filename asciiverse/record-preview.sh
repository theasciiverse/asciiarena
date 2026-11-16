#!/bin/sh
# Records the catalog preview, asciiverse/asciiarena.cast: 100x30,
# `asciiarena --demo` (six letters, all played by the computer, fighting
# through the arenas), stopped after about 50 seconds.
# Runs asciinema in a detached tmux pane, so nothing from this terminal gets
# in; a throwaway HOME keeps the settings out of the way.
#
#   cargo build --release && asciiverse/record-preview.sh
set -eu
cd "$(dirname "$0")/.."
out=asciiverse/asciiarena.cast
tmp=$(mktemp -d)
trap 'tmux -L asciiarena-rec kill-server 2>/dev/null || true; rm -rf "$tmp"' EXIT
rm -f "$out"
tmux -L asciiarena-rec new-session -d -s rec -x 100 -y 30 \
  "env -i PATH=$PWD/target/release:/usr/bin:/bin HOME=$tmp USER=player LOGNAME=player \
     TERM=xterm-256color COLORTERM=truecolor LANG=C.UTF-8 \
   asciinema rec --window-size 100x30 --idle-time-limit 2 \
     --capture-env TERM,COLORTERM,LANG --title 'AsciiArena' \
     --command 'asciiarena --demo' $out"
sleep 52
tmux -L asciiarena-rec send-keys -t rec Escape
sleep 2
ls -la "$out"
