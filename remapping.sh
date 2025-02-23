#!/bin/bash

REMAPPING_GEN="cargo run -q -p glam-cpi-gen remapping --config ../glam/anchor/programs/glam/src/cpi_autogen/config.yaml"

#
# DRIFT
#
DRIFT_IDL=$(realpath ../glam/anchor/deps/drift/drift.json)
DRIFT_OUT=../glam/anchor/programs/glam/src/cpi_autogen/remapping/drift.json
$REMAPPING_GEN $DRIFT_IDL \
    --program-id dRiftyHA39MWEi3m9aunc5MzRF1JYuBsbn6VPcn33UH \
    --ixs initializeUserStats \
    --ixs initializeUser \
    --ixs deleteUser \
    --ixs updateUserCustomMarginRatio \
    --ixs updateUserDelegate \
    --ixs updateUserMarginTradingEnabled \
    --ixs deposit \
    --ixs withdraw \
    --ixs cancelOrders \
    --ixs cancelOrdersByIds \
    --ixs modifyOrder \
    --ixs placeOrders \
    --output $DRIFT_OUT

#
# JUPITER GOV
#
JUP_GOV_IDL=$(realpath ../glam/anchor/deps/jupiter_gov/govern.json)
JUP_GOV_OUT=../glam/anchor/programs/glam/src/cpi_autogen/remapping/jupiter_gov.json
JUP_VOTE_IDL=$(realpath ../glam/anchor/deps/jupiter_vote/locked_voter.json)
JUP_VOTE_OUT=../glam/anchor/programs/glam/src/cpi_autogen/remapping/jupiter_vote.json

$REMAPPING_GEN $JUP_VOTE_IDL --idl-name-alias jupiter_vote \
    --program-id voTpe3tHQ7AjQHMapgSue2HJFAh2cGsdokqN3XqmVSj \
    --ixs newEscrow \
    --ixs increaseLockedAmount \
    --ixs openPartialUnstaking \
    --ixs mergePartialUnstaking \
    --ixs withdraw \
    --ixs withdrawPartialUnstaking \
    --ixs castVote \
    --output $JUP_VOTE_OUT

$REMAPPING_GEN $JUP_GOV_IDL --idl-name-alias jupiter_gov \
    --program-id GovaE4iu227srtG2s3tZzB4RmWBzw8sTwrCLZz7kN7rY \
    --ixs newVote \
    --output $JUP_GOV_OUT


#
# MARINADE
#
MARINADE_IDL=$(realpath ../glam/anchor/deps/marinade/marinade_finance.json)
MARINADE_OUT=../glam/anchor/programs/glam/src/cpi_autogen/remapping/marinade.json

$REMAPPING_GEN $MARINADE_IDL --idl-name-alias marinade \
    --program-id MarBmsSgKXdrN1egZf5sqe1TMai9K1rChYNDJgjq7aD \
    --ixs deposit \
    --ixs liquidUnstake \
    --ixs claim \
    --ixs orderUnstake \
    --ixs depositStakeAccount \
    --output $MARINADE_OUT
