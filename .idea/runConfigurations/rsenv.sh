#!/usr/bin/env bash
#
# DO NOT DELETE
#
[[ -f "$SOPS_PATH/environments/${RUN_ENV:-local}.env" ]] && rsenv build "$SOPS_PATH/environments/${RUN_ENV:-local}.env"
