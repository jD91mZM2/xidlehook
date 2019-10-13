# xidlehook (core)

This is the core of xidlehook, abstracted away as a library. The idea
is to keep the core reusable between different front-ends.

See [GitHub issue
#27](https://github.com/jD91mZM2/xidlehook/issues/27#issuecomment-536286742)
for an example why this split was made. Splitting some of the work out
in a library is also good for testing :)

In case this wasn't confusing enough for readers, there's also an
example named "xidlehook" which is basically a stripped-down version
of `xidlehook-daemon`. The reason for this is just to give a starting
point to people making their own clients that do not have to have a
full-blown async event loop like the current tool itself.

Preferrably, the feature creepiness of this crate should be turned all
the way down. I am a big fan of the Unix philosophy of keeping things
simple by design. The exception here are some basic modules that most
people will want.

**In short, use `xidlehook-daemon` and not this.**
