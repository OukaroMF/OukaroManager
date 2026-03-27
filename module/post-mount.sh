#!/system/bin/sh

MODDIR=${0%/*}
LOG=$MODDIR/oukaro.log

until [ -d "$MODDIR" ]; do
	sleep 1
done

RUST_BACKTRACE=1 "$MODDIR/oukaro" >>"$LOG" 2>&1
