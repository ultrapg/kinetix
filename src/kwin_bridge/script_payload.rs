
// NOTE: The header (KINETIX_MAX_WINDOWS, KINETIX_SWAP_ZONE_RATIO, etc.)
// is injected by Rust before this payload at load time.
pub const KWIN_SCRIPT_PAYLOAD: &str = r#"
// Kinetix Tiling Engine — KWin 6 Script v8
// Drag-and-drop bug fixes vs v7:
//   1. originalTile saved at drag-start; called .remove() after surgical split
//      so the empty original tile doesn't leave a blank area on screen
//   2. dragState kept active during 100ms post-drop delay so scheduleLayout
//      cannot fire between interactiveMoveResizeFinished and our handler
//   3. draggedWin explicitly unmanaged from any KWin-reassigned tile before
//      we manage it into our surgical split child
//   4. No-stepped-events fallback → scheduleLayout (not wrong frame-center)

(function() {
    'use strict';
    print("Kinetix: KWin 6 Tiling Script v8 Initialized.");

    var maxWindows    = (typeof KINETIX_MAX_WINDOWS    !== 'undefined') ? KINETIX_MAX_WINDOWS    : 0;
    var swapZoneRatio = (typeof KINETIX_SWAP_ZONE_RATIO !== 'undefined') ? KINETIX_SWAP_ZONE_RATIO : 0.4;
    var outerGap      = (typeof KINETIX_GAPS           !== 'undefined') ? KINETIX_GAPS           : 0;
    var innerGap      = (typeof KINETIX_INNER_GAPS     !== 'undefined') ? KINETIX_INNER_GAPS     : 0;

    print("Kinetix config: maxWindows=" + maxWindows + " swapZone=" + swapZoneRatio +
          " outerGap=" + outerGap + " innerGap=" + innerGap);

    var hookedWindows = {};  // winId -> true
    var tiledOrder    = [];  // winId ordering for layout

    // Per-drag state: winId -> { lastX, lastY, snapshot[], originalTile }
    // snapshot[i] = { winId, x, y, w, h }
    // Kept ALIVE during the 100ms post-drop delay to block scheduleLayout.
    var dragState = {};

    function anyDragging() {
        return Object.keys(dragState).length > 0;
    }

    function isTileable(client) {
        if (!client) return false;
        if (!client.normalWindow) return false;
        if (client.specialWindow) return false;
        if (client.dialog) return false;
        if (client.fullScreen) return false;
        if (client.minimized) return false;
        if (client.maximizeMode && client.maximizeMode !== 0) return false;
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

    function findWindowById(id) {
        var wins = workspace.windowList();
        for (var i = 0; i < wins.length; i++) {
            if (getWinId(wins[i]) === id) return wins[i];
        }
        return null;
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

    function chooseBestSplitTile(leaves) {
        var best = null, bestScore = -1;
        for (var i = 0; i < leaves.length; i++) {
            var g = leaves[i].absoluteGeometry;
            var area = g.width * g.height;
            if (area > bestScore) { bestScore = area; best = leaves[i]; }
        }
        return best;
    }

    function getOptimalSplitDir(tile) {
        var g = tile.absoluteGeometry;
        return (g.width / Math.max(1, g.height) >= 1.0) ? 1 : 2;
    }

    function syncTiledOrder(validWins) {
        var currentIds = [];
        for (var i = 0; i < validWins.length; i++) currentIds.push(getWinId(validWins[i]));

        tiledOrder = tiledOrder.filter(function(id) { return currentIds.indexOf(id) !== -1; });

        if (tiledOrder.length === 0 && validWins.length > 0) {
            validWins.sort(function(a, b) {
                var ga = a.frameGeometry, gb = b.frameGeometry;
                var ax = Math.round(ga.x), bx = Math.round(gb.x);
                var ay = Math.round(ga.y), by = Math.round(gb.y);
                if (Math.abs(ax - bx) > 20) return ax - bx;
                return ay - by;
            });
            for (var i = 0; i < validWins.length; i++) tiledOrder.push(getWinId(validWins[i]));
            return;
        }

        for (var i = 0; i < currentIds.length; i++) {
            if (tiledOrder.indexOf(currentIds[i]) === -1) tiledOrder.push(currentIds[i]);
        }
    }

    // =========================================================
    // Core Layout Engine
    // =========================================================
    function applyLayout() {
        var wins = workspace.windowList();
        var validWins = [];
        for (var i = 0; i < wins.length; i++) {
            var w = wins[i];
            if (!isTileable(w)) {
                if (w && (w.minimized || (w.maximizeMode && w.maximizeMode !== 0)) && w.tile) {
                    try { w.tile.unmanage(w); } catch(e) {}
                }
                continue;
            }
            if (dragState[getWinId(w)]) continue; // skip dragging windows
            validWins.push(w);
        }

        if (maxWindows > 0 && validWins.length > maxWindows)
            validWins = validWins.slice(0, maxWindows);

        if (validWins.length === 0) { print("Kinetix JS: No active tileable windows."); return; }

        syncTiledOrder(validWins);

        validWins.sort(function(a, b) {
            var ia = tiledOrder.indexOf(getWinId(a));
            var ib = tiledOrder.indexOf(getWinId(b));
            if (ia === -1) ia = 999;
            if (ib === -1) ib = 999;
            return ia - ib;
        });

        var rt = workspace.rootTile(validWins[0].output, workspace.currentDesktop);
        if (!rt) return;

        rt.padding = outerGap;

        for (var i = 0; i < validWins.length; i++) {
            if (validWins[i].tile) try { validWins[i].tile.unmanage(validWins[i]); } catch(e) {}
        }
        if (rt.tiles) while (rt.tiles.length > 0) try { rt.tiles[0].remove(); } catch(e) { break; }

        if (validWins.length === 1) { rt.manage(validWins[0]); return; }

        var attempts = 0;
        while (getLeafTiles(rt).length < validWins.length && attempts < 40) {
            var leaves = getLeafTiles(rt);
            var target = chooseBestSplitTile(leaves);
            if (!target) break;
            try { target.split(getOptimalSplitDir(target)); } catch(e) { break; }
            attempts++;
        }

        var finalLeaves = getLeafTiles(rt);
        if (innerGap > 0) for (var i = 0; i < finalLeaves.length; i++) finalLeaves[i].padding = innerGap;

        print("Kinetix JS: Layout applied -> " + validWins.length + " windows into " + finalLeaves.length + " tiles.");
        for (var j = 0; j < validWins.length && j < finalLeaves.length; j++) {
            try { finalLeaves[j].manage(validWins[j]); } catch(e) {}
        }
    }

    var debounceTimer = new QTimer();
    debounceTimer.singleShot = true;
    debounceTimer.interval = 200;
    debounceTimer.timeout.connect(function() { applyLayout(); });

    function scheduleLayout() {
        if (anyDragging()) return; // never relayout mid-drag (or during 100ms post-drop delay)
        if (debounceTimer.running) debounceTimer.stop();
        debounceTimer.start();
    }

    // =========================================================
    // Drag & Drop
    // =========================================================
    function computeDropAction(snap, cursorX, cursorY) {
        var relX = (cursorX - snap.x) / Math.max(1, snap.w);
        var relY = (cursorY - snap.y) / Math.max(1, snap.h);

        var zoneH = swapZoneRatio / 2;
        if (relX >= 0.5 - zoneH && relX <= 0.5 + zoneH &&
            relY >= 0.5 - zoneH && relY <= 0.5 + zoneH) {
            return "swap";
        }

        var dLeft = relX, dRight = 1.0 - relX, dTop = relY, dBottom = 1.0 - relY;
        var minDist = Math.min(dLeft, dRight, dTop, dBottom);
        if (minDist === dLeft)   return "split-left";
        if (minDist === dRight)  return "split-right";
        if (minDist === dTop)    return "split-top";
        return "split-bottom";
    }

    function findTargetSnap(snapshot, cursorX, cursorY) {
        for (var i = 0; i < snapshot.length; i++) {
            var s = snapshot[i];
            if (cursorX >= s.x && cursorX <= s.x + s.w &&
                cursorY >= s.y && cursorY <= s.y + s.h) {
                return s;
            }
        }
        // Fallback: closest tile centre (within 300px)
        var bestSnap = null, bestDist = 300;
        for (var i = 0; i < snapshot.length; i++) {
            var s = snapshot[i];
            var cx = s.x + s.w / 2, cy = s.y + s.h / 2;
            var dist = Math.sqrt(Math.pow(cursorX - cx, 2) + Math.pow(cursorY - cy, 2));
            if (dist < bestDist) { bestDist = dist; bestSnap = s; }
        }
        return bestSnap;
    }

    function handleDragDrop(draggedWin, cursorX, cursorY, snapshot, originalTile) {
        print("Kinetix JS: handleDragDrop cursor=(" + cursorX + "," + cursorY +
              ") snapshot=" + snapshot.length + " origTile=" + !!originalTile);

        // Safety: ensure dragged window is not in any tile before we re-assign it
        if (draggedWin.tile) {
            try { draggedWin.tile.unmanage(draggedWin); } catch(e) {}
        }

        var targetSnap = findTargetSnap(snapshot, cursorX, cursorY);
        if (!targetSnap) {
            print("Kinetix JS: No target found, re-tiling.");
            scheduleLayout();
            return;
        }

        var action    = computeDropAction(targetSnap, cursorX, cursorY);
        var draggedId = getWinId(draggedWin);
        var targetId  = targetSnap.winId;
        print("Kinetix JS: Drop action=" + action + " target=" + targetId);

        if (action === "swap") {
            // SWAP: exchange in tiledOrder then full rebuild
            var di = tiledOrder.indexOf(draggedId);
            var ti = tiledOrder.indexOf(targetId);
            if (di !== -1 && ti !== -1) {
                tiledOrder[di] = targetId;
                tiledOrder[ti] = draggedId;
            }
            applyLayout();

        } else {
            // SPLIT: surgically split the target's live tile, then remove the
            // now-empty original tile so it doesn't leave a blank area.
            var targetWin = findWindowById(targetId);
            var splitDir  = (action === "split-left" || action === "split-right") ? 1 : 2;
            var dragGoesTo   = (action === "split-left" || action === "split-top") ? 0 : 1;
            var targetGoesTo = 1 - dragGoesTo;

            var splitOk = false;
            if (targetWin && targetWin.tile) {
                try {
                    var targetTile = targetWin.tile;
                    targetTile.unmanage(targetWin);
                    targetTile.split(splitDir);

                    var children = getLeafTiles(targetTile);
                    if (children.length >= 2) {
                        children[targetGoesTo].manage(targetWin);
                        children[dragGoesTo].manage(draggedWin);
                        splitOk = true;
                        print("Kinetix JS: Surgical split " + action + " done.");

                        // === KEY FIX: remove the now-empty original tile ===
                        // Without this, the tile tree still holds the old tile as
                        // an empty leaf which leaves a blank region on screen.
                        if (originalTile) {
                            try {
                                originalTile.remove();
                                print("Kinetix JS: Removed empty originalTile.");
                            } catch(e) {
                                print("Kinetix JS: Could not remove originalTile: " + e);
                            }
                        }
                    }
                } catch(e) {
                    print("Kinetix JS: Surgical split error: " + e);
                }
            }

            // Update tiledOrder so future full rebuilds preserve the adjacency
            var di2 = tiledOrder.indexOf(draggedId);
            if (di2 !== -1) tiledOrder.splice(di2, 1);
            var ti2 = tiledOrder.indexOf(targetId);
            if (ti2 === -1) {
                tiledOrder.push(draggedId);
            } else {
                var insertAt2 = (action === "split-left" || action === "split-top") ? ti2 : ti2 + 1;
                tiledOrder.splice(insertAt2, 0, draggedId);
            }

            if (!splitOk) {
                print("Kinetix JS: Split fallback - rebuilding layout.");
                applyLayout();
            }
        }
    }

    // =========================================================
    // Signal Hooking
    // =========================================================
    function hookWindowSignals(w) {
        if (!w || !w.normalWindow) return;
        var wid = getWinId(w);
        if (hookedWindows[wid]) return;
        hookedWindows[wid] = true;

        try {
            w.interactiveMoveResizeStarted.connect(function() {
                if (!w.move) return;
                var id = getWinId(w);

                // Snapshot other tiled windows' bounds BEFORE any relayout
                var snapshot = [];
                var allWins = workspace.windowList();
                for (var i = 0; i < allWins.length; i++) {
                    var ow = allWins[i];
                    if (!isTileable(ow) || getWinId(ow) === id || !ow.tile) continue;
                    var b = ow.tile.absoluteGeometry;
                    snapshot.push({ winId: getWinId(ow), x: b.x, y: b.y, w: b.width, h: b.height });
                }

                // Save reference to the original tile BEFORE unmanaging so we
                // can call .remove() on it after the surgical split.
                var originalTile = w.tile || null;

                dragState[id] = { lastX: -1, lastY: -1, snapshot: snapshot, originalTile: originalTile };
                print("Kinetix JS: Drag started: " + w.caption +
                      " snapshot=" + snapshot.length + " origTile=" + !!originalTile);

                // Unmanage from tile so window moves freely during drag
                if (w.tile) try { w.tile.unmanage(w); } catch(e) {}
            });

            w.interactiveMoveResizeStepped.connect(function(rect) {
                // workspace.cursorPos works here but THROWS in Finished — track it here
                var id = getWinId(w);
                if (!dragState[id]) return;
                try {
                    var cur = workspace.cursorPos;
                    if (cur && typeof cur.x === 'number') {
                        dragState[id].lastX = cur.x;
                        dragState[id].lastY = cur.y;
                    }
                } catch(e) {}
            });

            w.interactiveMoveResizeFinished.connect(function() {
                var id = getWinId(w);
                if (!dragState[id]) return;

                var state = dragState[id];
                // Do NOT delete dragState yet — keep anyDragging()=true during the
                // 100ms delay so scheduleLayout() stays blocked.

                if (state.lastX === -1) {
                    // No movement detected (click without drag): just restore layout
                    delete dragState[id];
                    scheduleLayout();
                    return;
                }

                var cursorX = Math.round(state.lastX);
                var cursorY = Math.round(state.lastY);

                // Delay 100ms: KWin may re-assign the dragged window to a tile
                // between our callback and here; wait for KWin to finish, then
                // do our own assignment which takes final priority.
                var dropTimer = new QTimer();
                dropTimer.singleShot = true;
                dropTimer.interval = 100;
                dropTimer.timeout.connect(function() {
                    delete dragState[id]; // now allow scheduleLayout
                    handleDragDrop(w, cursorX, cursorY, state.snapshot, state.originalTile);
                });
                dropTimer.start();
            });

            if (w.minimizedChanged) {
                w.minimizedChanged.connect(function() {
                    if (w.minimized && w.tile) try { w.tile.unmanage(w); } catch(e) {}
                    scheduleLayout();
                });
            }

            if (w.maximizedChanged) {
                w.maximizedChanged.connect(function() {
                    if (w.maximizeMode !== 0 && w.tile) try { w.tile.unmanage(w); } catch(e) {}
                    scheduleLayout();
                });
            }
        } catch(e) {
            print("Kinetix JS: hookWindowSignals error for " + w.caption + ": " + e);
        }
    }

    workspace.windowList().forEach(function(w) { hookWindowSignals(w); });

    workspace.windowAdded.connect(function(client) {
        hookWindowSignals(client);
        if (isTileable(client)) {
            print("Kinetix JS: Window added: " + client.caption);
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
                    scheduleLayout();
                } else if (pollCount >= 15) { pollTimer.stop(); }
            });
            pollTimer.start();
        }
    });

    workspace.windowRemoved.connect(function(client) {
        var wid = getWinId(client);
        delete dragState[wid];
        delete hookedWindows[wid];
        print("Kinetix JS: Window removed: " + (client.caption || "?"));
        scheduleLayout();
    });

    var initTimer = new QTimer();
    initTimer.singleShot = true;
    initTimer.interval = 150;
    initTimer.timeout.connect(function() { applyLayout(); });
    initTimer.start();

    try {
        callDBus("org.kde.kinetix.Bridge", "/Bridge", "org.kde.kinetix.Bridge",
                 "ScriptReady", "", function() {});
    } catch(e) {}

    print("Kinetix JS: v8 script active.");
})();
"#;
