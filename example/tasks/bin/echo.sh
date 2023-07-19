#!/bin/bash

START=1
END="${TIMES}"

i=1
while [[ $i -le ${END} ]]
do
  echo "Echo message [$i]: ${MESSAGE}"
  ((i = i + 1))
done
