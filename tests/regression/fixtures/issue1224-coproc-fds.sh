# rubash#1224: an unnamed coproc displays with its default name COPROC in
# job listings (execute_cmd.c gives an unnamed coproc the name "COPROC";
# print_cmd.c prints it), and coproc fd pairs follow GNU sh_openpipe's
# four-end move_to_high_fd allocation: all four pipe ends take the
# highest free fds below 64 in rpipe[0], rpipe[1], wpipe[0], wpipe[1]
# order, the parent keeping rpipe[0] and wpipe[1] — 63/60 for the first
# coproc, 62/58 for the second while the first is live.
coproc ALPHA { sleep 5; }
coproc BETA { sleep 5; }
echo "alpha=${ALPHA[0]}/${ALPHA[1]} beta=${BETA[0]}/${BETA[1]}"
coproc { sleep 5; }
jobs
kill %1 %2 %3 2>/dev/null
echo done
