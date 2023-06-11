#!/bin/bash
COMMIT=$(git rev-parse HEAD)
DATE=$(date --iso)
LOCATION=$(dirname ${BASH_SOURCE[0]})
RESULTS=latest.txt
{
    cd $LOCATION
    echo "Last run on $DATE at commit $COMMIT" | tee $RESULTS
    echo "" | tee -a $RESULTS
    cargo run -- -q --testsuite "../jsmoo/misc/tests/GeneratedTests/z80/v1/" --check-undocumented --check-timings | tee -a $RESULTS
}
