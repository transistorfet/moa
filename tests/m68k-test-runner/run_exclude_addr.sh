#!/bin/bash
COMMIT=$(git rev-parse HEAD)
DATE=$(date --iso)
LOCATION=$(dirname ${BASH_SOURCE[0]})
RESULTS=latest-excluding-addr-error.txt
{
    cd $LOCATION
    echo "Last run on $DATE at commit $COMMIT" | tee $RESULTS
    echo "" | tee -a $RESULTS
    cargo run -- -q --testsuite "../m68000/v1/" -e exclude-addr | tee -a $RESULTS
}
