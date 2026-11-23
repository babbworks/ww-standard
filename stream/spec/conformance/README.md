# Conformance logs

Each directory is a log. `EXPECTED` says what a correct reader must report:
`ok`, an error when opening (`open:` plus text the error must contain), or a
problem when verifying (`verify:` plus text the problem must contain).

Every implementation of the kernel runs these. Two implementations that pass
the same cases agree on what a valid log is. Regenerate only with a new kernel.
