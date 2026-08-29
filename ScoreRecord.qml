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

  // Which direction wins. The game declares it; this widget must never
  // assume — a game scored on time or on goals conceded ranks the other
  // way, and picking the biggest number there shows the WORST run.
  // Absent in v1 records, which were all points-scored.
  readonly property bool higherIsBetter:
    !record || record.higher_is_better === undefined
      ? true
      : record.higher_is_better === true

  // Entries are written best-first under the game's own rule, so the
  // top row is the best without re-ranking here.
  readonly property int best: {
    if (!record || !Array.isArray(record.entries) || record.entries.length === 0)
      return 0
    var top = record.entries[0]
    return top && typeof top.score === "number" ? top.score : 0
  }

  // Best per difficulty, as { difficulty: score }. An easy run and a
  // hard run are different games, so they are never merged into one
  // number — the cabinet shows them as separate rows.
  readonly property var bestByDifficulty: {
    var out = ({})
    if (!record || !Array.isArray(record.entries))
      return out
    for (var i = 0; i < record.entries.length; i++) {
      var e = record.entries[i]
      if (!e || typeof e.score !== "number")
        continue
      // v1 entries predate the field.
      var d = e.difficulty === undefined ? "normal" : String(e.difficulty)
      // First match wins: already ordered best-first.
      if (out[d] === undefined)
        out[d] = e.score
    }
    return out
  }

  // Difficulty labels this game has scores for.
  //
  // Ordered easy-to-hard where the names are ones we recognise, then
  // anything else alphabetically. Object.keys alone returns insertion
  // order, which is best-score order — so a good hard run would list
  // before a poor easy one and the row would read as ranked rather
  // than as a scale.
  readonly property var difficultyOrder: ["easy", "normal", "hard"]

  readonly property var difficulties: {
    var keys = Object.keys(bestByDifficulty)
    var known = []
    var rest = []
    for (var i = 0; i < keys.length; i++) {
      if (difficultyOrder.indexOf(keys[i]) >= 0) known.push(keys[i])
      else rest.push(keys[i])
    }
    known.sort(function (a, b) {
      return difficultyOrder.indexOf(a) - difficultyOrder.indexOf(b)
    })
    rest.sort()
    return known.concat(rest)
  }

  // True when this game has more than one tier, so the cabinet knows
  // whether a difficulty label is worth showing at all.
  readonly property bool isTiered: difficulties.length > 1

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
