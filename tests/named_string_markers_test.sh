#!/bin/bash
# Test named string markers do not leak to user-visible output
# Verifies that __RUBASH_HD1__/CSB1__/CA1__ markers are resolved
# before text leaves the decode boundary.

set -e

THIS_SH="${THIS_SH:-/mnt/d/repo/rubash/target/debug/rubash.exe}"

echo "Test 1: No QUOTED_HEREDOC_MARKER in heredoc output"
output=$("$THIS_SH" -c 'cat <<EOF
hello
EOF')
if printf "%s" "$output" | grep -q '__RUBASH_HD1__'; then
    echo "FAIL: QUOTED_HEREDOC_MARKER leaked to stdout"
    exit 1
else
    echo "PASS: no QUOTED_HEREDOC_MARKER in stdout"
fi

echo "Test 2: No COMSUB_PAYLOAD_PREFIX in command substitution"
output=$("$THIS_SH" -c 'x=$(echo hello); echo "$x"')
if printf "%s" "$output" | grep -q '__RUBASH_CSB1_'; then
    echo "FAIL: COMSUB_PAYLOAD_PREFIX leaked to stdout"
    exit 1
else
    echo "PASS: no COMSUB_PAYLOAD_PREFIX in stdout"
fi

echo "Test 3: No COMPOUND_ASSIGNMENT_MARKER in compound assignment"
# Since rubash#1634 the carrier is the registry-zone char U+E31A (the
# ASCII spelling below is user data now and may legitimately appear).
output=$("$THIS_SH" -c 'arr=(a b c); echo "${arr[@]}"')
if printf "%s" "$output" | grep -q "$(printf '\uE31A')"; then
    echo "FAIL: COMPOUND_ASSIGNMENT_MARKER leaked to stdout"
    exit 1
else
    echo "PASS: no COMPOUND_ASSIGNMENT_MARKER in stdout"
fi

echo "All named string marker golden assertion tests passed"
