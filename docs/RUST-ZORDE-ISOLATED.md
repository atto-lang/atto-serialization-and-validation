# Rust isolated bootstrap and log admission candidate

Linux x86_64 only, std-only. Trusted clean parent must provision the
launcher binary and independently reviewed Bash gate digest; environment
clearing cannot undo dynamic loader execution before main. The bootstrap
passes only approved inputs and fixed PATH/locale; gate bytes are checked
before execution, and existing gate approval checks still apply. The
source checkout must be exclusive: hash-then-exec is not a hostile
same-UID filesystem snapshot. Capture uses directory-relative no-follow
opens from root, exclusive private output, a child RLIMIT_FSIZE of
16 MiB and GNU timeout. It refuses empty and exactly saturated logs,
nonzero status, link/path changes, and non-private evidence directories.
The compiler and guest must have been independently authenticated by the
caller; capture is not an authentication or receipt issuer. GNU timeout
is not a sandbox for daemonized or process-group-escaping descendants;
production requires an isolated runner/cgroup. Legacy Python gates remain
active pending exact-head tests and review. No actual guest evidence.
