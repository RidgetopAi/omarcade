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
// directory, this scans that directory and watches each file, and the
// highest number wins. Adding a second game needs no change here.
BarWidget {
  id: root
  moduleName: "ridgetopai.omarcade"

  readonly property string scoresDir:
    (Quickshell.env("XDG_STATE_HOME") || Quickshell.env("HOME") + "/.local/state")
    + "/omarcade/scores"

  // Game ids discovered on disk, e.g. ["omarcade-breakout"].
  property var gameIds: []

  // Highest score across every game, and which game holds it.
  readonly property int bestScore: {
    var top = 0
    for (var i = 0; i < records.count; i++) {
      var r = records.itemAt(i)
      if (r && r.best > top) top = r.best
    }
    return top
  }

  readonly property string bestGame: {
    var top = 0
    var name = ""
    for (var i = 0; i < records.count; i++) {
      var r = records.itemAt(i)
      if (r && r.best > top) { top = r.best; name = r.label }
    }
    return name
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
      root.bar.showTooltip(root, root.bestScore > 0
        ? "Omarcade — best: " + root.bestScore + " (" + root.bestGame + ")"
        : "Omarcade — no scores yet")
    }
    onExited: if (root.bar) root.bar.hideTooltip(root)

    // Re-scan on click, so someone who just finished a game does not wait
    // out the timer to see it.
    onClicked: root.rescan()
  }
}
