
// NOTE: The header (KINETIX_MAX_WINDOWS, KINETIX_SWAP_ZONE_RATIO, etc.)
// is injected by Rust before this payload at load time.
pub const KWIN_SCRIPT_PAYLOAD: &str = r#"
// Kinetix Tiling Engine — KWin 6 Script v9.7
// Fixes Overlapping Windows.
// 1. Strictly enforces 1 window per tile. If KWin glitches or limits cause 
//    windows to overlap in a single tile, Kinetix ejects the extra windows 
//    and forces them into their own tiles.
// 2. Fallback split checks: if a split direction fails (e.g. tile too narrow), 
//    it automatically tries the other direction or finds the next best tile.

(function() {
    'use strict';
    print("Kinetix: KWin 6 Tiling Script v9.7 Initialized.");

    var maxWindows    = (typeof KINETIX_MAX_WINDOWS    !== 'undefined') ? KINETIX_MAX_WINDOWS    : 0;
    var swapZoneRatio = (typeof KINETIX_SWAP_ZONE_RATIO !== 'undefined') ? KINETIX_SWAP_ZONE_RATIO : 0.4;
    var outerGap      = (typeof KINETIX_GAPS           !== 'undefined') ? KINETIX_GAPS           : 0;
    var innerGap      = (typeof KINETIX_INNER_GAPS     !== 'undefined') ? KINETIX_INNER_GAPS     : 0;

    var hookedWindows = {};
    var dragState     = {};
    var isDropping    = false;

    function anyDragging() { return Object.keys(dragState).length > 0; }

    // =========================================================
    // Overlay helpers
    // =========================================================
    function showOverlay(x, y, w, h, screenW, screenH) {
        try {
            callDBus("org.kde.kinetix.Bridge", "/Bridge", "org.kde.kinetix.Bridge",
                "ShowOverlay",
                JSON.stringify({ x: Math.round(x), y: Math.round(y), w: Math.round(w), h: Math.round(h), screenW: screenW, screenH: screenH }),
                function() {}
            );
        } catch(e) {}
    }
    function hideOverlay() {
        try { callDBus("org.kde.kinetix.Bridge", "/Bridge", "org.kde.kinetix.Bridge", "HideOverlay", "", function() {}); } catch(e) {}
    }

    // =========================================================
    // Window helpers
    // =========================================================
    function isTileable(client) {
        if (!client || !client.normalWindow || client.specialWindow || client.dialog || client.fullScreen) return false;
        if (client.minimized || (client.maximizeMode && client.maximizeMode !== 0)) return false;
        var cls = (client.resourceClass || "").toString().toLowerCase();
        var skip = ["plasmashell","krunner","yakuake","ksmserver","kwin",
                    "spectacle","kmix","plasma-desktop","plasmoidviewer"];
        for (var i = 0; i < skip.length; i++) if (cls.indexOf(skip[i]) !== -1) return false;
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
        for (var i = 0; i < wins.length; i++) { if (getWinId(wins[i]) === id) return wins[i]; }
        return null;
    }
    function getLeafTiles(tile) {
        var leaves = [];
        function collect(t) {
            if (!t) return;
            if (!t.tiles || t.tiles.length === 0) leaves.push(t);
            else for (var i = 0; i < t.tiles.length; i++) collect(t.tiles[i]);
        }
        collect(tile);
        return leaves;
    }
    function getOptimalSplitDir(tile) {
        var g = tile.absoluteGeometry;
        return (g.width / Math.max(1, g.height) >= 1.0) ? 1 : 2;
    }

    // =========================================================
    // Organic BSP Layout
    // =========================================================
    function applyLayout() {
        if (isDropping || anyDragging()) return;

        var wins = workspace.windowList();
        var validWins = [];
        var rt = null;

        for (var i = 0; i < wins.length; i++) {
            var w = wins[i];
            if (!rt && w.output) rt = workspace.rootTile(w.output, workspace.currentDesktop);
            if (!isTileable(w)) {
                if (w && w.tile) try { w.tile.unmanage(w); } catch(e) {}
                continue;
            }
            if (dragState[getWinId(w)]) continue;
            validWins.push(w);
        }

        if (validWins.length === 0 || !rt) return;
        rt.padding = outerGap;

        // 1. Strict overlap fix: Force 1 window per tile max!
        var allLeaves = getLeafTiles(rt);
        for (var i = 0; i < allLeaves.length; i++) {
            var l = allLeaves[i];
            while (l.windows.length > 1) {
                var extra = l.windows[l.windows.length - 1];
                try { l.unmanage(extra); } catch(e) {}
            }
        }

        // 2. Cleanup empty tiles
        var cleaned = true, maxLoops = 15;
        while (cleaned && maxLoops > 0) {
            cleaned = false; maxLoops--;
            var leaves = getLeafTiles(rt);
            for (var i = 0; i < leaves.length; i++) {
                if (leaves[i] !== rt && leaves[i].windows.length === 0) {
                    try { leaves[i].remove(); cleaned = true; } catch(e) {}
                }
            }
        }

        // 3. Float catching
        var floatingWins = [];
        for (var i = 0; i < validWins.length; i++) {
            if (!validWins[i].tile) floatingWins.push(validWins[i]);
        }

        // 4. Place floating windows
        for (var i = 0; i < floatingWins.length; i++) {
            var w = floatingWins[i];
            var leaves = getLeafTiles(rt);
            if (leaves.length === 1 && leaves[0].windows.length === 0) {
                leaves[0].manage(w); continue;
            }

            // Sort by largest area
            leaves.sort(function(a, b) {
                var ga = a.absoluteGeometry, gb = b.absoluteGeometry;
                return (gb.width * gb.height) - (ga.width * ga.height);
            });

            var placed = false;
            for (var j = 0; j < leaves.length; j++) {
                var target = leaves[j];
                try {
                    var dir = getOptimalSplitDir(target);
                    var occupant = target.windows.length > 0 ? target.windows[0] : null;
                    
                    target.split(dir);
                    var newLeaves = getLeafTiles(target);
                    if (newLeaves.length < 2) {
                        target.split(dir === 1 ? 2 : 1); // Try fallback direction
                        newLeaves = getLeafTiles(target);
                    }

                    if (newLeaves.length >= 2) {
                        if (occupant && occupant.tile) occupant.tile.unmanage(occupant);
                        if (occupant) newLeaves[0].manage(occupant);
                        newLeaves[1].manage(w);
                        placed = true;
                        break;
                    }
                } catch(e) {}
            }
            if (!placed) print("Kinetix JS: Tile capacity reached, could not split for window.");
        }

        // 5. Apply inner gaps
        var leaves = getLeafTiles(rt);
        if (innerGap > 0) { for (var i = 0; i < leaves.length; i++) leaves[i].padding = innerGap; }
    }

    var debounceTimer = new QTimer();
    debounceTimer.singleShot = true;
    debounceTimer.interval = 200;
    debounceTimer.timeout.connect(function() { applyLayout(); });
    function scheduleLayout() {
        if (anyDragging() || isDropping) return;
        if (debounceTimer.running) debounceTimer.stop();
        debounceTimer.start();
    }

    // =========================================================
    // Drag-and-drop
    // =========================================================
    function computeDropAction(snap, cursorX, cursorY) {
        var relX = (cursorX - snap.x) / Math.max(1, snap.w);
        var relY = (cursorY - snap.y) / Math.max(1, snap.h);
        var half = swapZoneRatio / 2;
        if (relX >= 0.5 - half && relX <= 0.5 + half && relY >= 0.5 - half && relY <= 0.5 + half) return "swap";
        var dL = relX, dR = 1 - relX, dT = relY, dB = 1 - relY;
        var m = Math.min(dL, dR, dT, dB);
        if (m === dL) return "split-left";
        if (m === dR) return "split-right";
        if (m === dT) return "split-top";
        return "split-bottom";
    }

    function computePreviewRect(snap, action) {
        var hw = Math.floor(snap.w / 2), hh = Math.floor(snap.h / 2);
        if (action === "swap")         return { x: snap.x,      y: snap.y,      w: snap.w,      h: snap.h };
        if (action === "split-left")   return { x: snap.x,      y: snap.y,      w: hw,          h: snap.h };
        if (action === "split-right")  return { x: snap.x + hw, y: snap.y,      w: snap.w - hw, h: snap.h };
        if (action === "split-top")    return { x: snap.x,      y: snap.y,      w: snap.w,      h: hh };
        if (action === "split-bottom") return { x: snap.x,      y: snap.y + hh, w: snap.w,      h: snap.h - hh };
        return null;
    }

    function findTargetSnap(snapshot, cursorX, cursorY) {
        for (var i = 0; i < snapshot.length; i++) {
            var s = snapshot[i];
            if (cursorX >= s.x && cursorX <= s.x + s.w && cursorY >= s.y && cursorY <= s.y + s.h) return s;
        }
        var bestSnap = null, bestDist = 300;
        for (var i = 0; i < snapshot.length; i++) {
            var s = snapshot[i];
            var dx = cursorX - (s.x + s.w / 2), dy = cursorY - (s.y + s.h / 2);
            var d = Math.sqrt(dx*dx + dy*dy);
            if (d < bestDist) { bestDist = d; bestSnap = s; }
        }
        return bestSnap;
    }

    function handleDragDrop(draggedWin, cursorX, cursorY, snapshot, originalTile) {
        if (draggedWin.tile) try { draggedWin.tile.unmanage(draggedWin); } catch(e) {}
        var targetSnap = findTargetSnap(snapshot, cursorX, cursorY);
        if (!targetSnap) return;

        var action    = computeDropAction(targetSnap, cursorX, cursorY);
        var targetId  = targetSnap.winId;
        var targetWin = findWindowById(targetId);

        if (action === "swap") {
            if (targetWin && targetWin.tile) {
                var targetTile = targetWin.tile;
                targetTile.unmanage(targetWin);
                targetTile.manage(draggedWin);
                if (originalTile && originalTile.windows.length === 0) originalTile.manage(targetWin);
            }
        } else {
            var splitDir = (action === "split-left" || action === "split-right") ? 1 : 2;
            if (targetWin && targetWin.tile) {
                try {
                    var targetTile = targetWin.tile;
                    targetTile.split(splitDir);
                    var children = getLeafTiles(targetTile);
                    
                    if (children.length < 2) {
                        targetTile.split(splitDir === 1 ? 2 : 1);
                        children = getLeafTiles(targetTile);
                    }

                    if (children.length >= 2) {
                        if (targetWin.tile) targetWin.tile.unmanage(targetWin);
                        
                        var g0 = children[0].absoluteGeometry;
                        var g1 = children[1].absoluteGeometry;
                        var leftChild, rightChild, topChild, bottomChild;
                        
                        if (g0.x === g1.x && g0.y === g1.y && g0.width === g1.width && g0.height === g1.height) {
                            leftChild = children[0]; rightChild = children[1];
                            topChild = children[0]; bottomChild = children[1];
                        } else {
                            leftChild  = g0.x <= g1.x ? children[0] : children[1];
                            rightChild = g0.x <= g1.x ? children[1] : children[0];
                            topChild   = g0.y <= g1.y ? children[0] : children[1];
                            bottomChild= g0.y <= g1.y ? children[1] : children[0];
                        }

                        var dragTile, targetTileChild;
                        if (action === "split-left" || action === "split-right") {
                            dragTile        = (action === "split-left")  ? leftChild  : rightChild;
                            targetTileChild = (action === "split-left")  ? rightChild : leftChild;
                        } else {
                            dragTile        = (action === "split-top")   ? topChild    : bottomChild;
                            targetTileChild = (action === "split-top")   ? bottomChild : topChild;
                        }

                        targetTileChild.manage(targetWin);
                        dragTile.manage(draggedWin);

                        if (originalTile) try { originalTile.remove(); } catch(e) {}
                    }
                } catch(e) {}
            }
        }
    }

    function hookWindowSignals(w) {
        if (!w || !w.normalWindow) return;
        var wid = getWinId(w);
        if (hookedWindows[wid]) return;
        hookedWindows[wid] = true;

        try {
            w.interactiveMoveResizeStarted.connect(function() {
                if (!w.move) return;
                var id = getWinId(w);
                var snapshot = [];
                var allWins = workspace.windowList();
                for (var i = 0; i < allWins.length; i++) {
                    var ow = allWins[i];
                    if (!isTileable(ow) || getWinId(ow) === id || !ow.tile) continue;
                    var b = ow.tile.absoluteGeometry;
                    snapshot.push({ winId: getWinId(ow), x: b.x, y: b.y, w: b.width, h: b.height });
                }
                var screenW = 0, screenH = 0;
                try {
                    var rt = workspace.rootTile(w.output, workspace.currentDesktop);
                    var rg = rt.absoluteGeometry;
                    screenW = rg.x + rg.width; screenH = rg.y + rg.height;
                } catch(e) {}

                dragState[id] = {
                    lastX: -1, lastY: -1,
                    snapshot: snapshot,
                    originalTile: w.tile || null,
                    lastOverlayKey: null,
                    screenW: screenW, screenH: screenH
                };
                if (w.tile) try { w.tile.unmanage(w); } catch(e) {}
            });

            w.interactiveMoveResizeStepped.connect(function(rect) {
                var id = getWinId(w);
                if (!dragState[id]) return;
                try {
                    var cur = workspace.cursorPos;
                    if (cur && typeof cur.x === 'number') { dragState[id].lastX = cur.x; dragState[id].lastY = cur.y; }
                } catch(e) {}

                var cx = dragState[id].lastX, cy = dragState[id].lastY;
                if (cx === -1) return;
                var snap   = findTargetSnap(dragState[id].snapshot, cx, cy);
                var action = snap ? computeDropAction(snap, cx, cy) : null;
                var key    = snap ? (snap.winId + "|" + action) : null;

                if (key === dragState[id].lastOverlayKey) return;
                dragState[id].lastOverlayKey = key;

                if (snap && action) {
                    var pr = computePreviewRect(snap, action);
                    if (pr) showOverlay(pr.x, pr.y, pr.w, pr.h, dragState[id].screenW, dragState[id].screenH);
                } else hideOverlay();
            });

            w.interactiveMoveResizeFinished.connect(function() {
                var id = getWinId(w);
                if (!dragState[id]) return;
                var state = dragState[id];
                if (state.lastX === -1) { hideOverlay(); delete dragState[id]; scheduleLayout(); return; }

                var cursorX = Math.round(state.lastX), cursorY = Math.round(state.lastY);
                hideOverlay();
                
                var dropTimer = new QTimer();
                dropTimer.singleShot = true;
                dropTimer.interval = 50;
                dropTimer.timeout.connect(function() {
                    isDropping = true;
                    delete dragState[id];
                    handleDragDrop(w, cursorX, cursorY, state.snapshot, state.originalTile);
                    isDropping = false;
                    scheduleLayout();
                });
                dropTimer.start();
            });

            if (w.minimizedChanged) w.minimizedChanged.connect(function() {
                if (w.minimized && w.tile) try { w.tile.unmanage(w); } catch(e) {}
                scheduleLayout();
            });
            if (w.maximizedChanged) w.maximizedChanged.connect(function() {
                if (w.maximizeMode !== 0 && w.tile) try { w.tile.unmanage(w); } catch(e) {}
                scheduleLayout();
            });
        } catch(e) {}
    }

    workspace.windowList().forEach(function(w) { hookWindowSignals(w); });

    workspace.windowAdded.connect(function(client) {
        hookWindowSignals(client);
        if (isTileable(client)) {
            scheduleLayout();
        } else {
            var pollCount = 0, pollTimer = new QTimer();
            pollTimer.singleShot = false; pollTimer.interval = 100;
            pollTimer.timeout.connect(function() {
                pollCount++;
                if (isTileable(client)) { pollTimer.stop(); scheduleLayout(); }
                else if (pollCount >= 15) pollTimer.stop();
            });
            pollTimer.start();
        }
    });

    workspace.windowRemoved.connect(function(client) {
        var wid = getWinId(client);
        if (dragState[wid]) { hideOverlay(); delete dragState[wid]; }
        delete hookedWindows[wid];
        scheduleLayout();
    });

    var initTimer = new QTimer();
    initTimer.singleShot = true; initTimer.interval = 150;
    initTimer.timeout.connect(function() { applyLayout(); });
    initTimer.start();

    try { callDBus("org.kde.kinetix.Bridge", "/Bridge", "org.kde.kinetix.Bridge", "ScriptReady", "", function() {}); } catch(e) {}
    print("Kinetix JS: v9.7 script active.");
})();
"#;
