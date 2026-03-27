SKIPUNZIP=1

require_path() {
	[ -e "$1" ] || abort "! Missing required module file: ${1#$MODPATH/}"
}

ui_print "- Installing OukaroManager"
ui_print "- Unpacking module payload"
if ! unzip -o "$ZIPFILE" -x "META-INF/*" -d "$MODPATH" >/dev/null; then
	abort "! Failed to unzip module payload"
fi

ui_print "- Verifying payload"
require_path "$MODPATH/module.prop"
require_path "$MODPATH/skip_mount"
require_path "$MODPATH/oukaro"
require_path "$MODPATH/okrmng"
require_path "$MODPATH/post-mount.sh"
require_path "$MODPATH/service.sh"
require_path "$MODPATH/webroot/index.html"

ui_print "- Setting permissions"
set_perm_recursive "$MODPATH" 0 0 0755 0644
set_perm "$MODPATH/oukaro" 0 0 0755
set_perm "$MODPATH/okrmng" 0 0 0755
set_perm "$MODPATH/post-mount.sh" 0 0 0755
set_perm "$MODPATH/service.sh" 0 0 0755
