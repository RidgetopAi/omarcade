import QtQuick
import Quickshell
import Quickshell.Io
import qs.Commons
import qs.Ui

// The Omarcade marquee: the best score across every installed game,
// live in the bar.
//
// This is the piece the suite exists for. Any one arcade game is a
// weekend; a cabinet marquee that knows about all of them is not.
//
// It owns no state of its own. Games write JSON into the scores
// directory, and this scans that directory and watches each file.
// Adding a game needs no change here: ranking is declared in each
// record rather than assumed by this widget, so a game where a LOWER
// score wins is read correctly without a single line about that game.
BarWidget {
  id: root
  moduleName: "ridgetopai.omarcade"

  readonly property string scoresDir:
    (Quickshell.env("XDG_STATE_HOME") || Quickshell.env("HOME") + "/.local/state")
    + "/omarcade/scores"

  // Game ids discovered on disk, e.g. ["omarcade-breakout"].
  property var gameIds: []

  // The game whose record was written most recently.
  //
  // This used to be `max(best)` across every game, which was only ever
  // coherent while every game scored the same way. It does not survive
  // a second title: a game where LOWER wins (a time, goals conceded)
  // can never hold a cross-game maximum, so it would be invisible in
  // the bar forever — and comparing 11 points of Pong against 4,200 of
  // Breakout was never a comparison in the first place.
  //
  // Most-recent is comparable by construction — one game, one rule —
  // and in a bar it is the more useful fact anyway: it shows what you
  // just played. The cabinet is where every game and difficulty is
  // listed side by side.
  readonly property var latestRecord: {
    var newest = null
    var newestAt = ""
    for (var i = 0; i < records.count; i++) {
      var r = records.itemAt(i)
      if (!r || !r.record || r.best <= 0) continue
      // RFC 3339 UTC to a fixed width, so a lexical compare IS a
      // chronological one — no date parsing in the shell process.
      var at = String(r.record.updated_at || "")
      if (at > newestAt) { newestAt = at; newest = r }
    }
    return newest
  }

  readonly property int bestScore: latestRecord ? latestRecord.best : 0
  readonly property string bestGame: latestRecord ? latestRecord.label : ""

  // Which difficulty that best was set on, shown only when the game
  // actually has tiers — a label reading "normal" on a single-tier
  // game is noise.
  readonly property string bestDifficulty: {
    if (!latestRecord || !latestRecord.isTiered) return ""
    var entries = latestRecord.record ? latestRecord.record.entries : null
    if (!Array.isArray(entries) || entries.length === 0) return ""
    var top = entries[0]
    return top && top.difficulty !== undefined ? String(top.difficulty) : ""
  }

  readonly property string icon: setting("icon", "🕹")
  readonly property bool showWhenEmpty: setting("showWhenEmpty", false)

  // Nothing to say before the first game is played, and a vertical bar
  // has no room for a score. Both stay out of the way rather than
  // rendering a stub.
  visible: !vertical && (bestScore > 0 || showWhenEmpty)

  implicitWidth: visible
    ? label.implicitWidth + Style.spacing.controlPaddingX * 2
    : 0
  implicitHeight: barSize

  // Width changes when a score gains a digit; animate so the neighbouring
  // widgets slide rather than jump.
  Behavior on implicitWidth {
    NumberAnimation { duration: 180; easing.type: Easing.OutCubic }
  }

  // ------------------------------------------------------------------
  // Discovery: which games have written a score file?
  //
  // A find(1) rather than a hardcoded list, so a game added later shows
  // up with no change to this widget. Same approach omarchy.agents uses
  // to discover agents.
  // ------------------------------------------------------------------

  Process {
    id: scan
    running: false
    command: ["find", root.scoresDir, "-maxdepth", "1", "-name", "*.json", "-printf", "%f\n"]

    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: root.applyListing(text)
    }

    // A missing directory is the normal state before anyone has played,
    // not an error. find exits non-zero; the empty listing is correct.
    onExited: function (code) {
      if (code !== 0) root.gameIds = []
    }
  }

  function applyListing(output) {
    var ids = []
    var lines = String(output || "").split("\n")
    for (var i = 0; i < lines.length; i++) {
      var name = lines[i].trim()
      if (name.length > 5 && name.endsWith(".json"))
        ids.push(name.slice(0, -5))
    }
    ids.sort()

    // Assigning unconditionally would rebuild every FileView on each
    // scan, dropping the watches. Only replace when the set changed.
    if (JSON.stringify(ids) !== JSON.stringify(root.gameIds))
      root.gameIds = ids
  }

  function rescan() {
    if (!scan.running) scan.running = true
  }

  Component.onCompleted: rescan()

  // A new game's first score creates a file this widget has never seen,
  // and no existing FileView is watching it. Re-scan occasionally so it
  // appears. Cheap: one find over a directory holding a handful of files.
  Timer {
    interval: 30000
    running: true
    repeat: true
    onTriggered: root.rescan()
  }

  Repeater {
    id: records
    model: root.gameIds
    delegate: ScoreRecord {
      gameId: modelData
      path: root.scoresDir + "/" + modelData + ".json"
    }
  }

  // ------------------------------------------------------------------
  // Presentation
  // ------------------------------------------------------------------

  Item {
    anchors.fill: parent
    anchors.leftMargin: Style.space(8)
    anchors.rightMargin: Style.space(8)
    clip: true

    Row {
      id: label
      anchors.verticalCenter: parent.verticalCenter
      spacing: Style.space(6)

      Text {
        anchors.verticalCenter: parent.verticalCenter
        text: root.icon
        font.pixelSize: Style.font.body
        color: root.bar ? root.bar.barForeground : Color.foreground
      }

      Text {
        anchors.verticalCenter: parent.verticalCenter
        text: root.bestScore > 0 ? String(root.bestScore) : "—"
        font.family: root.bar ? root.bar.fontFamily : Style.font.family
        font.pixelSize: Style.font.body
        // The score is the point; give it the full foreground weight
        // rather than the muted treatment a label would get.
        color: root.bar ? root.bar.barForeground : Color.foreground
      }
    }
  }

  MouseArea {
    anchors.fill: parent
    hoverEnabled: true
    cursorShape: Qt.PointingHandCursor

    onEntered: {
      if (!root.bar) return
      var tip = "Omarcade — no scores yet"
      if (root.bestScore > 0) {
        tip = root.bestGame + " — best: " + root.bestScore
        if (root.bestDifficulty !== "")
          tip += " (" + root.bestDifficulty + ")"
      }
      root.bar.showTooltip(root, tip)
    }
    onExited: if (root.bar) root.bar.hideTooltip(root)

    // Open the cabinet. Re-scan first, so someone who just finished a
    // game does not wait out the timer to see it.
    onClicked: {
      root.rescan()
      Quickshell.execDetached(["omarchy-shell", "shell", "toggle", root.moduleName, "{}"])
    }
  }
}
