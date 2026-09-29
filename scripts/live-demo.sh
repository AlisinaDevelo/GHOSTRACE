#!/usr/bin/env bash
# Guided GHOSTRACE live demo. Everything happens in a temporary workspace with
# its own GHOSTRACE home, key, folder, and Git repository; nothing uses the
# network or your own files, and the key, home, and workspace are deleted on
# exit, whatever happens. macOS only (login keychain).
#
#   scripts/live-demo.sh [path/to/ghostrace]    default: target/release/ghostrace
set -euo pipefail

G=${1:-target/release/ghostrace}
[ -x "$G" ] || {
	echo "Build first: cargo build --release --locked" >&2
	exit 1
}
G=$(cd "$(dirname "$G")" && pwd)/$(basename "$G")

WORK=$(mktemp -d)
HOME_DIR="$WORK/home"
FOLDER="$WORK/project"
REPO="$WORK/repo"
cleanup() {
	"$G" live forget --home "$HOME_DIR" --yes >/dev/null 2>&1 || true
	rm -rf "$WORK"
}
trap cleanup EXIT

# Show paths as placeholders so the transcript never contains this machine's
# temporary directory.
show() { sed -e "s|$WORK|<workspace>|g"; }
say() { printf '\n## %s\n' "$*"; }
cmd() {
	local shown
	shown=$(printf '%q ' "$@")
	printf '$ ghostrace %s\n' "$(printf '%s' "${shown% }" | show)"
	"$G" "$@" 2>&1 | show
}
# The demo repository ignores your own Git configuration entirely.
export GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1
git_quiet() { git -C "$REPO" -c user.name=demo -c user.email=demo@example.invalid "$@" >/dev/null; }

say "1. Create a private home with its key in the login keychain"
cmd live init --home "$HOME_DIR"

say "2. Nothing runs through GHOSTRACE until you consent"
cmd run --home "$HOME_DIR" -- /usr/bin/true || true
cmd live consent-shell --home "$HOME_DIR" --yes

say "3. Wrapped commands keep their exit codes; arguments are never recorded"
cmd run --home "$HOME_DIR" -- /bin/sh -c 'exit 0' DEMO_SECRET=abc123 || true
cmd run --home "$HOME_DIR" -- /bin/sh -c 'exit 3' || echo "(exit code $?)"
cmd run --home "$HOME_DIR" -- /nonexistent/tool || echo "(exit code $?)"

say "4. Watch one folder while files are created, renamed, and deleted"
mkdir -p "$FOLDER"
"$G" live watch "$FOLDER" --home "$HOME_DIR" --yes --seconds 5 2>&1 | show | tail -1 &
watcher=$!
sleep 1.5
echo draft >"$FOLDER/secret-plan.txt"
echo notes >"$FOLDER/notes.md"
mv "$FOLDER/secret-plan.txt" "$FOLDER/final-plan.txt"
rm "$FOLDER/notes.md"
wait $watcher

say "5. Snapshot a Git repository, commit, then rewrite history"
git -c init.templateDir= init -q -b demo-branch "$REPO"
echo one >"$REPO/demo-file.txt"
git_quiet add .
git_quiet commit -m first
cmd live git-snapshot --home "$HOME_DIR" "$REPO"
echo two >>"$REPO/demo-file.txt"
git_quiet commit -am second
cmd live git-snapshot --home "$HOME_DIR" "$REPO"
git_quiet commit --amend -m rewritten
cmd live git-snapshot --home "$HOME_DIR" "$REPO"

say "6. What the journal holds"
cmd live status --home "$HOME_DIR"
cmd live timeline --home "$HOME_DIR" --limit 100

say "7. An offline HTML report"
cmd live report --home "$HOME_DIR" --yes --output "$WORK/timeline.html"

say "8. Nothing sensitive was recorded"
"$G" live timeline --home "$HOME_DIR" --limit 1000 --json >"$WORK/all.txt"
cat "$WORK/timeline.html" >>"$WORK/all.txt"
for needle in DEMO_SECRET abc123 secret-plan final-plan notes.md demo-branch demo-file "$WORK"; do
	printf '%-14s %s\n' "$(printf '%s' "$needle" | show)" "$(grep -c -- "$needle" "$WORK/all.txt" || true) match(es)"
done

say "9. What GHOSTRACE did not observe"
cat <<'EOF'
- Anything run without `ghostrace run`, and the arguments, environment, and output
  of what was run through it.
- Which old file name became which new one: macOS reports a rename as two events,
  so the pairing stays contextual.
- File contents and readable file names: only salted digests are kept.
- Which program changed a file: FSEvents does not say.
- Changes macOS coalesced: a file created and deleted within one batch can appear
  as a single event, as notes.md may here.
- The commit that `--amend` replaced: it is gone from history, so a
  history_rewritten gap marks the break instead of a guess.
- Anything outside the one watched folder, or while nothing was running.
EOF

say "10. Forget: delete the key and the home"
cmd live forget --home "$HOME_DIR" --yes
