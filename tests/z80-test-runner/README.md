
Rad Test Suite
==============

This is a test running for moa that uses the [raddad772/jsmoo tests](https://github.com/raddad772/jsmoo).

To run, the jsmoo repository must be cloned into tests/ and then from the moa project root:
```shell
cargo run -p rad_tests -- [FILTER]
```

An optional filter can be specified, which will only run test files who's file name starts with the
filter text.  Timing tests are not done by default, but can be run with `-t` or `--timing`.  The output
can be increased or decreased with the `--debug` or `--quiet` flags, respectively.

Special thanks to [raddad772](https://github.com/raddad772) for the incredibly
exhaustive and thorough set of testcases.  Emulators everywhere will be better
for your efforts!

