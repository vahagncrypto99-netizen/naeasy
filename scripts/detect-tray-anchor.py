#!/usr/bin/env python3
"""Find the horizontal center of a tray icon in the GNOME top bar.

GTK/SNI trays report no geometry to the app (tray-icon's GTK backend returns
a hard `None`), so a popover cannot place itself under its own icon. GNOME
does expose the top bar over AT-SPI, though — this reads it, so the value can
go into `tray_anchor_x` in the config and the popover lands under the icon.

    ./scripts/detect-tray-anchor.py            # list what is in the top bar
    ./scripts/detect-tray-anchor.py --set icon # write naeasy's x to the config

naeasy's indicator shows up as 'icon'. Run it with naeasy running; re-run it
whenever the set of indicators changes, since that shifts the positions.

Needs python3-pyatspi (Ubuntu: apt install python3-pyatspi).
"""
import json
import os
import sys

import pyatspi

CONFIG = os.path.expanduser("~/.config/com.vahagn.naeasy/config.json")


def indicators():
    """Every top-bar indicator as (center_x, name), left to right."""
    found = []

    def walk(node, depth=0):
        if depth > 9:
            return
        for child in node:
            try:
                role, name = child.getRoleName(), child.name or ""
                ext = child.queryComponent().getExtents(pyatspi.DESKTOP_COORDS)
                if role == "menu" and ext.y < 30 and ext.width > 0:
                    found.append((ext.x + ext.width // 2, name))
            except Exception:
                pass
            try:
                walk(child, depth + 1)
            except Exception:
                pass

    for app in pyatspi.Registry.getDesktop(0):
        try:
            if app.name and "shell" in app.name.lower():
                walk(app)
        except Exception:
            pass
    return sorted(found)


def main():
    found = indicators()
    if not found:
        sys.exit("No top-bar indicators found — is the accessibility bus running?")

    wanted = None
    if len(sys.argv) == 3 and sys.argv[1] == "--set":
        wanted = sys.argv[2]

    for center, name in found:
        mark = "->" if wanted and name == wanted else "  "
        print(f"{mark} center x={center:5d}  '{name}'")

    if not wanted:
        print("\nPass --set <name> to write that x to tray_anchor_x in the config.")
        return

    matches = [c for c, name in found if name == wanted]
    if not matches:
        sys.exit(f"\nNo indicator named '{wanted}'.")
    if len(matches) > 1:
        sys.exit(f"\n'{wanted}' matches {len(matches)} indicators — rename one to tell them apart.")

    with open(CONFIG) as fh:
        config = json.load(fh)
    config["tray_anchor_x"] = matches[0]
    with open(CONFIG, "w") as fh:
        json.dump(config, fh, indent=2)
    print(f"\ntray_anchor_x = {matches[0]} written to {CONFIG}. Restart naeasy.")


if __name__ == "__main__":
    main()
