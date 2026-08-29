import QtQuick
import Quickshell.Io

// One game's score file, read straight off disk and re-read whenever it
// changes. The widget never learns how a score was made — a record that
// appears in the scores directory is a game, whoever wrote it.
//
// Deliberately mirrors omarchy.agents' Agent.qml, which solves the same
// shape of problem. Games write with an atomic rename; FileView's watcher
// survives that (verified) and fires on each replacement.
Item {
  id: root
  visible: false

  property string gameId: ""
  property string path: ""

  // Parsed contents, or null when the file is missing or unusable.
  property var record: null

  readonly property int best: {
    if (!record || !Array.isArray(record.entries) || record.entries.length === 0)
      return 0
    var top = record.entries[0]
    return top && typeof top.score === "number" ? top.score : 0
  }

  readonly property string label: record && record.name ? String(record.name) : gameId

  FileView {
    path: root.path
    watchChanges: true
    printErrors: false
    onFileChanged: reload()
    onLoaded: root.parse(text())
    onLoadFailed: root.record = null
  }

  function parse(content) {
    try {
      var parsed = JSON.parse(String(content || ""))
      // A record from a future release may not mean what this widget
      // thinks it means, so ignore it rather than misreport it.
      if (!parsed || typeof parsed !== "object" || parsed.schema_version !== 1) {
        root.record = null
        return
      }
      root.record = parsed
    } catch (e) {
      // Never throw: this runs inside the long-lived shell process.
      console.warn("omarcade", "ignoring bad score record", root.path, e)
      root.record = null
    }
  }
}
