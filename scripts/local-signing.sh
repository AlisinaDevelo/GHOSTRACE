#!/usr/bin/env bash
# A free, local code-signing identity for GHOSTRACE builds without a
# Developer ID. Signing every build with the same certificate keeps the
# binary's designated requirement stable, so the login-keychain item that
# holds the journal key does not ask for approval again after a rebuild.
#
#   create  make the identity in your login keychain (idempotent)
#   trust   mark it trusted for code signing; macOS asks for your password
#   sign    sign a built binary (default: target/release/ghostrace)
#   status  show whether the identity exists and is valid for signing
#   remove  delete the trust setting and the identity
#
# The private key is generated in a private temporary directory, imported
# into the login keychain, and the temporary copy is deleted; it is never
# written anywhere else. This is not Developer ID signing: Gatekeeper still
# treats the binary as unidentified, and the data-protection keychain still
# refuses it.
set -euo pipefail

NAME="GHOSTRACE Local Signing"
IDENTIFIER="com.alisinadevelo.ghostrace"
# Tests point this at a throwaway keychain.
KEYCHAIN="${GHOSTRACE_SIGNING_KEYCHAIN:-$HOME/Library/Keychains/login.keychain-db}"

usage() {
	sed -n '2,19p' "$0" | sed 's/^# \{0,1\}//'
}

require_macos() {
	if [ "$(uname -s)" != "Darwin" ]; then
		echo "local signing is macOS only" >&2
		exit 2
	fi
}

has_identity() {
	security find-certificate -c "$NAME" "$KEYCHAIN" >/dev/null 2>&1
}

create() {
	if has_identity; then
		echo "The identity \"$NAME\" already exists."
		return 0
	fi
	local work
	work=$(mktemp -d)
	chmod 700 "$work"
	trap 'rm -rf "$work"' RETURN
	cat >"$work/cert.cnf" <<EOF
[req]
distinguished_name=dn
x509_extensions=ext
prompt=no
[dn]
CN=$NAME
[ext]
basicConstraints=critical,CA:false
keyUsage=critical,digitalSignature
extendedKeyUsage=critical,codeSigning
EOF
	openssl req -x509 -newkey rsa:3072 -nodes -days 3650 -config "$work/cert.cnf" \
		-keyout "$work/key.pem" -out "$work/cert.pem" 2>/dev/null
	local password
	password=$(openssl rand -hex 24)
	# OpenSSL 3 needs -legacy for a PKCS#12 file macOS can import; LibreSSL
	# does not know the flag and already writes the compatible format.
	openssl pkcs12 -export -legacy -inkey "$work/key.pem" -in "$work/cert.pem" \
		-out "$work/identity.p12" -passout "pass:$password" 2>/dev/null ||
		openssl pkcs12 -export -inkey "$work/key.pem" -in "$work/cert.pem" \
			-out "$work/identity.p12" -passout "pass:$password"
	# -x marks the private key non-extractable: it can sign, but never leave
  # the keychain again.
  security import "$work/identity.p12" -k "$KEYCHAIN" -P "$password" -x -T /usr/bin/codesign >/dev/null
	echo "Created \"$NAME\" in $(basename "$KEYCHAIN")."
	echo "Next: $0 trust   (macOS asks for your password once)"
}

trust() {
	has_identity || {
		echo "Run '$0 create' first." >&2
		exit 1
	}
	local work
	work=$(mktemp -d)
	trap 'rm -rf "$work"' RETURN
	security find-certificate -c "$NAME" -p "$KEYCHAIN" >"$work/cert.pem"
	echo "macOS will ask for your password to trust \"$NAME\" for code signing only."
	security add-trusted-cert -p codeSign -k "$KEYCHAIN" "$work/cert.pem"
	echo "Trusted for code signing."
}

sign() {
	local binary=${1:-target/release/ghostrace}
	[ -f "$binary" ] || {
		echo "No binary at $binary; build it first." >&2
		exit 1
	}
	codesign --force --timestamp=none -s "$NAME" -i "$IDENTIFIER" "$binary"
	codesign --verify --strict "$binary"
	echo "Signed $binary. Designated requirement:"
	codesign -d -r - "$binary" 2>&1 | sed -n 's/^designated => /  /p'
}

status() {
	if ! has_identity; then
		echo "No \"$NAME\" identity. Run '$0 create'."
		return 0
	fi
	if security find-identity -v -p codesigning "$KEYCHAIN" | grep -q "\"$NAME\""; then
		echo "\"$NAME\" exists and is valid for code signing."
	else
		echo "\"$NAME\" exists but is not trusted for code signing yet. Run '$0 trust'."
	fi
}

remove() {
	if ! has_identity; then
		echo "There is no \"$NAME\" identity."
		return 0
	fi
	local work
	work=$(mktemp -d)
	trap 'rm -rf "$work"' RETURN
	security find-certificate -c "$NAME" -p "$KEYCHAIN" >"$work/cert.pem"
	security remove-trusted-cert "$work/cert.pem" 2>/dev/null || true
	security delete-identity -c "$NAME" "$KEYCHAIN" >/dev/null
	echo "Removed \"$NAME\". Binaries signed with it now prompt for keychain access again."
}

case "${1:-help}" in
create)
	require_macos
	create
	;;
trust)
	require_macos
	trust
	;;
sign)
	require_macos
	sign "${2:-}"
	;;
status)
	require_macos
	status
	;;
remove)
	require_macos
	remove
	;;
help | -h | --help) usage ;;
*)
	usage >&2
	exit 2
	;;
esac
