#!/bin/bash

# ls -lah .
echo Sleeping 1/3...
echo "Sleeping 1/3 (stderr)..." > /dev/stderr
sleep 1

# ls -lah .
echo Sleeping 2/3...
echo "Sleeping 2/3 (stderr)..." > /dev/stderr
sleep 1

# ls -lah .
echo Sleeping 3/3...
echo "Sleeping 3/3 (stderr)..." > /dev/stderr
sleep 1

echo "Done: ${MESSAGE}"

exit 123
