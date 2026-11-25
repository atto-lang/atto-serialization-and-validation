#!/usr/bin/env bash
# Run from reviewed repository root. Synthetic checkouts only.
set -euo pipefail
root="$(pwd -P)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
rustc --edition=2021 -D warnings "$root/scripts/verify-zorde-checkout.rs" -o "$tmp/verify"
rustc --edition=2021 -D warnings --test "$root/scripts/verify-zorde-checkout.rs" -o "$tmp/tests"
"$tmp/tests"
src="$tmp/source with spaces"
mkdir -p "$src/nested"
git -C "$src" init -q
git -C "$src" config user.name test
git -C "$src" config user.email test@example.invalid
printf 'original\n' > "$src/nested/file"
printf 'ignored*\n' > "$src/.gitignore"
git -C "$src" add .
git -C "$src" commit -qm baseline
head="$(git -C "$src" rev-parse HEAD)"
tree="$(git -C "$src" rev-parse 'HEAD^{tree}')"
count=0
ok() {
  (cd "$src"; "$tmp/verify" "$head" "$tree")
  count=$((count+1))
}
deny() {
  local status=0
  (cd "$src"; "$tmp/verify" "$1" "$2") >"$tmp/log" 2>&1 || status=$?
  test "$status" = 2
  grep -Fq "$3" "$tmp/log" || { cat "$tmp/log"; exit 1; }
  count=$((count+1))
}
ok
deny 0000000000000000000000000000000000000000 "$tree" 'HEAD does not equal reviewed commit'
deny "$head" 0000000000000000000000000000000000000000 'root tree does not equal reviewed tree'
git -C "$src" update-index --assume-unchanged nested/file
printf 'mutation\n' > "$src/nested/file"
deny "$head" "$tree" 'working-tree blob differs'
git -C "$src" update-index --no-assume-unchanged nested/file
git -C "$src" restore nested/file
git -C "$src" update-index --skip-worktree nested/file
printf 'mutation\n' > "$src/nested/file"
deny "$head" "$tree" 'working-tree blob differs'
git -C "$src" update-index --no-skip-worktree nested/file
git -C "$src" restore nested/file
touch "$src/ignored-file"
deny "$head" "$tree" 'unexpected physical file'
rm "$src/ignored-file"
touch "$src/extra"
git -C "$src" add extra
deny "$head" "$tree" 'unexpected physical file'
git -C "$src" rm --cached extra >/dev/null
rm "$src/extra"
chmod +x "$src/nested/file"
deny "$head" "$tree" 'tracked executable mode changed'
chmod -x "$src/nested/file"
mv "$src/nested" "$tmp/real"
ln -s "$tmp/real" "$src/nested"
deny "$head" "$tree" 'tracked parent directory is not a real directory'
rm "$src/nested"
mv "$tmp/real" "$src/nested"
mv "$src/nested/file" "$tmp/original"
ln -s "$tmp/original" "$src/nested/file"
deny "$head" "$tree" 'unexpected physical file'
rm "$src/nested/file"
mkfifo "$src/nested/file"
deny "$head" "$tree" 'unexpected physical file'
rm "$src/nested/file"
mv "$tmp/original" "$src/nested/file"
ok
alternate="$(printf 'alternate\n' | git -C "$src" commit-tree "$tree" -p "$head")"
git -C "$src" update-ref HEAD "$alternate"
deny "$head" "$tree" 'HEAD does not equal reviewed commit'
printf 'Rust source integrity synthetic CLI controls: %s PASS (no compiler executed)\n' "$count"
