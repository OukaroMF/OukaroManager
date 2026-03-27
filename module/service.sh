#!/system/bin/sh

MODDIR=${0%/*}

until [ -d "$MODDIR" ]; do
	sleep 1
done

# Runtime hot-reload is intentionally disabled.
# The next reboot's post-mount stage applies the saved config.
exit 0
