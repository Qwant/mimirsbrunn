#!/bin/bash

# Call the given command with extra defaults `--run-mode` `--config-dir`
# parameter values.
#
# You can still override those default parameters if you need with through
# variables $RUN_MODE and $CONFIG_DIR.
#
# This is useful in the context of the docker images where the config directory
# is embedded at a fixed path.

CMD=$1
shift
ARG=$@

RUN_MODE=${RUN_MODE:-"docker"}
CONFIG_DIR=${CONFIG_DIR:-"/etc/mimirsbrunn"}

$CMD --run-mode $RUN_MODE --config-dir $CONFIG_DIR $ARG
