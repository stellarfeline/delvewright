# What is in here, and where it came from

`SHA256SUMS-v1.4.0` is the file the engine's own release workflow published on
`v1.4.0`, downloaded verbatim from

    https://github.com/stellarfeline/delvewright/releases/download/v1.4.0/SHA256SUMS

and committed unchanged. It is a fixture and not a pin: nothing resolves a
version through it, and it is never re-fetched.

It is here because it is the only artifact that carries **both** forms coreutils
writes a checksum row in. Four of its five rows are the text form
(`<digest>  <name>`) and the Windows row is the binary form
(`<digest> *<name>`), and that one row is what the page's old shell extraction —
`grep " $ARCHIVE\$"` — could not match. A hand-written fixture would have
carried whichever form its author remembered; this one carries what was
actually published, which is the whole point of driving the parser with it.
