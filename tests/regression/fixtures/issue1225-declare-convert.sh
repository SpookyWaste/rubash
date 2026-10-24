# rubash#1225: converting an existing variable with declare -A/-a
# (variables.c convert_var_to_assoc / arrayfunc.c
# convert_var_to_array): a set scalar — even set-but-empty — becomes
# element [0]; a variable with NO existing cell stays unset, and the
# conversion never fabricates or preserves stale compound values.
v=; declare -A v; declare -p v
w=; declare -a w; declare -p w
s=scalar; declare -a s; declare -p s
t=scalar; declare -A t; declare -p t
declare -A fresh=; declare -p fresh
declare -a fresh2=; declare -p fresh2
declare -A nocell; declare -p nocell
a=(x y); declare -a a; declare -p a
