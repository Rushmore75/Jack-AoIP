# Jack-AoIP
Inspired by Dante, I wanted to create an FOSS version. This is build using jack2 as the audio system.

Ofc [this also](https://github.com/jackaudio/jack2/blob/develop/README_NETJACK2) exsists


[ ] when the program stops it generates a handful of Xruns, this is probably due to not stopping cleanly...
[ ] test mapping of large amount of connections
[ ] allow for buffer size to be chosen after compile, via lazy static and array slices (maybe)
[ ] put in a buffer of 0s when transport stops
[ ] add transport control / syncing (tcp?)
[ ] decouple aoip sending from transport