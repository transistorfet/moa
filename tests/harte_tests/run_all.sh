#!/bin/bash
COMMIT=$(git rev-parse HEAD)
DATE=$(date --iso)
LOCATION=$(dirname ${BASH_SOURCE[0]})
{
    cd $LOCATION
    echo "Last run on $DATE at commit $COMMIT" | tee latest.txt
    echo "" | tee -a latest.txt
    cargo run -- -q --testsuite "../ProcessorTests/680x0/68000/uncompressed/" | tee -a latest.txt
}
