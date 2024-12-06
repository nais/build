#!/bin/sh
# vi: se et:

if [ -z "${ACTION}" ]; then
  /app/nb preflight
else
  /app/nb "${ACTION}"
fi
