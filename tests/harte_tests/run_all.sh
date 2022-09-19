#!/bin/bash
COMMIT=$(git rev-parse HEAD)
DATE=$(date --iso)
LOCATION=$(dirname ${BASH_SOURCE[0]})
RESULTS=latest.txt
{
    cd $LOCATION
    echo "Last run on $DATE at commit $COMMIT" | tee $RESULTSt
    echo "" | tee -a $RESULTS
    cargo run -- -q --testsuite "../ProcessorTests/680x0/68000/uncompressed/" | tee -a $RESULTS
}
