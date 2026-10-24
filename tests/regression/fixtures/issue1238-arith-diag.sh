# rubash#1238/#1239: arithmetic diagnostics. Assigning to a readonly
# variable in (( ))/let reports ONLY `{name}: readonly variable`
# (expr.c/execute_cmd.c) — no synthetic follow-on syntax error. And
# under `set -u`, reading an element of an EMPTY associative array in
# arithmetic is an unbound-variable trap on the array name (subst.c
# nounset check), while a pure element assignment is allowed.
declare -r ro=5
((ro++)); echo "postinc rc=$?"
echo "val=$ro"
x=$((ro+1)); echo "sum=$x"
declare -ri ri=10
let ri=20; echo "let rc=$?"
(set -u; declare -A h; ((h[k]++)); echo "never printed")
echo "empty-assoc-read rc=$?"
(set -u; declare -A h2; ((h2[k]=5)); echo "pure-assign ok v=${h2[k]}")
(set -u; declare -A h3; h3[x]=1; ((h3[missing])); echo "nonempty-missing rc=$?")
