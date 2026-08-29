import QtQuick
import Quickshell
import Quickshell.Io
import qs.Commons
import qs.Ui

// The Omarcade cabinet: pick a game, see what there is to beat.
//
// Summoned from the marquee, or directly:
//   omarchy-shell shell summon ridgetopai.omarcade.cabinet '{}'
//
// Games are discovered from installed binaries rather than from score
// files, so a title that has never been played still appears — it just
// has nothing to beat yet. Scores come from the same watched files the
// marquee reads, so a run finished while this is open updates it live.
Item {
  id: root

  // ---- host injections ----------------------------------------------
  property var shell: null
  property var manifest: null

  readonly property string pluginId:
    (manifest && manifest.id) ? manifest.id : "ridgetopai.omarcade.cabinet"

  // ---- theme ---------------------------------------------------------
  readonly property color foreground: Color.foreground
  readonly property color background: Color.background
  readonly property color accent: Color.accent

  readonly property string scoresDir:
    (Quickshell.env("XDG_STATE_HOME") || Quickshell.env("HOME") + "/.local/state")
    + "/omarcade/scores"

  readonly property string binDir: Quickshell.env("HOME") + "/.local/bin"

  // ---- lifecycle -----------------------------------------------------

  function open(payloadJson) {
    scan()
    window.visible = true
    Qt.callLater(function () { if (keys) keys.forceActiveFocus() })
  }

  // Host-initiated close (`shell hide`): the shell already knows.
  function close() {
    window.visible = false
  }

  // User-initiated close (Esc, window button): tell the shell, or its
  // open-panel bookkeeping drifts and the next toggle does nothing.
  function requestClose() {
    if (shell && typeof shell.hide === "function") shell.hide(root.pluginId)
    else window.visible = false
  }

  // ---- discovery ------------------------------------------------------
  //
  // Installed binaries are the source of truth for "what can I play".
  // A find(1) over ~/.local/bin, so a second title needs no change here.

  property var games: []
  property int selected: 0

  Process {
    id: scanProc
    running: false
    command: ["find", root.binDir, "-maxdepth", "1", "-type", "f",
              "-name", "omarcade-*", "-printf", "%f\n"]

    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: root.applyListing(text)
    }

    onExited: function (code) { if (code !== 0) root.games = [] }
  }

  function scan() {
    if (!scanProc.running) scanProc.running = true
  }

  function applyListing(output) {
    var found = []
    var lines = String(output || "").split("\n")
    for (var i = 0; i < lines.length; i++) {
      var name = lines[i].trim()
      if (name.length > 0) found.push(name)
    }
    found.sort()

    // Only reassign when the set changed, or the Repeater rebuilds every
    // ScoreRecord and drops its file watches.
    if (JSON.stringify(found) !== JSON.stringify(root.games)) {
      root.games = found
      if (root.selected >= found.length) root.selected = 0
    }
  }

  // "omarcade-breakout" -> "Breakout". The score file carries a proper
  // display name; this is the fallback until one exists.
  function prettify(id) {
    var s = String(id).replace(/^omarcade-/, "").replace(/[-_]+/g, " ")
    return s.length ? s.charAt(0).toUpperCase() + s.slice(1) : id
  }

  function recordFor(index) {
    var r = records.itemAt(index)
    return r || null
  }

  function launch(index) {
    if (index < 0 || index >= games.length) return
    // uwsm-app puts the game in its own systemd scope: a crash stays the
    // game's problem instead of taking the shell down with it.
    Quickshell.execDetached(["uwsm-app", "--", root.binDir + "/" + games[index]])
    requestClose()
  }

  Repeater {
    id: records
    model: root.games
    delegate: ScoreRecord {
      gameId: modelData
      path: root.scoresDir + "/" + modelData + ".json"
    }
  }

  // ---- window ---------------------------------------------------------

  FloatingWindow {
    id: window
    title: "Omarcade"
    color: root.background
    implicitWidth: 560
    implicitHeight: 420
    minimumSize: Qt.size(420, 320)

    onVisibleChanged: if (!visible && !root.closingFromHost) root.requestClose()

    PanelKeyCatcher {
      id: keys
      anchors.fill: parent
      onCloseRequested: root.requestClose()
      onActivateRequested: root.launch(root.selected)
      onReturnRequested: root.launch(root.selected)
      onMoveRequested: function (dx, dy) {
        if (root.games.length === 0) return
        var next = root.selected + dy
        if (next < 0) next = root.games.length - 1
        if (next >= root.games.length) next = 0
        root.selected = next
      }

      Column {
        anchors.fill: parent
        anchors.margins: Style.space(20)
        spacing: Style.space(14)

        PanelSectionHeader {
          width: parent.width
          text: "OMARCADE"
          foreground: root.foreground
        }

        PanelSeparator {
          width: parent.width
          foreground: root.foreground
        }

        // Empty state. A fresh install has games but the directory may
        // be missing entirely; say so rather than showing a blank box.
        Text {
          width: parent.width
          visible: root.games.length === 0
          text: "No games found in ~/.local/bin.\nRun ./packaging/install.sh from the Omarcade repo."
          wrapMode: Text.WordWrap
          color: Qt.darker(root.foreground, 1.4)
          font.family: Style.font.family
          font.pixelSize: Style.font.body
        }

        Repeater {
          model: root.games

          Rectangle {
            required property int index
            required property string modelData

            width: parent ? parent.width : 0
            implicitHeight: hero.implicitHeight + Style.spacing.rowPaddingX * 2
            radius: Style.space(6)
            // The selected row gets a faint wash rather than a border:
            // it reads as a highlight without adding a second box edge.
            color: index === root.selected
              ? Qt.rgba(root.accent.r, root.accent.g, root.accent.b, 0.12)
              : "transparent"

            Behavior on color {
              ColorAnimation { duration: 120 }
            }

            PanelHero {
              id: hero
              anchors.fill: parent
              anchors.margins: Style.spacing.rowPaddingX
              foreground: root.foreground

              title: {
                var r = root.recordFor(index)
                return (r && r.record && r.record.name) ? r.record.name : root.prettify(modelData)
              }

              meta: {
                var r = root.recordFor(index)
                return (r && r.best > 0) ? "BEST " + r.best : "not played yet"
              }

              detail: modelData
            }

            MouseArea {
              anchors.fill: parent
              hoverEnabled: true
              cursorShape: Qt.PointingHandCursor
              onEntered: root.selected = index
              onClicked: root.launch(index)
            }
          }
        }
      }
    }
  }

  property bool closingFromHost: false
}
