
Tom Harte Test Suite
====================

This is a test running for moa that uses the [Tom Harte Test Suite](https://github.com/TomHarte/ProcessorTests).

To run, the ProcessorTests repository must be cloned into tests/ and then from the moa project root:
```shell
cargo run -p harte_tests -- [FILTER]
```

An optional filter can be specified, which will only run test files who's file name starts with the
filter text.  Timing tests are not done by default, but can be run with `-t` or `--timing`.  The output
can be increased or decreased with the `--debug` or `--quiet` flags, respectively.

Special thanks to [Tom](https://github.com/TomHarte) for painstakingly constructing this test suite.
Emulators everywhere will be better for your efforts!

