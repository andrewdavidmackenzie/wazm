# wazm
Explorations in web-assembly specific compression algorithms, written in rust.

## prerequisites
Install these tools manually before running `make test`
- gzip / gunzip

## Test files
Test files should be as optimal as possible valid wasm files, processed with tools like wasm-strip, wasm-opt,
wasm-gc etc to remove unnecessary symbols, sections etc.

## TODO
Investigate inclusion of output strings in the wasm files.

Detect the non-optimal input files and how they can be optimized before compressing, and print a
warning. WIll require investigating code of wasm-opt, gc, etc.

Do we expect the uncompressed file to be identical to the input?
If there are unpredictable or lossy changes (not affecting functioning) then create a new input
file and warn on that, before compressing. then when testing compare uncompressed files to 
that and not the original source.

First pass will be to parse the incoming file and print out a dump of all of it.

Much of it will need to be kept in memory....all of it?
Or can do it by section or something?
generate the output file without compression and see if still the same or compatible...

Then for each type of input, investigate types of compression that can be applied.