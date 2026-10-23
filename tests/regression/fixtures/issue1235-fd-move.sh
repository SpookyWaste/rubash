# rubash#1235/#1236: r_move descriptor semantics (redir.c
# do_redirection_internal + add_undo_redirect). During the command the
# target holds the source's description and the source is closed. When
# the command ends the close is UNDONE only if the target was open at
# apply time; a move onto a closed target closes the source for good.
# A self-dup/self-move (redir_fd == redirector) is a total no-op: it
# succeeds even on a closed fd and never closes an open one.
exec 3>capA
echo one >&3
echo two 5>&3-
if (echo probe >&3) 2>/dev/null; then echo "move-to-closed: fd3 open"; else echo "move-to-closed: fd3 closed"; fi
exec 3>capB
exec 6>capC
echo three 6>&3-
if (echo probe >&6) 2>/dev/null; then echo "move-to-open: fd6 open"; else echo "move-to-open: fd6 closed"; fi
if (echo probe >&3) 2>/dev/null; then echo "move-to-open: fd3 open"; else echo "move-to-open: fd3 closed"; fi
echo hi 99>&99 >/dev/null; echo "selfdup-closed rc=$?"
echo hi 99>&99- >/dev/null; echo "selfmove-closed rc=$?"
exec 8>capD
exec 8>&8-
if (echo probe >&8) 2>/dev/null; then echo "selfmove: fd8 open"; else echo "selfmove: fd8 closed"; fi
echo "capA:"; cat capA
echo "capB:"; cat capB
echo "capC:"; cat capC
echo "capD:"; cat capD
