import QtQuick
import qs.Commons
import qs.Commons as Commons

Item {
  id: controls

  property var view: null
  property var widthFor: function(key) { return -1 }
  property int triggerWidth: Style.space(66)
  property int adderWidth: Style.space(20)
  readonly property int count: view ? view.columns.length : 0
  readonly property bool canAdd: !!view && (view.canAddColumn !== undefined ? view.canAddColumn : view.columns.length < view.options.filter(function(option) {
    var key = String(option.key || option.value || "")
    return key !== "off" && key !== "none"
  }).length)
  readonly property bool addGuideVisible: controls.visible && adder.visible && adder.hovered
  readonly property real addGuideX: adder.x + adder.width / 2
  readonly property real addSlotWidth: controls.visible && adder.visible ? adder.width + strip.spacing : 0
  property int dragFrom: -1
  property int dragTarget: -1

  implicitWidth: strip.implicitWidth
  implicitHeight: Style.space(24)

  function openFilter() {
    var first = pickers.itemAt(0)
    if (first) first.openFilter()
    else adder.openFilter()
  }

  function targetIndexAt(x) {
    var best = -1
    for (var i = 0; i < pickers.count; i++) {
      var item = pickers.itemAt(i)
      if (!item) continue
      if (x < item.x + item.width / 2) return i
      best = i
    }
    return Math.max(0, best)
  }

  function beginOrUpdateDrag(index, x) {
    dragFrom = index
    dragTarget = targetIndexAt(x)
  }

  function finishDrag(x) {
    var from = dragFrom
    var to = targetIndexAt(x)
    dragFrom = -1
    dragTarget = -1
    if (view && from >= 0 && to >= 0 && from !== to) view.moveColumn(from, to)
  }

  function cancelDrag() {
    dragFrom = -1
    dragTarget = -1
  }

  Row {
    id: strip
    anchors.fill: parent
    spacing: Style.space(2)

    Repeater {
      id: pickers
      model: controls.count

      delegate: MetricPicker {
        id: cell
        required property int index
        readonly property string key: controls.view && index < controls.view.columns.length ? String(controls.view.columns[index]) : "off"
        readonly property int explicitWidth: controls.widthFor(key)
        readonly property int wanted: explicitWidth > 0
          ? explicitWidth
          : (controls.view && typeof controls.view.columnWidthFor === "function" ? controls.view.columnWidthFor(key) : controls.triggerWidth)
        view: controls.view
        columnIndex: index
        triggerWidth: wanted
        height: strip.height
        opacity: controls.dragFrom === index ? 0.5 : 1
        onDragMoved: function(x) { controls.beginOrUpdateDrag(index, x) }
        onDragFinished: function(x) { controls.finishDrag(x) }
        onDragCancelled: controls.cancelDrag()
      }
    }

    MetricPicker {
      id: adder
      view: controls.view
      columnIndex: -1
      triggerWidth: controls.adderWidth
      height: strip.height
      visible: controls.canAdd
    }
  }

  Rectangle {
    visible: controls.dragFrom >= 0 && controls.dragTarget >= 0
    x: {
      var item = pickers.itemAt(controls.dragTarget)
      if (!item) return 0
      return controls.dragTarget > controls.dragFrom ? item.x + item.width - 1 : item.x
    }
    width: 2
    height: parent.height
    color: Commons.Color.accent
  }

}
