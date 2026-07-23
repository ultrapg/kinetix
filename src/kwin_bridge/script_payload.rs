// NOTE: The header (KINETIX_MAX_WINDOWS, KINETIX_SWAP_ZONE_RATIO, etc.)
// is injected by Rust before this payload at load time.
pub const KWIN_SCRIPT_PAYLOAD: &str = r#"
// Kinetix Tiling Engine — Self-Contained KWin 6 Script v3
// Injected config parameters:
//   KINETIX_MAX_WINDOWS     (0 = unlimited)
//   KINETIX_SWAP_ZONE_RATIO (0.0–1.0, fraction of window center counted as "swap" zone)
//   KINETIX_GAPS            (outer screen gap px)
//   KINETIX_INNER_GAPS      (inner gap between tiles px)

(function() {
    'use strict';
    print("Kinetix: KWin 6 Tiling Script v3 (Aspect Ratio & Drag-Split) Initialized.");

    var maxWindows    = (typeof KINETIX_MAX_WINDOWS    !== 'undefined') ? KINETIX_MAX_WINDOWS    : 0;
    var swapZoneRatio = (typeof KINETIX_SWAP_ZONE_RATIO !== 'undefined') ? KINETIX_SWAP_ZONE_RATIO : 0.4;
    var outerGap      = (typeof KINETIX_GAPS           !== 'undefined') ? KINETIX_GAPS           : 0;
    var innerGap      = (typeof KINETIX_INNER_GAPS     !== 'undefined') ? KINETIX_INNER_GAPS     : 0;

    print("Kinetix config: maxWindows=" + maxWindows + " swapZone=" + swapZoneRatio +
          " outerGap=" + outerGap + " innerGap=" + innerGap);

    var draggingWindows = {};   // winId -> true while dragging
    var hookedWindows = {};     // winId -> true if signals connected

    function isTileable(client) {
        if (!client) return false;
        if (!client.normalWindow) return false;
        if (client.specialWindow) return false;
        if (client.dialog) return false;
        if (client.fullScreen) return false;
        var cls = (client.resourceClass || "").toString().toLowerCase();
        var skip = ["plasmashell","krunner","yakuake","ksmserver","kwin",
                    "spectacle","kmix","plasma-desktop","plasmoidviewer"];
        for (var i = 0; i < skip.length; i++) {
            if (cls.indexOf(skip[i]) !== -1) return false;
        }
        return true;
    }

    function getWinId(w) {
        if (!w) return "null";
        if (w.internalId) return w.internalId.toString();
        if (w.windowId)   return w.windowId.toString();
        return (w.caption || "win") + "_" + (w.resourceClass || "");
    }

    function getLeafTiles(tile) {
        var leaves = [];
        function collect(t) {
            if (!t) return;
            if (!t.tiles || t.tiles.length === 0) { leaves.push(t); }
            else { for (var i = 0; i < t.tiles.length; i++) collect(t.tiles[i]); }
        }
        collect(tile);
        return leaves;
    }

    function tileArea(t) {
        var g = t.absoluteGeometry;
        return g.width * g.height;
    }

    // Smart tile selection for splitting to preserve standard aspect ratios (e.g. 16:9 / 4:3)
    // Determines optimal split direction:
    //   If tile width > 1.2 * height -> split vertically (Horizontal direction = 1, produces left/right)
    //   Else -> split horizontally (Vertical direction = 2, produces top/bottom)
    function chooseBestSplitTile(leaves) {
        var best = null;
        var bestScore = -1;
        for (var i = 0; i < leaves.length; i++) {
            var area = tileArea(leaves[i]);
            if (area > bestScore) {
                bestScore = area;
                best = leaves[i];
            }
        }
        return best;
    }

    function getOptimalSplitDir(tile) {
        var g = tile.absoluteGeometry;
        var ratio = g.width / Math.max(1, g.height);
        // If wider than tall, split side-by-side (dir = 1 / Horizontal)
        // If taller than wide, split top-and-bottom (dir = 2 / Vertical)
        return (ratio >= 1.0) ? 1 : 2;
    }

    // =========================================================
    // Core Layout Engine
    // =========================================================
    function applyLayout() {
        var wins = workspace.windowList();
        var validWins = [];
        for (var i = 0; i < wins.length; i++) {
            var w = wins[i];
            if (!isTileable(w)) continue;
            if (draggingWindows[getWinId(w)]) continue; // Skip windows currently being dragged
            validWins.push(w);
        }

        if (maxWindows > 0 && validWins.length > maxWindows) {
            validWins = validWins.slice(0, maxWindows);
        }

        if (validWins.length === 0) return;

        // Sort windows deterministically: leftmost first, then topmost
        validWins.sort(function(a, b) {
            var ga = a.frameGeometry, gb = b.frameGeometry;
            var ax = Math.round(ga.x), bx = Math.round(gb.x);
            var ay = Math.round(ga.y), by = Math.round(gb.y);
            if (Math.abs(ax - bx) > 20) return ax - bx;
            return ay - by;
        });

        var firstWin = validWins[0];
        if (!firstWin.output) return;

        var rt = workspace.rootTile(firstWin.output, workspace.currentDesktop);
        if (!rt) return;

        rt.padding = outerGap;

        // Unmanage all windows from tiles
        for (var i = 0; i < validWins.length; i++) {
            if (validWins[i].tile) {
                try { validWins[i].tile.unmanage(validWins[i]); } catch(e) {}
            }
            try {
                if (validWins[i].maximizeMode !== 0) validWins[i].setMaximize(false, false);
            } catch(e) {}
        }

        // Clear tiles
        if (rt.tiles) {
            while (rt.tiles.length > 0) try { rt.tiles[0].remove(); } catch(e) { break; }
        }

        if (validWins.length === 1) {
            rt.manage(validWins[0]);
            print("Kinetix JS: 1 window -> full screen tile");
            return;
        }

        // Split tiles preserving aspect ratio
        var attempts = 0;
        while (getLeafTiles(rt).length < validWins.length && attempts < 40) {
            var leaves = getLeafTiles(rt);
            var target = chooseBestSplitTile(leaves);
            if (!target) break;
            var dir = getOptimalSplitDir(target);
            try { target.split(dir); } catch(e) { print("Kinetix JS: split error: " + e); break; }
            attempts++;
        }

        var finalLeaves = getLeafTiles(rt);

        if (innerGap > 0) {
            for (var i = 0; i < finalLeaves.length; i++) {
                finalLeaves[i].padding = innerGap;
            }
        }

        print("Kinetix JS: Layout applied -> " + validWins.length + " windows into " + finalLeaves.length + " tiles.");

        for (var j = 0; j < validWins.length && j < finalLeaves.length; j++) {
            try { finalLeaves[j].manage(validWins[j]); } catch(e) {
                print("Kinetix JS: manage error win[" + j + "]: " + e);
            }
        }
    }

    // Debounced layout scheduler
    var debounceTimer = new QTimer();
    debounceTimer.singleShot = true;
    debounceTimer.interval = 250;
    debounceTimer.timeout.connect(function() { applyLayout(); });

    function scheduleLayout() {
        if (debounceTimer.running) debounceTimer.stop();
        debounceTimer.start();
    }

    // =========================================================
    // Advanced Drag & Drop Handler (Swap & Multi-Directional Split)
    // =========================================================
    function findTargetWindowAt(draggedWin, dropX, dropY) {
        var wins = workspace.windowList();
        var draggedId = getWinId(draggedWin);
        for (var i = 0; i < wins.length; i++) {
            var w = wins[i];
            if (!isTileable(w) || getWinId(w) === draggedId || !w.tile) continue;
            var g = w.frameGeometry;
            if (dropX >= g.x && dropX <= g.x + g.width &&
                dropY >= g.y && dropY <= g.y + g.height) {
                return w;
            }
        }
        return null;
    }

    function computeDropAction(targetWin, dropX, dropY) {
        var g = targetWin.frameGeometry;
        var relX = (dropX - g.x) / Math.max(1, g.width);
        var relY = (dropY - g.y) / Math.max(1, g.height);

        var zoneH = swapZoneRatio / 2;
        var cLeft  = 0.5 - zoneH;
        var cRight = 0.5 + zoneH;
        var cTop   = 0.5 - zoneH;
        var cBot   = 0.5 + zoneH;

        // If drop point is in the inner center zone -> SWAP
        if (relX >= cLeft && relX <= cRight && relY >= cTop && relY <= cBot) {
            return "swap";
        }

        // Otherwise find closest edge
        var dLeft   = relX;
        var dRight  = 1.0 - relX;
        var dTop    = relY;
        var dBottom = 1.0 - relY;
        var minDist = Math.min(dLeft, dRight, dTop, dBottom);

        if (minDist === dLeft)   return "split-left";
        if (minDist === dRight)  return "split-right";
        if (minDist === dTop)    return "split-top";
        return "split-bottom";
    }

    function handleDragDrop(draggedWin, dropX, dropY) {
        var target = findTargetWindowAt(draggedWin, dropX, dropY);
        if (!target) {
            print("Kinetix JS: Drop target not found at (" + dropX + "," + dropY + "), re-tiling layout.");
            scheduleLayout();
            return;
        }

        var action = computeDropAction(target, dropX, dropY);
        print("Kinetix JS: Drag drop action on " + target.caption + ": " + action);

        if (action === "swap") {
            var targetTile = target.tile;
            if (targetTile) {
                try {
                    targetTile.manage(draggedWin);
                    print("Kinetix JS: Swapped " + draggedWin.caption + " into target tile.");
                } catch(e) {
                    print("Kinetix JS: Swap error: " + e);
                }
            }
            scheduleLayout();
        } else {
            // Split action: split-left, split-right, split-top, split-bottom
            var targetTile = target.tile;
            if (!targetTile) { scheduleLayout(); return; }

            // 1 = Horizontal split (left/right children)
            // 2 = Vertical split (top/bottom children)
            var splitDir = (action === "split-left" || action === "split-right") ? 1 : 2;

            try {
                targetTile.split(splitDir);
                var children = getLeafTiles(targetTile);
                if (children.length >= 2) {
                    var draggedChild = (action === "split-left" || action === "split-top") ? children[0] : children[1];
                    var targetChild  = (action === "split-left" || action === "split-top") ? children[1] : children[0];
                    
                    targetChild.manage(target);
                    draggedChild.manage(draggedWin);
                    print("Kinetix JS: Managed split drag drop for " + draggedWin.caption + " (" + action + ")");
                } else {
                    scheduleLayout();
                }
            } catch(e) {
                print("Kinetix JS: Drag split error: " + e);
                scheduleLayout();
            }
        }
    }

    // =========================================================
    // Dynamic Window Event Connections
    // =========================================================
    function hookWindowSignals(w) {
        if (!isTileable(w)) return;
        var wid = getWinId(w);
        if (hookedWindows[wid]) return;
        hookedWindows[wid] = true;

        try {
            w.interactiveMoveResizeStarted.connect(function() {
                if (w.move) {
                    draggingWindows[getWinId(w)] = true;
                    print("Kinetix JS: User started dragging: " + w.caption);
                    // Unmanage from tile so movement is completely free & smooth
                    if (w.tile) {
                        try { w.tile.unmanage(w); } catch(e) {}
                    }
                }
            });

            w.interactiveMoveResizeFinished.connect(function() {
                var widStr = getWinId(w);
                if (draggingWindows[widStr]) {
                    delete draggingWindows[widStr];
                    var g = w.frameGeometry;
                    var dropX = Math.round(g.x + g.width / 2);
                    var dropY = Math.round(g.y + g.height / 2);
                    print("Kinetix JS: User dropped " + w.caption + " at center (" + dropX + "," + dropY + ")");
                    handleDragDrop(w, dropX, dropY);
                }
            });
        } catch(e) {
            print("Kinetix JS: Signal hook error for " + w.caption + ": " + e);
        }
    }

    // Hook existing windows
    workspace.windowList().forEach(function(w) {
        hookWindowSignals(w);
    });

    workspace.windowAdded.connect(function(client) {
        if (isTileable(client)) {
            print("Kinetix JS: Window added: " + client.caption);
            hookWindowSignals(client);
            scheduleLayout();
        } else {
            var pollCount = 0;
            var pollTimer = new QTimer();
            pollTimer.singleShot = false;
            pollTimer.interval = 100;
            pollTimer.timeout.connect(function() {
                pollCount++;
                if (isTileable(client)) {
                    pollTimer.stop();
                    print("Kinetix JS: Window initialized: " + client.caption);
                    hookWindowSignals(client);
                    scheduleLayout();
                } else if (pollCount >= 15) {
                    pollTimer.stop();
                }
            });
            pollTimer.start();
        }
    });

    workspace.windowRemoved.connect(function(client) {
        var wid = getWinId(client);
        delete draggingWindows[wid];
        delete hookedWindows[wid];
        print("Kinetix JS: Window removed: " + (client.caption || "?"));
        scheduleLayout();
    });

    // Initial layout execution
    var initTimer = new QTimer();
    initTimer.singleShot = true;
    initTimer.interval = 150;
    initTimer.timeout.connect(function() { applyLayout(); });
    initTimer.start();

    try {
        callDBus("org.kde.kinetix.Bridge", "/Bridge", "org.kde.kinetix.Bridge",
                 "ScriptReady", "", function() {});
    } catch(e) {}

    print("Kinetix JS: v3 script active.");
})();
"#;
