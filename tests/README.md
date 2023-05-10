
Tests
=====

This directory contains CPU tests for the 68k and Z80.  The test cases themselves are provided by
Tom Harte and raddad772, and must be cloned from their respective repositories before running the
tests.


Downloading
-----------

To download the 68k tests, from the `tests/` directory, run:
```sh
git clone git@github.com:TomHarte/ProcessorTests.git
```

To download the Z80 tests, from the `tests/` directory, run:
```sh
git clone --no-checkout git@github.com:raddad772/jsmoo.git
cd jsmoo
git checkout origin/HEAD -- misc/tests/GeneratedTests
```


Running
-------

The 68k tests can be run from the moa root with:
```sh
tests/harte_tests/run_all.sh
```
By default, the script will use the compressed versions of the tests which are slower to run because
they must be unzipped every time the tests are run. To speed it up for repeat runs, the tests can be
gunzip'ed to their own directory and the test suite location can be change on the command line or in
the script to point to the uncompressed versions

The Z80 tests can be run with:
```sh
tests/rad_tests/run_all.sh
```


Thanks to [Tom Harte](https://github.com/TomHarte) and [raddad772](https://github.com/raddad772) for
providing these incredibly valuable tests

