The idea of this feature is taking from our explorations of the resonate sdk and how it works (see /home/darkvoid/Boxxed/@dev/repo-expolorations/resonate).


## Durable Promises

Whilst not all its ideas are applicable, there is actually relevant benefit to it's idea of durable promises that are backed by a datastore (sqlite - think libsql, turso, Cloudflare D1) or even Postgres.

A durable promise is a record of an operation that has relatively 5 states:

- Pending - created and awaiting to be executed or put back in pending state
- Running - now being handled by some process for completion.
- Resolved - where it is resolved with some serialized data value
- Failed/Rejected - where it fails with some serialized failing data value
- Blocked/Packed - where its blocked on another promise

With an additional ownership state of:

1. Owned - Being that a process owns it and has lease over it for some period of time that needs to be updated via heartbeats
2. Free - Being no process owns it and can be leased and taken ownership of by any available process.


Promises can be dependent on other promises via a direct 1 to 1 relationships or depend on multiple promises via a 1 to many relationships,  I think it is also possible for many promises to depend on 1 promise (so many to 1 style) as well.

But the core idea is: a promise ultimated is Resolved or Rejected and we can get the value of that final state from the attached data element it has.


## Tasks

These are another interesting concept by resonate, Task somewhat represent some concrete notion of a specific type of computation whoes results or end state is represented by a Promise, meaning the completion of a task is 1 to 1 mapped to the completion of a Promise related to the task.

Meaning every task has a Promise, and some tasks could evidently be blocked by the dependents of whatever Promise it depends on (that 1 to many, 1 to 1 or many to 1 relationships we spoke about in durable promise).

In my head, i think a task represent some registered computation that the user indicates can run and instead a task is completed when its promise is resolved/rejected.

And a task will contain the name of the task, the location of the 

And building on that construct that
