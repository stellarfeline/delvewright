# Issue kinds

Every issue in this repository carries exactly one kind label. The kind is decided from the issue's body, not its title alone. The first line of each kind below is that label's description on GitHub.

## `bug`

Something built behaves wrongly: wrong output, a false refusal, or a check passing a wrong thing.

- The engine, a tool or a skill page produces output that is wrong: malformed commands, a wrong render, a wrong exit status, two pages that contradict each other.
- A check refuses something that is right, or reds when nothing it guards changed.
- A check or proof that exists accepts something wrong, or binds to nothing for objects in its own class: a delve can ship broken while the machine says it is fine.

## `idea`

A capability, design or research question that is not built yet.

- A new DSL surface, a new kind of object or base, or a new judgement over a property no check claims to cover today.
- A research round, a redesign of an existing surface, or a spec still to be written.

## `debt`

Process, tooling, docs or measurement upkeep that is neither a defect nor a new capability.

- A missing lint or CI step, a gate's scope still to widen, a citation without its revision, a file to split, a format to bring up to date, a contract that names no home for something.
- Nothing built gives a wrong answer today, and nothing new becomes buildable when it is done.
