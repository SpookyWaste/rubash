# rubash#1227/#1231: shell_execve (execute_cmd.c) hands the child the
# command word exactly as typed for argv[0] — `exec -a` replaces it,
# `exec -l` prefixes '-', a hash -p name stays the typed name — never
# the resolved program path.
( exec -a customname bash -c 'echo "argv0:$0"' )
( exec -l bash -c 'echo "dash0:$0"' )
( exec -a customS sh -c 'echo "sh0:$0"' )
hash -p /bin/dash mydash
mydash -c 'echo "hash0:$0"'
/bin/dash -c 'echo "slash0:$0"'
bash -c 'echo "plain0:$0"'
