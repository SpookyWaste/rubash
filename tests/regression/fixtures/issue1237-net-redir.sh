# rubash#1237 (items 1-3): redirection diagnostics. A failed <>
# open names the target once (redirection_error consumes the
# path_error text as-is). A refused /dev/tcp connect and an unknown
# service name each get GNU netopen's extra sys_error line BEFORE the
# redirection_error line: `{name}: connect: Connection refused`
# (script name only) and `{name}: line N: {service}: Servname not
# supported for ai_socktype` (glibc gai_strerror for EAI_SERVICE).
# Known limitation: the netopen first lines are written to the
# process stderr directly, so under an explicit `2>&1` on the failing
# command they do not follow the fd-2 rebinding into the merged
# stream (the redirection_error line itself does).
echo x 1<>nodir34/file 2>&1; echo "rw rc=$?"
( exec 3<>/dev/tcp/127.0.0.1/1 ); echo "tcp rc=$?"
( exec 3<>/dev/tcp/127.0.0.1/notaport ); echo "svc rc=$?"
( exec 3<>/dev/tcp/127.0.0.1/http ); echo "http rc=$?"
