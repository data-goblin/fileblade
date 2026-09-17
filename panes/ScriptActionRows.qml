import QtQuick
import qs.Commons
import qs.Ui
import "../lib/ActionRows.js" as ActionRows
import "../ui" as PluginUi
import "../theme"

Column {
  id: strip

  required property var controller
  required property var menu

  readonly property var actions: controller.services && controller.services.actions
    ? controller.services.actions : null
  readonly property var rows: actions ? actions.rowsFor(menu.entries, controller.rootPath) : []
  property string resultText: ""

  visible: rows.length > 0
  spacing: Style.space(5)

  function label(action) {
    return action.glyph + "  " + action.title
  }

  function activate(action) {
    if (action.confirm) {
      confirmDialog.pendingKey = action.key
      confirmDialog.open(strip.confirmText(action), [
        { key: "cancel", label: "Cancel" },
        { key: "run", label: "Run", danger: true }
      ])
      return
    }
    strip.start(action.key, false)
  }

  function confirmText(action) {
    var context = ActionRows.contextOf(action, menu.entries, controller.rootPath)
    var count = ActionRows.targetCount(context, menu.entries, controller.rootPath)
    return "Run " + action.title + " on " + count + (count === 1 ? " item" : " items")
      + " (" + context + " context)?"
      + "\n" + action.program
      + "\n" + (action.source === "user" ? "your own action" : action.plugin)
  }

  function start(key, yes) {
    if (!actions) return
    actions.run(key, menu.entries, controller.rootPath, menu.hostWindow ? menu.hostWindow.screen : null, yes)
  }

  Rectangle {
    width: parent.width
    height: 1
    color: Util.alpha(Color.menu.text, 0.14)
  }

  Text {
    textFormat: Text.PlainText
    width: parent.width
    text: "SCRIPTS"
    color: Color.muted
    font.family: Style.font.family
    font.pixelSize: Typography.caption
    font.weight: Font.DemiBold
    font.letterSpacing: 0.4
  }

  Repeater {
    model: strip.rows

    delegate: MenuButton {
      id: scriptRow
      required property var modelData
      menu: strip.menu
      text: strip.label(modelData)
      visible: strip.menu.matches(text)
      enabled: !!strip.actions && !strip.actions.isRunning(modelData.key)
      onClicked: strip.activate(modelData)

      PanelToolTip {
        visible: scriptRow.modelData.description !== "" && scriptRow.menuHighlighted
        text: scriptRow.modelData.description
      }
    }
  }

  PluginUi.ErrorNotice {
    width: parent.width
    text: strip.resultText
    foreground: Color.muted
    onDismissed: strip.resultText = ""
  }

  PluginUi.ActionDialog {
    id: confirmDialog
    actionKeys: strip.menu.actionKeys
    parent: strip.menu.contentItem
    anchors.fill: parent
    z: 97
    property string pendingKey: ""
    onChosen: function(key) {
      var wanted = confirmDialog.pendingKey
      confirmDialog.pendingKey = ""
      if (key === "run") strip.start(wanted, true)
      strip.menu.focusFirst(false)
    }
    onCanceled: {
      confirmDialog.pendingKey = ""
      strip.menu.focusFirst(false)
    }
  }

  Connections {
    target: strip.actions
    ignoreUnknownSignals: true
    function onFinished(key, result) {
      if (!strip.menu.visible) return
      if (result.ok === true && strip.silentFor(key)) strip.menu.returnToTree()
      else strip.resultText = strip.summary(key, result)
    }
  }

  Connections {
    target: strip.menu
    ignoreUnknownSignals: true
    function onVisibleChanged() { if (!strip.menu.visible) strip.resultText = "" }
  }

  function silentFor(key) {
    var action = actions ? actions.find(key) : null
    return !!action && action.output === "silent"
  }

  function summary(key, result) {
    var action = actions ? actions.find(key) : null
    var title = action ? action.title : String(key)
    if (result.ok !== true) return ""
    if (result.detached === true) return title + " started"
    return title + ": " + actions.outcome(result)
  }
}
