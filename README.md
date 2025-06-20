# Mutex-pool

probably fastest locking on pool of objects

So I one had an issue of having to balance between many connection and this is the lib I made.
It works using atomics and atomic operations so they are limited to amount to `len <= atomic::BITS` it should be enough for most usecases tho

## Alternatives 

- [apool](https://github.com/JavaDerg/apool) - Made be woke DEI dragon offers unlimited size pools tho they are slower than my, should be renamed to wokepool

## I have a pool that is faster than this project

Great make a PR here: https://github.com/AwesomeQubic/poolers-competion and prove it